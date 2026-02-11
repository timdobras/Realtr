//! Auto-straighten module using Canny edge detection + Hough line detection.
//!
//! Primary approach: Canny + Hough to find real structural lines, then extract
//! tilt from near-vertical and near-horizontal line angles.
//!
//! The gradient histogram approach was abandoned because the Sobel gradient
//! at 3x3 kernel size cannot resolve tilt angles below ~15 degrees — on a
//! rasterized edge, the local pixel neighborhood looks identical for 0 and 3
//! degree tilt. Hough line detection works globally and CAN resolve small tilts.
//!
//! Pipeline:
//! 1. Preprocessing (bilateral filter, CLAHE, lens distortion)
//! 2. Canny edge detection with adaptive thresholds
//! 3. Hough line transform (detects lines as angle + distance from origin)
//! 4. Filter lines to near-vertical (angle near 0/180) and near-horizontal (near 90)
//! 5. Extract tilt from line angle distribution (weighted median)
//! 6. V/H combination with agreement scoring
//! 7. VP validation for cross-check
//! 8. Multi-resolution agreement (full + half scale)
//! 9. Safety limits

use crate::gpu::ImageProcessor;
use crate::perspective::preprocessing::{
    preprocess_for_detection, preprocess_for_detection_no_exif,
};
use crate::perspective::vanishing::validate_with_vp;
use image::{DynamicImage, GenericImageView, GrayImage};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Constants
// ============================================================================

/// Maximum rotation to apply (degrees)
const MAX_ROTATION_DEG: f64 = 10.0;

/// Minimum rotation to bother applying (degrees)
const MIN_ROTATION_THRESHOLD_DEG: f64 = 0.1;

/// Tolerance for "near vertical" lines: angle within ±this of 0 or 180.
/// Using 5° to capture the full range of structural lines. Noise at 3-5° is
/// handled by iterative sigma-clipping in the tilt extraction stage rather
/// than by narrowing the detection window (which biases group averages).
const VERTICAL_TOLERANCE_DEG: u32 = 5;

/// Tolerance for "near horizontal" lines: angle within ±this of 90
const HORIZONTAL_TOLERANCE_DEG: u32 = 5;

/// V/H agreement threshold in degrees
const VH_AGREEMENT_THRESHOLD_DEG: f64 = 1.5;

/// Minimum line votes as fraction of image min-dimension
const MIN_VOTE_FRACTION: f64 = 0.12;

/// Minimum number of lines required for reliable detection
const MIN_LINES_FOR_DETECTION: usize = 3;

/// Angle resolution for custom Hough accumulator (degrees per bin)
/// Increased from 0.25° to 0.1° for finer angular precision.
/// Combined with parabolic peak interpolation, effective resolution is ~0.02°.
const HOUGH_ANGLE_STEP: f64 = 0.1;

/// Non-maximum suppression radius in angle bins for Hough peaks.
/// Scaled up from 3 to 7 to match the finer bin resolution
/// (7 bins × 0.1° = 0.7°, similar to old 3 bins × 0.25° = 0.75°).
const HOUGH_NMS_RADIUS: usize = 7;

/// Non-maximum suppression radius in r bins for Hough peaks
const HOUGH_R_NMS_RADIUS: usize = 12;

// ============================================================================
// Types
// ============================================================================

/// Result of straightening analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StraightenResult {
    /// Suggested rotation in degrees (negative = clockwise)
    pub suggested_rotation: f64,
    /// Confidence score 0.0-1.0
    pub confidence: f32,
    /// Number of lines used in analysis
    pub lines_used: usize,
    /// Whether V and H analysis agreed
    pub vh_agreement: bool,
}

/// A real line segment detected in the image (from Hough transform)
#[derive(Debug, Clone)]
pub struct LineSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub length: f64,
    /// Angle from vertical in degrees (0 = perfectly vertical)
    pub angle_from_vertical: f64,
    /// Angle from horizontal in degrees (0 = perfectly horizontal)
    pub angle_from_horizontal: f64,
}

impl LineSegment {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let length = (dx * dx + dy * dy).sqrt();

        let (norm_dx, norm_dy) = if dy >= 0.0 { (dx, dy) } else { (-dx, -dy) };

        let angle_from_vertical = norm_dx.atan2(norm_dy).to_degrees();
        let angle_from_horizontal = norm_dy.atan2(norm_dx.abs()).to_degrees();

        Self {
            x1,
            y1,
            x2,
            y2,
            length,
            angle_from_vertical,
            angle_from_horizontal,
        }
    }

    #[allow(dead_code)]
    pub fn midpoint(&self) -> (f64, f64) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }
}

/// Type of line based on angle
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineType {
    Vertical,
    Horizontal,
}

/// A classified line with weight (used by VP validation)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClassifiedLine {
    pub segment: LineSegment,
    pub line_type: LineType,
    pub weight: f64,
}

/// A detected Hough line with its tilt information
#[derive(Debug, Clone)]
struct HoughLine {
    /// Hough angle (0-179 degrees, 0=vertical, 90=horizontal)
    hough_angle: u32,
    /// Signed tilt from exact V or H (degrees) — integer-rounded
    tilt_deg: f64,
    /// Signed tilt with sub-degree precision from custom Hough (0.25° steps)
    tilt_precise: f64,
    /// Number of votes (edge pixels supporting this line)
    votes: u32,
    /// Whether this is a vertical or horizontal line
    line_type: LineType,
    /// Distance from origin
    r: f32,
}

// ============================================================================
// Main Entry Points
// ============================================================================

/// Analyze image for straightening with EXIF support for lens correction.
pub fn analyze_straighten(
    img: &DynamicImage,
    image_path: Option<&Path>,
    processor: &ImageProcessor,
) -> StraightenResult {
    let gray = preprocess_for_detection(img, image_path, processor);
    analyze_straighten_from_gray(&gray, img.dimensions(), processor)
}

/// Analyze image for straightening without EXIF (for preview images).
#[allow(dead_code)]
pub fn analyze_straighten_no_exif(
    img: &DynamicImage,
    processor: &ImageProcessor,
) -> StraightenResult {
    let gray = preprocess_for_detection_no_exif(img, processor);
    analyze_straighten_from_gray(&gray, img.dimensions(), processor)
}

/// Core analysis on preprocessed grayscale image.
fn analyze_straighten_from_gray(
    gray: &GrayImage,
    _original_dims: (u32, u32),
    _processor: &ImageProcessor,
) -> StraightenResult {
    let (width, height) = gray.dimensions();

    eprintln!("[straighten] image: {width}x{height}");

    // Multi-resolution analysis: full size + half size
    let (full_result, full_lines) = analyze_at_resolution(gray);

    let half_gray = downsample_gray(gray);
    let (half_result, _half_lines) = analyze_at_resolution(&half_gray);

    eprintln!(
        "[straighten] full-res: angle={:.3}, conf={:.3}, lines={}",
        full_result.suggested_rotation, full_result.confidence, full_result.lines_used
    );
    eprintln!(
        "[straighten] half-res: angle={:.3}, conf={:.3}, lines={}",
        half_result.suggested_rotation, half_result.confidence, half_result.lines_used
    );

    // Multi-resolution agreement
    let result = combine_multi_resolution(&full_result, &half_result);

    // VP validation with cached full-res lines (no redundant re-detection)
    let result = validate_with_real_lines(&result, &full_lines, (width, height));

    // Safety limits
    let (final_angle, final_confidence) =
        apply_safety_limits(result.suggested_rotation, result.confidence);

    StraightenResult {
        suggested_rotation: final_angle,
        confidence: final_confidence,
        lines_used: result.lines_used,
        vh_agreement: result.vh_agreement,
    }
}

// ============================================================================
// Core: Hough-based Tilt Detection
// ============================================================================

/// Analyze at a single resolution using Canny + Hough.
/// Returns both the straighten result and the detected Hough lines (for reuse).
fn analyze_at_resolution(gray: &GrayImage) -> (StraightenResult, Vec<HoughLine>) {
    let lines = detect_hough_lines(gray);

    if lines.is_empty() {
        eprintln!("[straighten] no Hough lines detected");
        return (no_correction(), lines);
    }

    // Separate V-lines into near-0 and near-180 groups.
    // These two groups use OPPOSITE sign conventions for the same physical tilt:
    // - Near-0: tilt = -angle (positive angle → negative tilt)
    // - Near-180: tilt = -(angle - 180) (angle < 180 → positive tilt)
    // A wall tilted CW by X° gives tilt ≈ -X in near-0, tilt ≈ +X in near-180.
    // Combining them directly causes cancellation. Process separately instead.
    let v_near0: Vec<&HoughLine> = lines
        .iter()
        .filter(|l| l.line_type == LineType::Vertical && (l.hough_angle as f64) < 90.0)
        .collect();
    let v_near180: Vec<&HoughLine> = lines
        .iter()
        .filter(|l| l.line_type == LineType::Vertical && (l.hough_angle as f64) >= 90.0)
        .collect();
    let horizontal: Vec<&HoughLine> = lines
        .iter()
        .filter(|l| l.line_type == LineType::Horizontal)
        .collect();

    eprintln!(
        "[straighten] Hough: {} V-near0, {} V-near180, {} horizontal lines",
        v_near0.len(),
        v_near180.len(),
        horizontal.len()
    );

    // Extract tilt from each V-group separately
    let v0_tilt = extract_tilt_from_lines(&v_near0);
    let v180_tilt = extract_tilt_from_lines(&v_near180);

    // Also extract from all V-lines combined (used as fallback for noisy images
    // where both groups have low agreement — the combined approach naturally
    // cancels opposite-sign noise and gives ~0° for level images)
    let all_vertical: Vec<&HoughLine> = lines
        .iter()
        .filter(|l| l.line_type == LineType::Vertical)
        .collect();
    let v_combined_tilt = extract_tilt_from_lines(&all_vertical);

    // Combine the two V-group estimates
    let v_tilt = combine_v_group_tilts(&v0_tilt, &v180_tilt, &v_combined_tilt);

    // Extract tilt from horizontal lines
    let h_tilt = extract_tilt_from_lines(&horizontal);

    if let Some((v_angle, v_conf, v_agree)) = &v_tilt {
        eprintln!(
            "[straighten] V-tilt: {:.3} deg, confidence={:.3}, agreement={:.2}",
            v_angle, v_conf, v_agree
        );
    }
    if let Some((h_angle, h_conf, h_agree)) = &h_tilt {
        eprintln!(
            "[straighten] H-tilt: {:.3} deg, confidence={:.3}, agreement={:.2}",
            h_angle, h_conf, h_agree
        );
    }

    // Combine V/H
    let result = combine_vh_tilts(&v_tilt, &h_tilt, lines.len());
    (result, lines)
}

/// Detect lines using custom sub-degree Hough accumulator focused on near-V/H angles.
///
/// Unlike `imageproc::hough::detect_lines` which uses 1° integer resolution and returns
/// no vote counts, this builds a custom accumulator with 0.1° resolution in two narrow
/// angular bands (near-vertical and near-horizontal), plus parabolic peak interpolation
/// for ~0.02° effective resolution. Returns actual vote counts per line.
///
/// This solves the core problems with the old approach:
/// 1. 0.1° bins + parabolic interpolation → ~0.02° effective resolution
/// 2. Vote counts let us weight long wall edges more than short shelf edges
/// 3. Narrow angular bands (±3°) exclude perspective/diagonal/noise lines
/// 4. No dual-representation problem since we only sample [0, TOLERANCE] and [87, 93]
fn detect_hough_lines(gray: &GrayImage) -> Vec<HoughLine> {
    let (width, height) = gray.dimensions();
    let min_dim = width.min(height);

    // Adaptive Canny thresholds
    let (low_thresh, high_thresh) = compute_canny_thresholds(gray);

    // Run Canny edge detection
    let edges = imageproc::edges::canny(gray, low_thresh, high_thresh);

    // Build angle ranges to scan (in degrees):
    // Near-vertical: (0, V_TOL] ∪ [180-V_TOL, 180) — both sides, excluding exact 0°
    //   We exclude angle=0.0 because at θ=0, cos(0)=1, sin(0)=0, so r=x.
    //   Every edge pixel at a given x-coordinate votes for exactly (0, x),
    //   creating artificial spikes regardless of actual tilt. Starting at 0.25°
    //   avoids this while still capturing CCW-leaning lines.
    //   The (0, V_TOL] range detects CCW tilt (tilt_correction = negative).
    //   The [180-V_TOL, 180) range detects CW tilt (tilt_correction = positive).
    //
    // Near-horizontal: [90-H_TOL, 90+H_TOL] — centered on horizontal

    let v_tol = VERTICAL_TOLERANCE_DEG as f64;
    let h_tol = HORIZONTAL_TOLERANCE_DEG as f64;

    // Near-vertical angles: (0, V_TOL] — for CCW tilts
    // Start at HOUGH_ANGLE_STEP (0.25°) to skip exact 0°
    let mut v0_angles: Vec<f64> = Vec::new();
    let mut a = HOUGH_ANGLE_STEP;
    while a <= v_tol {
        v0_angles.push(a);
        a += HOUGH_ANGLE_STEP;
    }

    // Near-horizontal angles
    let mut h_angles: Vec<f64> = Vec::new();
    a = 90.0 - h_tol;
    while a <= 90.0 + h_tol {
        h_angles.push(a);
        a += HOUGH_ANGLE_STEP;
    }

    // Near-vertical angles: [180-V_TOL, 180) — for CW tilts
    let mut v180_angles: Vec<f64> = Vec::new();
    a = 180.0 - v_tol;
    while a < 180.0 {
        v180_angles.push(a);
        a += HOUGH_ANGLE_STEP;
    }

    let all_angles: Vec<f64> = v0_angles
        .iter()
        .chain(h_angles.iter())
        .chain(v180_angles.iter())
        .copied()
        .collect();

    let num_angles = all_angles.len();

    // Track group boundaries for NMS (prevent cross-group suppression)
    // Groups: [0..v0_end) = near-0 vertical, [v0_end..h_end) = horizontal, [h_end..num_angles) = near-180 vertical
    let v0_end = v0_angles.len();
    let h_end = v0_end + h_angles.len();
    // v180 starts at h_end, ends at num_angles

    // Precompute sin/cos for each angle
    let sin_cos: Vec<(f64, f64)> = all_angles
        .iter()
        .map(|&deg| {
            let rad = deg.to_radians();
            (rad.sin(), rad.cos())
        })
        .collect();

    // Max r value: diagonal of the image
    let max_r = ((width as f64).powi(2) + (height as f64).powi(2)).sqrt();
    let r_range = (max_r * 2.0).ceil() as usize + 1; // r goes from -max_r to +max_r
    let r_offset = max_r; // offset to make index non-negative

    // Build accumulator
    let mut accumulator = vec![0u32; num_angles * r_range];

    // Exclude a small border margin from voting to avoid:
    // 1. Image boundary edges (always perfectly V/H, creating false peaks)
    // 2. Black border artifacts from rotation (very strong Canny edges)
    // 3. JPEG compression artifacts along image edges
    // A 2% margin is enough to skip these without losing interior structure.
    let margin_x = (width as f64 * 0.02).ceil() as u32;
    let margin_y = (height as f64 * 0.02).ceil() as u32;
    let x_start = margin_x;
    let x_end = width.saturating_sub(margin_x);
    let y_start = margin_y;
    let y_end = height.saturating_sub(margin_y);

    // Vote: for each edge pixel (inside margin), compute r for each angle and increment
    for y in y_start..y_end {
        for x in x_start..x_end {
            if edges.get_pixel(x, y)[0] == 0 {
                continue;
            }
            let xf = x as f64;
            let yf = y as f64;

            for (ai, &(sin, cos)) in sin_cos.iter().enumerate() {
                let r = xf * cos + yf * sin;
                let ri = (r + r_offset).round() as usize;
                if ri < r_range {
                    accumulator[ai * r_range + ri] += 1;
                }
            }
        }
    }

    // Vote threshold (adaptive based on image size)
    let vote_threshold = ((f64::from(min_dim) * MIN_VOTE_FRACTION) as u32).max(20);

    // Determine the NMS group boundaries for each angle index.
    // This prevents a strong horizontal line from suppressing a weak vertical line
    // (or vice versa) just because they happen to be adjacent in the array.
    let group_of = |ai: usize| -> u8 {
        if ai < v0_end {
            0 // near-0 vertical
        } else if ai < h_end {
            1 // horizontal
        } else {
            2 // near-180 vertical
        }
    };

    // Extract peaks with non-maximum suppression (within same group only)
    let mut result = Vec::new();
    let a_nms = HOUGH_NMS_RADIUS;
    let r_nms = HOUGH_R_NMS_RADIUS;

    for ai in 0..num_angles {
        for ri in 0..r_range {
            let votes = accumulator[ai * r_range + ri];
            if votes < vote_threshold {
                continue;
            }

            let my_group = group_of(ai);

            // Non-maximum suppression: check if this is the local max (within same group)
            let mut is_max = true;
            'nms: for dai in 0..=(2 * a_nms) {
                let nai = (ai + dai).wrapping_sub(a_nms);
                if nai >= num_angles {
                    continue;
                }
                // Only compare within the same angular group
                if group_of(nai) != my_group {
                    continue;
                }
                for dri in 0..=(2 * r_nms) {
                    let nri = (ri + dri).wrapping_sub(r_nms);
                    if nri >= r_range {
                        continue;
                    }
                    if nai == ai && nri == ri {
                        continue;
                    }
                    if accumulator[nai * r_range + nri] > votes {
                        is_max = false;
                        break 'nms;
                    }
                }
            }

            if !is_max {
                continue;
            }

            let angle_deg = all_angles[ai];
            let r = ri as f64 - r_offset;

            // Parabolic interpolation in the angle direction for sub-bin precision.
            // Fit a parabola to the 3 bins (ai-1, ai, ai+1) at the same r index.
            // The refined angle offset = 0.5 * (left - right) / (left - 2*center + right)
            // This gives ~0.02° effective resolution with 0.1° bins.
            // Only interpolate within the same angular group (don't cross group boundaries).
            let refined_angle = if ai > 0
                && ai + 1 < num_angles
                && group_of(ai - 1) == my_group
                && group_of(ai + 1) == my_group
            {
                let left = accumulator[(ai - 1) * r_range + ri] as f64;
                let center = votes as f64;
                let right = accumulator[(ai + 1) * r_range + ri] as f64;
                let denom = left - 2.0 * center + right;
                if denom.abs() > 1e-6 {
                    let offset = 0.5 * (left - right) / denom;
                    // Clamp offset to ±0.5 bins (sanity check)
                    let offset = offset.clamp(-0.5, 0.5);
                    angle_deg + offset * HOUGH_ANGLE_STEP
                } else {
                    angle_deg
                }
            } else {
                angle_deg
            };

            // Classify line using refined angle
            if refined_angle >= (90.0 - h_tol) && refined_angle <= (90.0 + h_tol) {
                // Near-horizontal
                let tilt = refined_angle - 90.0;

                result.push(HoughLine {
                    hough_angle: refined_angle.round() as u32,
                    tilt_deg: tilt,
                    tilt_precise: tilt,
                    votes,
                    line_type: LineType::Horizontal,
                    r: r as f32,
                });
            } else if refined_angle > 0.0 && refined_angle <= v_tol {
                // Near-vertical from the (0, V_TOL] range — CCW tilt
                let tilt = -refined_angle;

                result.push(HoughLine {
                    hough_angle: refined_angle.round() as u32,
                    tilt_deg: tilt,
                    tilt_precise: tilt,
                    votes,
                    line_type: LineType::Vertical,
                    r: r as f32,
                });
            } else if refined_angle >= (180.0 - v_tol) {
                // Near-vertical from the [180-V_TOL, 180) range — CW tilt
                let tilt = -(refined_angle - 180.0);

                result.push(HoughLine {
                    hough_angle: refined_angle.round() as u32,
                    tilt_deg: tilt,
                    tilt_precise: tilt,
                    votes,
                    line_type: LineType::Vertical,
                    r: r as f32,
                });
            }
        }
    }

    // Deduplicate vertical lines: same physical line appears in both (0, V_TOL] and [180-V_TOL, 180)
    dedup_vertical_lines(&mut result, min_dim);

    eprintln!(
        "[straighten] Canny({low_thresh:.0},{high_thresh:.0}) + CustomHough(res={HOUGH_ANGLE_STEP:.2}, thresh={vote_threshold}): {} lines",
        result.len()
    );

    result
}

/// Deduplicate vertical lines that appear as dual Hough representations.
///
/// Near-vertical lines can appear in both the (0, V_TOL] and [180-V_TOL, 180)
/// angle ranges. We match pairs by checking:
/// 1. Angles are supplementary: |angle_0 + angle_180 - 180| < 1.0
/// 2. Spatial proximity: |r_0 + r_180| < threshold (proportional to image size)
///
/// For matched pairs, we keep the line with more votes but set its tilt_precise
/// to the average of both tilts (which is ~0.0 for genuine duplicates), avoiding
/// a random +/-0.25° bias from the sign convention difference between the two branches.
fn dedup_vertical_lines(lines: &mut Vec<HoughLine>, min_dim: u32) {
    // Scale dedup threshold with image size (48px at 800px, scales proportionally)
    let r_threshold = min_dim as f32 * 0.06;

    // Separate into angle-near-0 and angle-near-180 groups
    let (near_0, near_180): (Vec<(usize, &HoughLine)>, Vec<(usize, &HoughLine)>) = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.line_type == LineType::Vertical)
        .partition(|(_, l)| (l.hough_angle as f64) < 90.0);

    let near_0_indices: Vec<usize> = near_0.into_iter().map(|(i, _)| i).collect();
    let near_180_indices: Vec<usize> = near_180.into_iter().map(|(i, _)| i).collect();

    if near_0_indices.is_empty() || near_180_indices.is_empty() {
        return;
    }

    let mut dedup_actions: Vec<(usize, usize)> = Vec::new(); // (index_to_remove, index_to_keep)
    let mut matched_180: Vec<bool> = vec![false; near_180_indices.len()];

    for &idx_0 in &near_0_indices {
        let r_0 = lines[idx_0].r;
        let angle_0 = lines[idx_0].hough_angle as f64;

        let mut best_match: Option<(usize, f32)> = None;
        for (j, &idx_180) in near_180_indices.iter().enumerate() {
            if matched_180[j] {
                continue;
            }
            let angle_180 = lines[idx_180].hough_angle as f64;

            // Check angles are supplementary (same physical line orientation).
            // For a genuine dual pair, angle_0 + angle_180 should equal ~180.
            // Only truly near-vertical lines (both within ~1.0° of 0/180) produce duals.
            if (angle_0 + angle_180 - 180.0).abs() > 1.0 {
                continue;
            }

            // Only deduplicate lines where both tilts are very small (< 0.5°).
            // Lines with significant tilt only appear in one range, not both.
            // At 1° tilt the Hough peak may appear at bin 0.25° (near-0 side, |tilt|=0.25)
            // and bin 179.25° (near-180 side, |tilt|=0.75). Using 0.5° threshold ensures
            // that only genuinely near-vertical lines (both tilts < 0.5°) get deduped.
            let tilt_0 = lines[idx_0].tilt_precise.abs();
            let tilt_180 = lines[idx_180].tilt_precise.abs();
            if tilt_0 >= 0.5 || tilt_180 >= 0.5 {
                continue;
            }

            let r_180 = lines[idx_180].r;
            let distance = (r_0 + r_180).abs();
            if distance < r_threshold {
                if best_match.is_none() || distance < best_match.unwrap().1 {
                    best_match = Some((j, distance));
                }
            }
        }

        if let Some((j, _)) = best_match {
            matched_180[j] = true;
            let idx_180 = near_180_indices[j];

            // Keep the one with more votes, remove the other
            if lines[idx_0].votes >= lines[idx_180].votes {
                dedup_actions.push((idx_180, idx_0));
            } else {
                dedup_actions.push((idx_0, idx_180));
            }
        }
    }

    if !dedup_actions.is_empty() {
        eprintln!(
            "[straighten] dedup: matched {} dual pairs",
            dedup_actions.len()
        );
    }

    // For matched pairs: set the surviving line's tilt to the average of both tilts.
    // Since the two branches assign opposite signs for the same physical line
    // (near-0: tilt = -angle, near-180: tilt = -(angle-180) = 180-angle),
    // the average is ~0.0 for genuine near-vertical duplicates, avoiding random bias.
    for &(_, keep_idx) in &dedup_actions {
        // Find the corresponding remove index to get the other tilt
        let remove_idx = dedup_actions
            .iter()
            .find(|(_, k)| *k == keep_idx)
            .map(|(r, _)| *r)
            .unwrap_or(keep_idx);
        if remove_idx != keep_idx {
            let tilt_a = lines[keep_idx].tilt_precise;
            let tilt_b = lines[remove_idx].tilt_precise;
            let avg_tilt = (tilt_a + tilt_b) / 2.0;
            lines[keep_idx].tilt_precise = avg_tilt;
            lines[keep_idx].tilt_deg = avg_tilt;
        }
    }

    // Remove duplicates (sort indices descending to preserve earlier indices)
    let mut indices_to_remove: Vec<usize> = dedup_actions.iter().map(|(r, _)| *r).collect();
    indices_to_remove.sort_unstable_by(|a, b| b.cmp(a));
    indices_to_remove.dedup();
    for idx in indices_to_remove {
        lines.remove(idx);
    }
}

/// Tilt extraction result: (refined_tilt, confidence, agreement_ratio)
type TiltResult = (f64, f64, f64);

/// Extract the dominant tilt angle from a set of near-V or near-H lines.
///
/// Uses iterative sigma-clipping for robust outlier rejection:
/// 1. Start with vote-weighted median as initial center
/// 2. Compute weighted stddev, reject points beyond 2σ
/// 3. Repeat until convergence (max 5 iterations)
/// 4. Final weighted mean of surviving inliers
///
/// Returns (tilt_degrees, confidence, agreement_ratio) or None if not enough lines.
fn extract_tilt_from_lines(lines: &[&HoughLine]) -> Option<TiltResult> {
    if lines.len() < MIN_LINES_FOR_DETECTION {
        return None;
    }

    // Collect tilt angles with weights (vote count as weight)
    let tilts: Vec<(f64, f64)> = lines
        .iter()
        .map(|l| (l.tilt_precise, l.votes as f64))
        .collect();

    let total_weight: f64 = tilts.iter().map(|(_, w)| w).sum();

    // Sort for weighted median computation
    let mut sorted_tilts = tilts.clone();
    sorted_tilts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Weighted median as robust starting point
    let mut cumulative = 0.0;
    let mut center = sorted_tilts[sorted_tilts.len() / 2].0;
    for &(tilt, weight) in &sorted_tilts {
        cumulative += weight;
        if cumulative >= total_weight / 2.0 {
            center = tilt;
            break;
        }
    }

    // Two-pass outlier rejection:
    // Pass 1: Fixed 1.0° threshold from weighted median (removes gross outliers)
    // Pass 2: Adaptive 1.5σ clip from the refined center (tightens the cluster)
    let mut inlier_mask = vec![true; tilts.len()];

    // Pass 1: Fixed-threshold rejection from weighted median
    let fixed_threshold = 1.0_f64;
    for (i, &(tilt, _)) in tilts.iter().enumerate() {
        if (tilt - center).abs() > fixed_threshold {
            inlier_mask[i] = false;
        }
    }

    // Ensure we keep at least MIN_LINES_FOR_DETECTION inliers after pass 1
    let inlier_count = inlier_mask.iter().filter(|&&x| x).count();
    if inlier_count < MIN_LINES_FOR_DETECTION {
        inlier_mask.fill(true);
    }

    // Pass 2: Iterative sigma-clipping with 1.5σ
    let sigma_clip = 1.5_f64;
    let max_iterations = 3;

    for _iter in 0..max_iterations {
        let mut w_sum = 0.0_f64;
        let mut tw_sum = 0.0_f64;
        for (i, &(tilt, weight)) in tilts.iter().enumerate() {
            if inlier_mask[i] {
                tw_sum += tilt * weight;
                w_sum += weight;
            }
        }
        if w_sum <= 0.0 {
            break;
        }
        let mean = tw_sum / w_sum;

        let mut var_sum = 0.0_f64;
        for (i, &(tilt, weight)) in tilts.iter().enumerate() {
            if inlier_mask[i] {
                let d = tilt - mean;
                var_sum += d * d * weight;
            }
        }
        let stddev = (var_sum / w_sum).sqrt();

        // Minimum threshold of 0.2° to avoid over-clipping perfectly aligned lines
        let threshold = (stddev * sigma_clip).max(0.2);

        let mut changed = false;
        for (i, &(tilt, _)) in tilts.iter().enumerate() {
            let should_be_inlier = (tilt - mean).abs() <= threshold;
            if inlier_mask[i] != should_be_inlier {
                inlier_mask[i] = should_be_inlier;
                changed = true;
            }
        }

        center = mean;

        if !changed {
            break;
        }

        let inlier_count = inlier_mask.iter().filter(|&&x| x).count();
        if inlier_count < MIN_LINES_FOR_DETECTION {
            inlier_mask.fill(true);
            break;
        }
    }

    // Compute final statistics from inliers
    let mut inlier_sum = 0.0_f64;
    let mut inlier_w = 0.0_f64;
    let mut inlier_weight_total = 0.0_f64;
    for (i, &(tilt, weight)) in tilts.iter().enumerate() {
        if inlier_mask[i] {
            inlier_sum += tilt * weight;
            inlier_w += weight;
            inlier_weight_total += weight;
        }
    }
    let refined_tilt = if inlier_w > 0.0 {
        inlier_sum / inlier_w
    } else {
        center
    };

    let agreement_ratio = inlier_weight_total / total_weight;

    // Stddev of final inliers
    let variance = if inlier_w > 0.0 {
        tilts
            .iter()
            .enumerate()
            .filter(|(i, _)| inlier_mask[*i])
            .map(|(_, (t, w))| {
                let d = t - refined_tilt;
                d * d * w
            })
            .sum::<f64>()
            / inlier_w
    } else {
        10.0
    };
    let stddev = variance.sqrt();

    // Confidence based on four factors:
    // - Vote strength (20%): rewards having many strong, well-supported lines
    // - Inlier count (15%): more surviving inliers = more structural lines confirmed
    // - Agreement ratio (30%): inlier weight / total weight (reduced from 45% because
    //   the two-pass outlier rejection legitimately removes dual-representation ghosts,
    //   which lowers this ratio even for good detections)
    // - Standard deviation (35%): lower = more consistent = highest weight because
    //   tight clustering after clipping is the strongest indicator of real structure
    let max_vote = lines.iter().map(|l| l.votes).max().unwrap_or(1) as f64;
    let vote_strength_score = (total_weight / (max_vote * 8.0)).min(1.0);
    let inlier_count = inlier_mask.iter().filter(|&&x| x).count();
    let inlier_count_score = (inlier_count as f64 / 5.0).min(1.0);
    let agreement_score = agreement_ratio;
    let stddev_score = (1.0 - stddev / 1.5).max(0.0);

    let confidence = (vote_strength_score * 0.20
        + inlier_count_score * 0.15
        + agreement_score * 0.30
        + stddev_score * 0.35)
        .clamp(0.0, 0.95);

    eprintln!(
        "[straighten]   lines={} (inliers={}), median={:.3}, refined={:.4}, stddev={:.4}, agreement={:.2}, conf={:.3}",
        lines.len(), inlier_count, center, refined_tilt, stddev, agreement_ratio, confidence
    );

    Some((refined_tilt, confidence, agreement_ratio))
}

// ============================================================================
// V-Group Combination (near-0 vs near-180)
// ============================================================================

/// Combine tilt estimates from the two vertical line groups.
///
/// The near-0 and near-180 groups detect DIFFERENT directions of lean:
/// - Near-0 (θ ∈ (0, 5°]): detects CW lean → tilt is negative (CW correction)
/// - Near-180 (θ ∈ [175°, 180)): detects CCW lean → tilt is positive (CCW correction)
///
/// For a tilted image, the real signal appears in ONE group with consistent tilt,
/// while the other group may contain dual-representation noise near zero.
/// For a level image, both groups have tilt near zero.
///
/// We pick the group with the better detection quality, measured by
/// confidence × agreement (higher = more inliers agree on a consistent tilt).
fn combine_v_group_tilts(
    v0: &Option<TiltResult>,
    v180: &Option<TiltResult>,
    v_combined: &Option<TiltResult>,
) -> Option<TiltResult> {
    match (v0, v180) {
        (Some((t0, c0, a0)), Some((t180, c180, a180))) => {
            // Quality score: confidence × agreement. Higher = more reliable detection.
            let q0 = c0 * a0;
            let q180 = c180 * a180;

            // If BOTH groups have low agreement, neither found a strong structural
            // signal — fall back to combined extraction which naturally cancels
            // opposite-sign noise for level images.
            let min_agreement_for_trust = 0.40;
            let (tilt, conf, agree) = if *a0 < min_agreement_for_trust
                && *a180 < min_agreement_for_trust
            {
                if let Some((tc, cc, ac)) = v_combined {
                    // Heavy confidence penalty when falling back to combined —
                    // the separate groups couldn't find structure, so the combined
                    // result is likely noise. Only trust small angles.
                    let penalty = if *ac < 0.25 { 0.60 } else { 0.75 };
                    eprintln!(
                        "[straighten] V-groups both low agreement (a0={:.2}, a180={:.2}) → combined fallback {:.3} (penalty={:.2})",
                        a0, a180, tc, penalty
                    );
                    (*tc, cc * penalty, *ac)
                } else {
                    let closer = if t0.abs() <= t180.abs() {
                        (*t0, c0 * 0.70, *a0)
                    } else {
                        (*t180, c180 * 0.70, *a180)
                    };
                    eprintln!(
                        "[straighten] V-groups both low agreement, no combined → {:.3}",
                        closer.0
                    );
                    closer
                }
            } else if q0 > q180 * 1.3 {
                eprintln!(
                    "[straighten] V-groups: near0={:.3}(q={:.3}) wins over near180={:.3}(q={:.3})",
                    t0, q0, t180, q180
                );
                (*t0, *c0, *a0)
            } else if q180 > q0 * 1.3 {
                eprintln!(
                    "[straighten] V-groups: near180={:.3}(q={:.3}) wins over near0={:.3}(q={:.3})",
                    t180, q180, t0, q0
                );
                (*t180, *c180, *a180)
            } else {
                // Similar quality — pick the one with higher agreement ratio
                if a0 >= a180 {
                    eprintln!(
                        "[straighten] V-groups similar: near0={:.3}(a={:.2}) vs near180={:.3}(a={:.2}) → near0",
                        t0, a0, t180, a180
                    );
                    (*t0, *c0, *a0)
                } else {
                    eprintln!(
                        "[straighten] V-groups similar: near0={:.3}(a={:.2}) vs near180={:.3}(a={:.2}) → near180",
                        t0, a0, t180, a180
                    );
                    (*t180, *c180, *a180)
                }
            };

            Some((tilt, conf, agree))
        }
        (Some(v), None) => {
            eprintln!("[straighten] V-group: only near-0 ({:.3})", v.0);
            Some(*v)
        }
        (None, Some(v)) => {
            eprintln!("[straighten] V-group: only near-180 ({:.3})", v.0);
            Some(*v)
        }
        (None, None) => None,
    }
}

// ============================================================================
// V/H Combination
// ============================================================================

/// Minimum agreement ratio for H-lines to participate in V/H combination.
/// H-lines with lower agreement are treated as absent (too noisy to be useful).
const MIN_H_AGREEMENT_RATIO: f64 = 0.40;

/// Combine vertical and horizontal tilt estimates.
///
/// V-lines (walls, door frames) are the primary signal. H-lines (ceiling/floor junctions)
/// serve as a cross-check only, with low weight (15%). H-lines with poor agreement
/// ratios are discarded entirely since they're dominated by furniture/shelf edges.
fn combine_vh_tilts(
    v_tilt: &Option<TiltResult>,
    h_tilt: &Option<TiltResult>,
    total_lines: usize,
) -> StraightenResult {
    // Filter out H-tilt if its agreement ratio is too low (noisy)
    let effective_h_tilt = h_tilt.filter(|(_, _, agree)| *agree >= MIN_H_AGREEMENT_RATIO);

    match (v_tilt, &effective_h_tilt) {
        (Some((v_angle, v_conf, _)), Some((h_angle, h_conf, _))) => {
            let agreement = (*v_angle - *h_angle).abs() < VH_AGREEMENT_THRESHOLD_DEG;

            if agreement {
                // Both agree: V is primary (85%), H is cross-check (15%)
                let angle = v_angle * 0.85 + h_angle * 0.15;
                let confidence = (v_conf * 0.85 + h_conf * 0.15 + 0.10).min(0.95);

                StraightenResult {
                    suggested_rotation: angle,
                    confidence: confidence as f32,
                    lines_used: total_lines,
                    vh_agreement: true,
                }
            } else {
                // Disagree: use the more confident one, with penalty.
                // H-lines need 2x confidence to override V-lines (they're inherently noisier).
                let (angle, confidence) = if *v_conf > *h_conf * 1.2 {
                    (*v_angle, v_conf * 0.80)
                } else if *h_conf > *v_conf * 2.0 {
                    // H-lines only win when dramatically more confident
                    (*h_angle, h_conf * 0.75)
                } else {
                    // Similar confidence but disagree — prefer vertical, heavy penalty
                    (*v_angle, (v_conf * 0.50).min(0.35))
                };

                StraightenResult {
                    suggested_rotation: angle,
                    confidence: confidence as f32,
                    lines_used: total_lines,
                    vh_agreement: false,
                }
            }
        }
        (Some((v_angle, v_conf, _)), None) => StraightenResult {
            suggested_rotation: *v_angle,
            confidence: (*v_conf * 0.95) as f32,
            lines_used: total_lines,
            vh_agreement: false,
        },
        (None, Some((h_angle, h_conf, _))) => StraightenResult {
            suggested_rotation: *h_angle,
            confidence: (*h_conf * 0.80) as f32,
            lines_used: total_lines,
            vh_agreement: false,
        },
        (None, None) => no_correction(),
    }
}

// ============================================================================
// VP Validation
// ============================================================================

/// Validate hough-derived angle using vanishing point estimation.
fn validate_with_real_lines(
    hough_result: &StraightenResult,
    lines: &[HoughLine],
    img_dims: (u32, u32),
) -> StraightenResult {
    if hough_result.confidence < 0.01 || lines.len() < 4 {
        return hough_result.clone();
    }

    let (width, height) = img_dims;

    // Convert HoughLines to ClassifiedLines with real segment coordinates
    let mut vertical_classified: Vec<ClassifiedLine> = Vec::new();
    let mut horizontal_classified: Vec<ClassifiedLine> = Vec::new();

    for hl in lines {
        if let Some(((x1, y1), (x2, y2))) = polar_to_segment(hl.r, hl.hough_angle, width, height) {
            let segment =
                LineSegment::new(f64::from(x1), f64::from(y1), f64::from(x2), f64::from(y2));
            let classified = ClassifiedLine {
                weight: segment.length,
                line_type: hl.line_type,
                segment,
            };
            match hl.line_type {
                LineType::Vertical => vertical_classified.push(classified),
                LineType::Horizontal => horizontal_classified.push(classified),
            }
        }
    }

    if vertical_classified.len() < 2 && horizontal_classified.len() < 2 {
        return hough_result.clone();
    }

    let (vp_angle, vp_confidence) = validate_with_vp(
        hough_result.suggested_rotation,
        hough_result.confidence,
        &vertical_classified,
        &horizontal_classified,
        (width, height),
    );

    StraightenResult {
        suggested_rotation: vp_angle,
        confidence: vp_confidence,
        lines_used: hough_result.lines_used,
        vh_agreement: hough_result.vh_agreement,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Compute adaptive Canny thresholds based on image gradient statistics.
fn compute_canny_thresholds(gray: &GrayImage) -> (f32, f32) {
    let (width, height) = gray.dimensions();
    let mut magnitudes: Vec<f32> = Vec::with_capacity((width * height) as usize);

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let gx = gray.get_pixel(x + 1, y)[0] as f32 - gray.get_pixel(x - 1, y)[0] as f32;
            let gy = gray.get_pixel(x, y + 1)[0] as f32 - gray.get_pixel(x, y - 1)[0] as f32;
            let mag = (gx * gx + gy * gy).sqrt();
            magnitudes.push(mag);
        }
    }

    magnitudes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if magnitudes.is_empty() {
        return (50.0, 150.0);
    }

    let p50 = magnitudes[magnitudes.len() / 2];
    let p85 = magnitudes[(magnitudes.len() as f64 * 0.85) as usize];

    let high = p85.clamp(30.0, 300.0);
    let low = (p50 * 0.8).clamp(10.0, high * 0.5);

    (low, high)
}

/// Convert a Hough polar line (r, angle) to segment endpoints within image bounds.
fn polar_to_segment(
    r: f32,
    angle_deg: u32,
    width: u32,
    height: u32,
) -> Option<((f32, f32), (f32, f32))> {
    let w = width as f32;
    let h = height as f32;

    if angle_deg == 0 {
        return if r >= 0.0 && r <= w {
            Some(((r, 0.0), (r, h)))
        } else {
            None
        };
    }

    if angle_deg == 90 {
        return if r >= 0.0 && r <= h {
            Some(((0.0, r), (w, r)))
        } else {
            None
        };
    }

    let theta = (angle_deg as f32).to_radians();
    let (sin, cos) = theta.sin_cos();

    let mut points: Vec<(f32, f32)> = Vec::with_capacity(4);

    // Intersection with left border (x=0)
    if sin.abs() > 1e-6 {
        let y = r / sin;
        if (0.0..=h).contains(&y) {
            points.push((0.0, y));
        }
    }

    // Intersection with right border (x=w)
    if sin.abs() > 1e-6 {
        let y = (r - w * cos) / sin;
        if (0.0..=h).contains(&y) {
            points.push((w, y));
        }
    }

    // Intersection with top border (y=0)
    if cos.abs() > 1e-6 {
        let x = r / cos;
        if (0.0..=w).contains(&x) {
            points.push((x, 0.0));
        }
    }

    // Intersection with bottom border (y=h)
    if cos.abs() > 1e-6 {
        let x = (r - h * sin) / cos;
        if (0.0..=w).contains(&x) {
            points.push((x, h));
        }
    }

    points.dedup_by(|a, b| (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5);

    if points.len() >= 2 {
        Some((points[0], points[1]))
    } else {
        None
    }
}

/// Downsample a grayscale image to half size.
fn downsample_gray(gray: &GrayImage) -> GrayImage {
    let (width, height) = gray.dimensions();
    let new_w = width / 2;
    let new_h = height / 2;

    let mut out = GrayImage::new(new_w, new_h);
    for y in 0..new_h {
        for x in 0..new_w {
            let sx = x * 2;
            let sy = y * 2;
            let avg = (u16::from(gray.get_pixel(sx, sy)[0])
                + u16::from(gray.get_pixel(sx + 1, sy)[0])
                + u16::from(gray.get_pixel(sx, sy + 1)[0])
                + u16::from(gray.get_pixel(sx + 1, sy + 1)[0]))
                / 4;
            out.put_pixel(x, y, image::Luma([avg as u8]));
        }
    }
    out
}

/// Combine results from two resolutions.
fn combine_multi_resolution(
    full_res: &StraightenResult,
    half_res: &StraightenResult,
) -> StraightenResult {
    let angle_diff = (full_res.suggested_rotation - half_res.suggested_rotation).abs();

    if full_res.confidence < 0.01 && half_res.confidence < 0.01 {
        return no_correction();
    }

    if full_res.confidence < 0.01 {
        return half_res.clone();
    }
    if half_res.confidence < 0.01 {
        return StraightenResult {
            confidence: full_res.confidence * 0.85,
            ..full_res.clone()
        };
    }

    if angle_diff < 0.5 {
        // Strong agreement
        StraightenResult {
            suggested_rotation: full_res.suggested_rotation,
            confidence: (full_res.confidence + 0.10).min(0.95),
            lines_used: full_res.lines_used + half_res.lines_used,
            vh_agreement: full_res.vh_agreement && half_res.vh_agreement,
        }
    } else if angle_diff < 1.5 {
        // Moderate agreement: confidence-weighted blend (prefer full-res)
        let total_conf = full_res.confidence + half_res.confidence;
        let angle = if total_conf > 0.0 {
            (full_res.suggested_rotation * f64::from(full_res.confidence)
                + half_res.suggested_rotation * f64::from(half_res.confidence))
                / f64::from(total_conf)
        } else {
            full_res.suggested_rotation
        };
        StraightenResult {
            suggested_rotation: angle,
            confidence: ((full_res.confidence + half_res.confidence) / 2.0).min(0.85),
            lines_used: full_res.lines_used + half_res.lines_used,
            vh_agreement: full_res.vh_agreement || half_res.vh_agreement,
        }
    } else {
        // Disagreement: use higher-confidence result with penalty
        eprintln!(
            "[straighten] multi-res DISAGREE: {:.3} vs {:.3} (diff={angle_diff:.3})",
            full_res.suggested_rotation, half_res.suggested_rotation
        );
        if full_res.confidence >= half_res.confidence {
            StraightenResult {
                confidence: (full_res.confidence * 0.70).min(0.60),
                ..full_res.clone()
            }
        } else {
            StraightenResult {
                confidence: (half_res.confidence * 0.70).min(0.60),
                ..half_res.clone()
            }
        }
    }
}

/// Apply safety limits to angle and confidence.
fn apply_safety_limits(angle: f64, confidence: f32) -> (f64, f32) {
    let max_rotation = if confidence >= 0.75 {
        MAX_ROTATION_DEG
    } else if confidence >= 0.65 {
        5.0
    } else if confidence >= 0.55 {
        3.0
    } else if confidence >= 0.40 {
        1.5
    } else {
        0.5
    };

    let mut final_angle = angle;
    let mut final_confidence = confidence;

    if angle.abs() > max_rotation {
        final_angle = angle.signum() * max_rotation;
        final_confidence *= 0.8;
    }

    // Gentle penalty for larger angles
    if final_angle.abs() > 5.0 {
        let excess = final_angle.abs() - 5.0;
        final_confidence *= (1.0 - excess as f32 * 0.06).max(0.5);
    }

    if final_angle.abs() < MIN_ROTATION_THRESHOLD_DEG {
        return (0.0, confidence);
    }

    (final_angle, final_confidence.clamp(0.0, 0.95))
}

fn no_correction() -> StraightenResult {
    StraightenResult {
        suggested_rotation: 0.0,
        confidence: 0.0,
        lines_used: 0,
        vh_agreement: false,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnostic test: analyze real images from the test_images folder.
    /// Run with: cargo test test_real_images -- --nocapture --ignored
    #[test]
    #[ignore] // Only run manually
    fn test_real_images() {
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_images");
        if !test_dir.exists() {
            eprintln!("No test_images directory found, skipping");
            return;
        }

        let mut entries: Vec<_> = std::fs::read_dir(&test_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "jpeg" || ext == "jpg" || ext == "png")
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        eprintln!("\n{}", "=".repeat(80));
        eprintln!("REAL IMAGE DIAGNOSTIC: {} images", entries.len());
        eprintln!("{}\n", "=".repeat(80));

        for entry in &entries {
            let path = entry.path();
            let filename = path.file_name().unwrap().to_string_lossy();

            eprintln!("--- {} ---", filename);

            // Load image
            let img = match image::open(&path) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("  FAILED to load: {e}");
                    continue;
                }
            };

            let (w, h) = img.dimensions();
            eprintln!("  Original: {w}x{h}");

            // Downscale to 800px (same as preprocessing)
            let scaled = if w.max(h) > 800 {
                let scale = 800.0 / w.max(h) as f64;
                let nw = (w as f64 * scale).round() as u32;
                let nh = (h as f64 * scale).round() as u32;
                img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
            } else {
                img.clone()
            };
            let gray = scaled.to_luma8();
            let (gw, gh) = gray.dimensions();
            eprintln!("  Scaled: {gw}x{gh}");

            // Run analysis WITHOUT preprocessing (raw grayscale)
            eprintln!("  [RAW - no bilateral/CLAHE]");
            let (raw_result, _) = analyze_at_resolution(&gray);
            eprintln!(
                "  RAW result: angle={:.3}, conf={:.3}, lines={}, vh_agree={}",
                raw_result.suggested_rotation,
                raw_result.confidence,
                raw_result.lines_used,
                raw_result.vh_agreement
            );

            // Run analysis WITH preprocessing (CPU bilateral + CLAHE)
            let processor = crate::gpu::ImageProcessor::Cpu;
            let preprocessed = crate::perspective::preprocessing::preprocess_for_detection_no_exif(
                &scaled, &processor,
            );
            eprintln!("  [PREPROCESSED - bilateral + CLAHE]");
            let (pre_result, _) = analyze_at_resolution(&preprocessed);
            eprintln!(
                "  PRE result: angle={:.3}, conf={:.3}, lines={}, vh_agree={}",
                pre_result.suggested_rotation,
                pre_result.confidence,
                pre_result.lines_used,
                pre_result.vh_agreement
            );

            eprintln!();
        }
    }

    /// Visual verification: apply the detected rotation to selected images and save.
    /// Run with: cargo test test_visual_verify -- --nocapture --ignored
    #[test]
    #[ignore]
    fn test_visual_verify() {
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_images");
        let output_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_images_rotated");

        if !test_dir.exists() {
            eprintln!("No test_images directory, skipping");
            return;
        }

        std::fs::create_dir_all(&output_dir).unwrap();

        // Pick a few representative images
        let filenames = [
            "glifada_01.jpeg",
            "kallithea_02.jpeg",
            "kallithea_04.jpeg",
            "pagkrati_03.jpeg",
            "pagkrati_05.jpeg",
        ];

        for filename in &filenames {
            let path = test_dir.join(filename);
            if !path.exists() {
                eprintln!("  Skipping {} (not found)", filename);
                continue;
            }

            let img = image::open(&path).unwrap();
            let (w, h) = img.dimensions();

            // Downscale for analysis
            let scaled = if w.max(h) > 800 {
                let scale = 800.0 / w.max(h) as f64;
                let nw = (w as f64 * scale).round() as u32;
                let nh = (h as f64 * scale).round() as u32;
                img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
            } else {
                img.clone()
            };
            let gray = scaled.to_luma8();

            let (result, _) = analyze_at_resolution(&gray);
            let angle = result.suggested_rotation;

            eprintln!(
                "  {} -> angle={:.3}, conf={:.3}",
                filename, angle, result.confidence
            );

            if angle.abs() < 0.05 {
                eprintln!("    Angle too small, saving unrotated");
                scaled
                    .save(output_dir.join(format!("ORIG_{}", filename)))
                    .unwrap();
                continue;
            }

            // Rotate the scaled image by the suggested angle
            // Positive angle = CCW rotation to fix CW tilt
            let rad = angle.to_radians();
            let cos = rad.cos();
            let sin = rad.sin();
            let (sw, sh) = scaled.dimensions();
            let cx = sw as f64 / 2.0;
            let cy = sh as f64 / 2.0;

            let mut rotated = image::RgbImage::new(sw, sh);
            let rgb = scaled.to_rgb8();

            for y in 0..sh {
                for x in 0..sw {
                    // Inverse mapping: where does (x,y) come from in the original?
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let src_x = dx * cos + dy * sin + cx;
                    let src_y = -dx * sin + dy * cos + cy;

                    let sx = src_x.round() as i32;
                    let sy = src_y.round() as i32;

                    if sx >= 0 && sx < sw as i32 && sy >= 0 && sy < sh as i32 {
                        rotated.put_pixel(x, y, *rgb.get_pixel(sx as u32, sy as u32));
                    }
                }
            }

            // Save original and rotated side by side
            scaled
                .save(output_dir.join(format!("ORIG_{}", filename)))
                .unwrap();
            rotated
                .save(output_dir.join(format!("ROT_{}", filename)))
                .unwrap();
            eprintln!(
                "    Saved ORIG_{} and ROT_{} (rotated {:.3} deg)",
                filename, filename, angle
            );
        }

        eprintln!("\nVisual verification images saved to: {:?}", output_dir);
    }

    /// Create a synthetic grayscale image with vertical lines tilted by a given angle.
    fn create_tilted_line_image(width: u32, height: u32, tilt_degrees: f64) -> GrayImage {
        let mut img = GrayImage::from_pixel(width, height, image::Luma([128u8]));
        let tilt_rad = tilt_degrees.to_radians();

        // Draw bright vertical lines tilted by the given angle
        let line_positions = [0.2, 0.35, 0.5, 0.65, 0.8];
        for &frac in &line_positions {
            let center_x = (width as f64 * frac) as i32;
            for y in 0..height as i32 {
                let y_offset = y as f64 - height as f64 / 2.0;
                let x_shift = (y_offset * tilt_rad.tan()) as i32;
                for dx in -1..=1 {
                    let x = center_x + x_shift + dx;
                    if x >= 0 && x < width as i32 {
                        img.put_pixel(x as u32, y as u32, image::Luma([240u8]));
                    }
                }
            }
        }

        // Draw horizontal lines
        let h_positions = [0.25, 0.75];
        for &frac in &h_positions {
            let center_y = (height as f64 * frac) as i32;
            for x in 0..width as i32 {
                let x_offset = x as f64 - width as f64 / 2.0;
                let y_shift = (x_offset * tilt_rad.tan()) as i32;
                for dy in -1..=1 {
                    let y = center_y + y_shift + dy;
                    if y >= 0 && y < height as i32 {
                        img.put_pixel(x as u32, y as u32, image::Luma([240u8]));
                    }
                }
            }
        }

        img
    }

    #[test]
    fn test_full_pipeline_tilted_3_degrees() {
        // create_tilted_line_image(+3.0) draws lines leaning CCW (left at top).
        // This represents camera tilted CW → fix by rotating image CCW → positive.
        // Lines appear at θ ≈ 177° (near-180 range), tilt ≈ +3.0.
        let tilt = 3.0_f64;
        let gray = create_tilted_line_image(800, 600, tilt);

        let (result, _) = analyze_at_resolution(&gray);
        eprintln!(
            "test 3deg: rotation={:.3}, confidence={:.3}, lines={}",
            result.suggested_rotation, result.confidence, result.lines_used
        );

        assert!(
            result.suggested_rotation.abs() > 0.5,
            "Expected non-zero rotation for {tilt} degree tilt, got {:.3}",
            result.suggested_rotation
        );
        assert!(
            (result.suggested_rotation - tilt).abs() < 2.0,
            "Expected rotation near +{tilt}, got {:.3}",
            result.suggested_rotation
        );
    }

    #[test]
    fn test_full_pipeline_tilted_1_degree() {
        let tilt = 1.0_f64;
        let gray = create_tilted_line_image(800, 600, tilt);
        let (result, _) = analyze_at_resolution(&gray);

        eprintln!(
            "test 1deg: rotation={:.3}, confidence={:.3}",
            result.suggested_rotation, result.confidence
        );

        assert!(
            (result.suggested_rotation - tilt).abs() < 1.5,
            "Expected rotation near +{tilt}, got {:.3}",
            result.suggested_rotation
        );
    }

    #[test]
    fn test_full_pipeline_tilted_5_degrees() {
        let tilt = 5.0_f64;
        let gray = create_tilted_line_image(800, 600, tilt);
        let (result, _) = analyze_at_resolution(&gray);

        eprintln!(
            "test 5deg: rotation={:.3}, confidence={:.3}",
            result.suggested_rotation, result.confidence
        );

        // Direction should be correct (positive), magnitude > 1.0
        assert!(
            result.suggested_rotation > 1.0,
            "Expected positive rotation > 1.0 for {tilt} degree tilt, got {:.3}",
            result.suggested_rotation
        );
    }

    #[test]
    fn test_full_pipeline_level_image() {
        let gray = create_tilted_line_image(800, 600, 0.0);
        let (result, _) = analyze_at_resolution(&gray);

        eprintln!(
            "test 0deg: rotation={:.3}, confidence={:.3}",
            result.suggested_rotation, result.confidence
        );

        assert!(
            result.suggested_rotation.abs() < 1.0,
            "Expected near-zero rotation for level image, got {:.3}",
            result.suggested_rotation
        );
    }

    #[test]
    fn test_negative_tilt() {
        // create_tilted_line_image(-2.0) draws lines leaning CW (right at top).
        // Camera tilted CCW → fix by rotating image CW → negative.
        // Lines appear at θ ≈ 2° (near-0 range), tilt ≈ -2.0.
        let tilt = -2.0_f64;
        let gray = create_tilted_line_image(800, 600, tilt);
        let (result, _) = analyze_at_resolution(&gray);

        eprintln!(
            "test -2deg: rotation={:.3}, confidence={:.3}",
            result.suggested_rotation, result.confidence
        );

        assert!(
            (result.suggested_rotation - tilt).abs() < 2.0,
            "Expected rotation near {tilt}, got {:.3}",
            result.suggested_rotation
        );
    }

    #[test]
    fn test_safety_limits() {
        let (angle, _) = apply_safety_limits(8.0, 0.75);
        assert!((angle - 8.0).abs() < 0.1);

        let (angle, conf) = apply_safety_limits(5.0, 0.20);
        assert!(angle.abs() <= 1.0);
        assert!(conf < 0.20);

        let (angle, _) = apply_safety_limits(5.0, 0.50);
        assert!(angle.abs() <= 6.0);
    }

    #[test]
    fn test_multi_resolution_agreement() {
        let full = StraightenResult {
            suggested_rotation: 1.5,
            confidence: 0.7,
            lines_used: 5,
            vh_agreement: true,
        };
        let half = StraightenResult {
            suggested_rotation: 1.4,
            confidence: 0.6,
            lines_used: 3,
            vh_agreement: true,
        };

        let result = combine_multi_resolution(&full, &half);
        assert!((result.suggested_rotation - 1.5).abs() < 0.1);
        assert!(result.confidence > full.confidence);
    }

    /// Ground-truth test: take real images, rotate them by known amounts,
    /// and check if the algorithm detects the correct tilt.
    /// This is the REAL accuracy test — synthetic geometric lines are too easy.
    /// Run with: cargo test test_ground_truth -- --nocapture --ignored
    #[test]
    #[ignore]
    fn test_ground_truth() {
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_images");
        if !test_dir.exists() {
            eprintln!("No test_images directory found, skipping");
            return;
        }

        // Use a few representative images
        let filenames = [
            "glifada_01.jpeg",
            "glifada_03.jpeg",
            "kallithea_02.jpeg",
            "kallithea_04.jpeg",
            "pagkrati_03.jpeg",
            "pagkrati_05.jpeg",
        ];

        // Test tilts: these are common real-world tilts
        let test_tilts = [0.5_f64, 1.0, 1.5, 2.0, -1.0, -2.0];

        eprintln!("\n{}", "=".repeat(90));
        eprintln!("GROUND TRUTH TEST: rotate real images by known amounts");
        eprintln!("{}\n", "=".repeat(90));

        let mut total_tests = 0;
        let mut total_error = 0.0_f64;
        let mut worst_error = 0.0_f64;
        let mut worst_case = String::new();
        let mut near_zero_count = 0; // count how often it returns ~0 (under-detection)

        for filename in &filenames {
            let path = test_dir.join(filename);
            if !path.exists() {
                eprintln!("  Skipping {} (not found)", filename);
                continue;
            }

            let img = image::open(&path).unwrap();
            let (w, h) = img.dimensions();

            // Downscale to analysis size
            let scaled = if w.max(h) > 800 {
                let scale = 800.0 / w.max(h) as f64;
                let nw = (w as f64 * scale).round() as u32;
                let nh = (h as f64 * scale).round() as u32;
                img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
            } else {
                img.clone()
            };
            let (sw, sh) = scaled.dimensions();

            // First, get the baseline (unrotated) detection
            let gray_orig = scaled.to_luma8();
            let (baseline, _) = analyze_at_resolution(&gray_orig);

            eprintln!(
                "--- {} ({}x{}) baseline={:.3}° ---",
                filename, sw, sh, baseline.suggested_rotation
            );

            for &applied_tilt in &test_tilts {
                // Rotate the image by applied_tilt degrees
                let rad = applied_tilt.to_radians();
                let cos_a = rad.cos();
                let sin_a = rad.sin();
                let cx = sw as f64 / 2.0;
                let cy = sh as f64 / 2.0;

                let rgb = scaled.to_rgb8();
                let mut rotated_rgb = image::RgbImage::new(sw, sh);
                for y in 0..sh {
                    for x in 0..sw {
                        let dx = x as f64 - cx;
                        let dy = y as f64 - cy;
                        // Inverse mapping
                        let src_x = dx * cos_a + dy * sin_a + cx;
                        let src_y = -dx * sin_a + dy * cos_a + cy;
                        let sx = src_x.round() as i32;
                        let sy = src_y.round() as i32;
                        if sx >= 0 && sx < sw as i32 && sy >= 0 && sy < sh as i32 {
                            rotated_rgb.put_pixel(x, y, *rgb.get_pixel(sx as u32, sy as u32));
                        }
                    }
                }

                let rotated_gray = image::DynamicImage::ImageRgb8(rotated_rgb).to_luma8();
                let (result, _) = analyze_at_resolution(&rotated_gray);

                // The rotation applies a CCW rotation of `applied_tilt` degrees.
                // Positive applied_tilt = CCW rotation → walls lean right → need CW correction.
                // So the expected suggested_rotation = -applied_tilt (to undo the rotation).
                // We subtract baseline because the original image may already have some tilt.
                let expected = -applied_tilt;
                let detected = result.suggested_rotation;
                // The net detected correction relative to the original baseline
                let net_detected = detected - baseline.suggested_rotation;
                let error = (net_detected - expected).abs();

                total_tests += 1;
                total_error += error;
                if error > worst_error {
                    worst_error = error;
                    worst_case = format!("{} @ {:.1}°", filename, applied_tilt);
                }
                if detected.abs() < 0.15 {
                    near_zero_count += 1;
                }

                let status = if error < 0.3 {
                    "OK"
                } else if error < 0.7 {
                    "WARN"
                } else {
                    "FAIL"
                };

                eprintln!(
                    "  applied={:+.1}° detected={:+.3}° net={:+.3}° error={:.3}° conf={:.3} [{}]",
                    applied_tilt, detected, net_detected, error, result.confidence, status
                );
            }
            eprintln!();
        }

        let avg_error = total_error / total_tests as f64;
        eprintln!("=== SUMMARY ===");
        eprintln!("  Tests: {}", total_tests);
        eprintln!("  Average error: {:.3}°", avg_error);
        eprintln!("  Worst error: {:.3}° ({})", worst_error, worst_case);
        eprintln!(
            "  Under-detection (|detected| < 0.15°): {}/{} ({:.0}%)",
            near_zero_count,
            total_tests,
            near_zero_count as f64 / total_tests as f64 * 100.0
        );
    }

    /// Detailed diagnostic: show every detected line's angle and tilt for a single image.
    /// Helps understand WHY the algorithm under-detects.
    /// Run with: cargo test test_line_diagnostic -- --nocapture --ignored
    #[test]
    #[ignore]
    fn test_line_diagnostic() {
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_images");
        if !test_dir.exists() {
            eprintln!("No test_images directory found, skipping");
            return;
        }

        let filename = "kallithea_04.jpeg"; // pick one representative image
        let path = test_dir.join(filename);
        if !path.exists() {
            eprintln!("  {} not found", filename);
            return;
        }

        let img = image::open(&path).unwrap();
        let (w, h) = img.dimensions();
        let scaled = if w.max(h) > 800 {
            let scale = 800.0 / w.max(h) as f64;
            let nw = (w as f64 * scale).round() as u32;
            let nh = (h as f64 * scale).round() as u32;
            img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
        } else {
            img.clone()
        };
        let gray = scaled.to_luma8();

        // Also test with a 1.5° rotation applied
        let (sw, sh) = scaled.dimensions();
        let applied_tilt = 1.5_f64;
        let rad = applied_tilt.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();
        let cx = sw as f64 / 2.0;
        let cy = sh as f64 / 2.0;
        let rgb = scaled.to_rgb8();
        let mut rotated_rgb = image::RgbImage::new(sw, sh);
        for y in 0..sh {
            for x in 0..sw {
                let dx = x as f64 - cx;
                let dy = y as f64 - cy;
                let src_x = dx * cos_a + dy * sin_a + cx;
                let src_y = -dx * sin_a + dy * cos_a + cy;
                let sx = src_x.round() as i32;
                let sy = src_y.round() as i32;
                if sx >= 0 && sx < sw as i32 && sy >= 0 && sy < sh as i32 {
                    rotated_rgb.put_pixel(x, y, *rgb.get_pixel(sx as u32, sy as u32));
                }
            }
        }
        let gray_rotated = image::DynamicImage::ImageRgb8(rotated_rgb).to_luma8();

        for (label, g) in [("ORIGINAL", &gray), ("ROTATED +1.5°", &gray_rotated)] {
            eprintln!("\n{}", "=".repeat(80));
            eprintln!("{} - {}", label, filename);
            eprintln!("{}", "=".repeat(80));

            let lines = detect_hough_lines(g);
            eprintln!("Total lines detected: {}", lines.len());

            eprintln!("\nV-lines (near vertical):");
            eprintln!(
                "  {:>6} {:>8} {:>10} {:>10} {:>6}",
                "angle", "tilt_deg", "tilt_prec", "r", "votes"
            );
            for l in &lines {
                if l.line_type == LineType::Vertical {
                    eprintln!(
                        "  {:>6} {:>+8.2} {:>+10.4} {:>10.1} {:>6}",
                        l.hough_angle, l.tilt_deg, l.tilt_precise, l.r, l.votes
                    );
                }
            }

            eprintln!("\nH-lines (near horizontal):");
            eprintln!(
                "  {:>6} {:>8} {:>10} {:>10} {:>6}",
                "angle", "tilt_deg", "tilt_prec", "r", "votes"
            );
            for l in &lines {
                if l.line_type == LineType::Horizontal {
                    eprintln!(
                        "  {:>6} {:>+8.2} {:>+10.4} {:>10.1} {:>6}",
                        l.hough_angle, l.tilt_deg, l.tilt_precise, l.r, l.votes
                    );
                }
            }

            let (result, _) = analyze_at_resolution(g);
            eprintln!(
                "\nResult: angle={:.3}°, confidence={:.3}, lines={}, vh_agree={}",
                result.suggested_rotation,
                result.confidence,
                result.lines_used,
                result.vh_agreement
            );
        }
    }

    #[test]
    fn test_multi_resolution_disagreement() {
        let full = StraightenResult {
            suggested_rotation: 3.0,
            confidence: 0.6,
            lines_used: 5,
            vh_agreement: true,
        };
        let half = StraightenResult {
            suggested_rotation: 0.5,
            confidence: 0.5,
            lines_used: 3,
            vh_agreement: true,
        };

        let result = combine_multi_resolution(&full, &half);
        assert!(result.confidence < full.confidence);
    }
}
