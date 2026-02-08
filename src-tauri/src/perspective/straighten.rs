//! Main auto-straighten module using LSD + Line Classification + RANSAC.
//!
//! This module provides the primary entry point for image straightening,
//! replacing the old 4-method ensemble approach with a modern pipeline:
//!
//! 1. Preprocessing (bilateral filter, CLAHE, lens distortion)
//! 2. LSD line segment detection
//! 3. Line classification (Vertical/Horizontal, Border/Structural/Interior)
//! 4. Separate V/H RANSAC analysis
//! 5. V/H combination with agreement bonus
//! 6. Vanishing point validation (optional)
//! 7. Spatial diversity scoring
//! 8. Final confidence computation

use crate::perspective::preprocessing::{preprocess_for_detection, preprocess_for_detection_no_exif};
use crate::perspective::vanishing::validate_with_vp;
use image::{DynamicImage, GenericImageView, GrayImage};
use opencv::core::{Mat, Scalar, CV_8UC1};
use opencv::imgproc;
use opencv::prelude::{LineSegmentDetectorTrait, MatTrait, MatTraitConst};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Constants
// ============================================================================

/// Minimum line length as ratio of image dimension (5% = keep more lines than before)
const MIN_LINE_LENGTH_RATIO: f64 = 0.05;

/// Border zone - outer 15% of image
const BORDER_ZONE_RATIO: f64 = 0.15;

/// Structural line threshold - spans at least 30% of image dimension
const STRUCTURAL_SPAN_RATIO: f64 = 0.30;

/// Lines within ±20° of vertical are classified as vertical
const VERTICAL_ANGLE_THRESHOLD_DEG: f64 = 20.0;

/// Lines within ±20° of horizontal are classified as horizontal
const HORIZONTAL_ANGLE_THRESHOLD_DEG: f64 = 20.0;

/// RANSAC iterations
const RANSAC_ITERATIONS: usize = 500;

/// RANSAC inlier threshold in degrees
const RANSAC_INLIER_THRESHOLD_DEG: f64 = 1.5;

/// V/H agreement threshold in degrees
const VH_AGREEMENT_THRESHOLD_DEG: f64 = 1.0;

/// Minimum lines required for analysis
const MIN_LINES_REQUIRED: usize = 3;

/// Maximum rotation to apply (degrees)
const MAX_ROTATION_DEG: f64 = 6.0;

/// Minimum rotation to bother applying (degrees)
const MIN_ROTATION_THRESHOLD_DEG: f64 = 0.1;

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

/// A raw line segment from LSD
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

        // Normalize direction so we measure consistently
        let (norm_dx, norm_dy) = if dy >= 0.0 { (dx, dy) } else { (-dx, -dy) };

        // Angle from vertical: atan2(dx, dy) - 0 means vertical
        let angle_from_vertical = norm_dx.atan2(norm_dy).to_degrees();

        // Angle from horizontal: atan2(dy, dx) - 0 means horizontal
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

    /// Get the midpoint of this line segment
    pub fn midpoint(&self) -> (f64, f64) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }
}

/// Type of line based on angle
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineType {
    Vertical,
    Horizontal,
    Diagonal,
}

/// Position classification of a line
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionType {
    /// Line is in outer 15% of image (likely a wall)
    Border,
    /// Line spans >30% of image dimension
    Structural,
    /// Everything else (furniture, decorations)
    Interior,
}

/// A classified line with weight
#[derive(Debug, Clone)]
pub struct ClassifiedLine {
    pub segment: LineSegment,
    pub line_type: LineType,
    pub position: PositionType,
    pub weight: f64,
}

/// RANSAC analysis result
#[derive(Debug, Clone)]
pub struct RansacResult {
    pub angle: f64,
    pub confidence: f32,
    pub inlier_count: usize,
    pub inlier_weight: f64,
    pub total_weight: f64,
    pub angle_stddev: f64,
}

// ============================================================================
// Main Entry Points
// ============================================================================

/// Analyze image for straightening with EXIF support for lens correction.
///
/// This is the main entry point for auto-straightening.
pub fn analyze_straighten(img: &DynamicImage, image_path: Option<&Path>) -> StraightenResult {
    // 1. Preprocess the image
    let gray = preprocess_for_detection(img, image_path);

    analyze_straighten_from_gray(&gray, img.dimensions())
}

/// Analyze image for straightening without EXIF (for preview images).
pub fn analyze_straighten_no_exif(img: &DynamicImage) -> StraightenResult {
    let gray = preprocess_for_detection_no_exif(img);
    analyze_straighten_from_gray(&gray, img.dimensions())
}

/// Core analysis on preprocessed grayscale image.
fn analyze_straighten_from_gray(gray: &GrayImage, original_dims: (u32, u32)) -> StraightenResult {
    let (width, height) = gray.dimensions();
    // Verify preprocessing produced a valid image (not uniform)
    let (min_val, max_val) = gray.pixels().fold((255u8, 0u8), |(min, max), p| {
        (min.min(p[0]), max.max(p[0]))
    });
    eprintln!(
        "[straighten] gray image: {width}x{height}, original: {}x{}, pixel range: {min_val}..{max_val}",
        original_dims.0, original_dims.1
    );

    if max_val - min_val < 10 {
        eprintln!("[straighten] WARNING: near-uniform image (range={}), LSD will find no lines", max_val - min_val);
    }

    // 2. Detect line segments using LSD
    let all_lines = match detect_line_segments_lsd(gray) {
        Ok(lines) => {
            eprintln!("[straighten] LSD detected {} lines (after length filter)", lines.len());
            lines
        }
        Err(e) => {
            eprintln!("[straighten] LSD detection FAILED: {e}");
            return no_correction();
        }
    };

    if all_lines.len() < MIN_LINES_REQUIRED {
        eprintln!("[straighten] too few lines ({} < {}), returning no correction", all_lines.len(), MIN_LINES_REQUIRED);
        return no_correction();
    }

    // 3. Classify lines
    let classified = classify_lines(all_lines, (width, height));
    eprintln!("[straighten] classified: {} lines (V={}, H={}, dropped diagonal)",
        classified.len(),
        classified.iter().filter(|l| l.line_type == LineType::Vertical).count(),
        classified.iter().filter(|l| l.line_type == LineType::Horizontal).count(),
    );

    // 4. Separate into vertical and horizontal lines
    let vertical_lines: Vec<_> = classified
        .iter()
        .filter(|l| l.line_type == LineType::Vertical)
        .cloned()
        .collect();

    let horizontal_lines: Vec<_> = classified
        .iter()
        .filter(|l| l.line_type == LineType::Horizontal)
        .cloned()
        .collect();

    // 5. Run RANSAC on vertical and horizontal separately
    let v_result = analyze_lines_ransac(&vertical_lines);
    let h_result = analyze_lines_ransac(&horizontal_lines);

    // 6. Combine V/H results
    let (angle, base_confidence, vh_agreement) = combine_vh_results(&v_result, &h_result);

    // 7. Vanishing point validation (skip if already very confident with small angle)
    let (vp_validated_angle, vp_confidence) = if base_confidence < 0.80 || angle.abs() > 2.0 {
        // Use VP validation for extra confidence
        let combined_result = RansacResult {
            angle,
            confidence: base_confidence,
            inlier_count: v_result.inlier_count + h_result.inlier_count,
            inlier_weight: v_result.inlier_weight + h_result.inlier_weight,
            total_weight: v_result.total_weight + h_result.total_weight,
            angle_stddev: (v_result.angle_stddev + h_result.angle_stddev) / 2.0,
        };
        validate_with_vp(&combined_result, &vertical_lines, &horizontal_lines, (width, height))
    } else {
        // Skip VP for already high-confidence small corrections
        (angle, base_confidence)
    };

    // 8. Compute spatial diversity
    let all_inliers: Vec<_> = classified
        .iter()
        .filter(|l| {
            let angle_diff = match l.line_type {
                LineType::Vertical => (l.segment.angle_from_vertical - vp_validated_angle).abs(),
                LineType::Horizontal => (l.segment.angle_from_horizontal - vp_validated_angle).abs(),
                LineType::Diagonal => 999.0,
            };
            angle_diff < RANSAC_INLIER_THRESHOLD_DEG
        })
        .collect();

    let diversity = spatial_diversity_score(&all_inliers, (width, height));

    // 9. Compute final confidence
    let lines_used = v_result.inlier_count + h_result.inlier_count;
    let confidence = compute_final_confidence(
        vp_confidence,
        vh_agreement,
        diversity,
        vp_validated_angle,
        lines_used,
    );

    // 10. Apply safety limits
    let (final_angle, final_confidence) = apply_safety_limits(vp_validated_angle, confidence);

    StraightenResult {
        suggested_rotation: final_angle,
        confidence: final_confidence,
        lines_used,
        vh_agreement,
    }
}

// ============================================================================
// LSD Detection
// ============================================================================

/// Detect line segments using OpenCV's LSD
fn detect_line_segments_lsd(gray: &GrayImage) -> Result<Vec<LineSegment>, String> {
    let (width, height) = gray.dimensions();
    let min_length = f64::from(height.min(width)) * MIN_LINE_LENGTH_RATIO;

    // Convert to OpenCV Mat
    let mat = gray_image_to_mat(gray)?;

    // Create LSD detector
    let mut lsd = imgproc::create_line_segment_detector_def()
        .map_err(|e| format!("Failed to create LSD: {e}"))?;

    // Detect lines
    let mut lines = Mat::default();
    let mut width_out = Mat::default();
    let mut prec_out = Mat::default();
    let mut nfa_out = Mat::default();

    lsd.detect(&mat, &mut lines, &mut width_out, &mut prec_out, &mut nfa_out)
        .map_err(|e| format!("LSD failed: {e}"))?;

    // Convert to LineSegment structs
    let mut segments = Vec::new();
    let num_lines = lines.rows();
    eprintln!("[straighten/lsd] raw output: {num_lines} lines, mat: {}x{} type={}", lines.rows(), lines.cols(), lines.typ());

    for i in 0..num_lines {
        let line: &opencv::core::Vec4f = lines
            .at(i)
            .map_err(|e| format!("Failed to get line {i}: {e}"))?;

        let segment = LineSegment::new(
            f64::from(line[0]),
            f64::from(line[1]),
            f64::from(line[2]),
            f64::from(line[3]),
        );

        // Filter by minimum length
        if segment.length >= min_length {
            segments.push(segment);
        }
    }

    Ok(segments)
}

/// Convert GrayImage to OpenCV Mat
fn gray_image_to_mat(gray: &GrayImage) -> Result<Mat, String> {
    let (width, height) = gray.dimensions();
    let raw_data = gray.as_raw();

    // Build row slices and use from_slice_2d for robust bulk copy
    let rows: Vec<&[u8]> = (0..height as usize)
        .map(|y| {
            let start = y * width as usize;
            &raw_data[start..start + width as usize]
        })
        .collect();

    let mat = Mat::from_slice_2d(&rows)
        .map_err(|e| format!("Failed to create Mat from image data: {e}"))?;

    eprintln!(
        "[straighten/mat] created: {}x{}, type={}, continuous={}",
        mat.cols(),
        mat.rows(),
        mat.typ(),
        mat.is_continuous()
    );

    Ok(mat)
}

// ============================================================================
// Line Classification
// ============================================================================

/// Classify all line segments by type and position
pub fn classify_lines(lines: Vec<LineSegment>, img_dims: (u32, u32)) -> Vec<ClassifiedLine> {
    let (width, height) = img_dims;
    let border_x = f64::from(width) * BORDER_ZONE_RATIO;
    let border_y = f64::from(height) * BORDER_ZONE_RATIO;
    let structural_threshold_x = f64::from(width) * STRUCTURAL_SPAN_RATIO;
    let structural_threshold_y = f64::from(height) * STRUCTURAL_SPAN_RATIO;

    lines
        .into_iter()
        .filter_map(|segment| {
            // Classify by angle
            let line_type = classify_line_type(&segment);

            // Skip diagonal lines entirely
            if line_type == LineType::Diagonal {
                return None;
            }

            // Classify by position
            let position = classify_position(
                &segment,
                (f64::from(width), f64::from(height)),
                border_x,
                border_y,
                structural_threshold_x,
                structural_threshold_y,
            );

            // Compute weight based on type, position, and length
            let weight = compute_line_weight(&segment, line_type, position);

            Some(ClassifiedLine {
                segment,
                line_type,
                position,
                weight,
            })
        })
        .collect()
}

/// Classify a line as Vertical, Horizontal, or Diagonal based on its angle
fn classify_line_type(segment: &LineSegment) -> LineType {
    let v_angle = segment.angle_from_vertical.abs();
    let h_angle = segment.angle_from_horizontal.abs();

    if v_angle <= VERTICAL_ANGLE_THRESHOLD_DEG {
        LineType::Vertical
    } else if h_angle <= HORIZONTAL_ANGLE_THRESHOLD_DEG {
        LineType::Horizontal
    } else {
        LineType::Diagonal
    }
}

/// Classify line position (Border, Structural, Interior)
fn classify_position(
    segment: &LineSegment,
    img_dims: (f64, f64),
    border_x: f64,
    border_y: f64,
    structural_x: f64,
    structural_y: f64,
) -> PositionType {
    let (mid_x, mid_y) = segment.midpoint();
    let (width, height) = img_dims;

    // Check if in border zone
    let in_left_border = mid_x < border_x;
    let in_right_border = mid_x > width - border_x;
    let in_top_border = mid_y < border_y;
    let in_bottom_border = mid_y > height - border_y;

    if in_left_border || in_right_border || in_top_border || in_bottom_border {
        return PositionType::Border;
    }

    // Check if structural (spans significant portion)
    let x_span = (segment.x2 - segment.x1).abs();
    let y_span = (segment.y2 - segment.y1).abs();

    if x_span >= structural_x || y_span >= structural_y {
        return PositionType::Structural;
    }

    PositionType::Interior
}

/// Compute weight for a classified line
fn compute_line_weight(segment: &LineSegment, line_type: LineType, position: PositionType) -> f64 {
    // Base weight from position
    let position_weight = match position {
        PositionType::Border => 3.0,
        PositionType::Structural => 2.5,
        PositionType::Interior => 1.0,
    };

    // Type bonus (vertical walls are most reliable)
    let type_weight = match line_type {
        LineType::Vertical => 1.5,
        LineType::Horizontal => 1.0,
        LineType::Diagonal => 0.0, // Should never happen, filtered out
    };

    // Length factor (longer = more reliable)
    let length_factor = (segment.length / 100.0).min(2.0).max(0.5);

    position_weight * type_weight * length_factor
}

// ============================================================================
// RANSAC Analysis
// ============================================================================

/// Run RANSAC analysis on a set of classified lines
fn analyze_lines_ransac(lines: &[ClassifiedLine]) -> RansacResult {
    if lines.is_empty() {
        return RansacResult {
            angle: 0.0,
            confidence: 0.0,
            inlier_count: 0,
            inlier_weight: 0.0,
            total_weight: 0.0,
            angle_stddev: 0.0,
        };
    }

    if lines.len() == 1 {
        let angle = match lines[0].line_type {
            LineType::Vertical => lines[0].segment.angle_from_vertical,
            LineType::Horizontal => lines[0].segment.angle_from_horizontal,
            LineType::Diagonal => 0.0,
        };
        return RansacResult {
            angle,
            confidence: 0.3, // Low confidence for single line
            inlier_count: 1,
            inlier_weight: lines[0].weight,
            total_weight: lines[0].weight,
            angle_stddev: 0.0,
        };
    }

    let mut rng = rand::thread_rng();
    let inlier_threshold = RANSAC_INLIER_THRESHOLD_DEG;

    // Compute total weight for weighted sampling
    let total_weight: f64 = lines.iter().map(|l| l.weight * l.segment.length).sum();

    let mut best_angle = 0.0;
    let mut best_inlier_weight = 0.0;
    let mut best_inlier_count = 0;

    for _ in 0..RANSAC_ITERATIONS {
        // Weighted random sample
        let sample = weighted_random_sample(lines, total_weight, &mut rng);

        let hypothesis_angle = match sample.line_type {
            LineType::Vertical => sample.segment.angle_from_vertical,
            LineType::Horizontal => sample.segment.angle_from_horizontal,
            LineType::Diagonal => continue,
        };

        // Count inliers
        let mut inlier_weight = 0.0;
        let mut inlier_count = 0;

        for line in lines {
            let line_angle = match line.line_type {
                LineType::Vertical => line.segment.angle_from_vertical,
                LineType::Horizontal => line.segment.angle_from_horizontal,
                LineType::Diagonal => continue,
            };

            if (line_angle - hypothesis_angle).abs() < inlier_threshold {
                inlier_weight += line.weight * line.segment.length;
                inlier_count += 1;
            }
        }

        if inlier_weight > best_inlier_weight {
            best_inlier_weight = inlier_weight;
            best_angle = hypothesis_angle;
            best_inlier_count = inlier_count;
        }

        // Early exit if we have strong consensus
        if inlier_weight > total_weight * 0.8 {
            break;
        }
    }

    // Refine angle with weighted average of inliers
    let (refined_angle, angle_stddev) =
        refine_angle_with_inliers(lines, best_angle, inlier_threshold);

    // Compute confidence
    let confidence = if total_weight > 0.0 {
        (best_inlier_weight / total_weight) as f32
    } else {
        0.0
    };

    RansacResult {
        angle: refined_angle,
        confidence,
        inlier_count: best_inlier_count,
        inlier_weight: best_inlier_weight,
        total_weight,
        angle_stddev,
    }
}

/// Weighted random sample - probability proportional to weight * length
fn weighted_random_sample<'a>(
    lines: &'a [ClassifiedLine],
    total_weight: f64,
    rng: &mut impl Rng,
) -> &'a ClassifiedLine {
    let target = rng.gen_range(0.0..total_weight);
    let mut cumulative = 0.0;

    for line in lines {
        cumulative += line.weight * line.segment.length;
        if cumulative >= target {
            return line;
        }
    }

    // Fallback to last line
    lines.last().expect("lines should not be empty")
}

/// Refine angle using weighted average of inliers, return stddev
fn refine_angle_with_inliers(
    lines: &[ClassifiedLine],
    best_angle: f64,
    threshold: f64,
) -> (f64, f64) {
    let mut weight_sum = 0.0;
    let mut angle_sum = 0.0;
    let mut angles_weights: Vec<(f64, f64)> = Vec::new();

    for line in lines {
        let line_angle = match line.line_type {
            LineType::Vertical => line.segment.angle_from_vertical,
            LineType::Horizontal => line.segment.angle_from_horizontal,
            LineType::Diagonal => continue,
        };

        if (line_angle - best_angle).abs() < threshold {
            let w = line.weight * line.segment.length;
            angle_sum += line_angle * w;
            weight_sum += w;
            angles_weights.push((line_angle, w));
        }
    }

    let refined = if weight_sum > 0.0 {
        angle_sum / weight_sum
    } else {
        best_angle
    };

    // Compute weighted standard deviation
    let variance = if weight_sum > 0.0 && angles_weights.len() > 1 {
        angles_weights
            .iter()
            .map(|(a, w)| {
                let diff = a - refined;
                diff * diff * w
            })
            .sum::<f64>()
            / weight_sum
    } else {
        0.0
    };

    (refined, variance.sqrt())
}

// ============================================================================
// V/H Combination
// ============================================================================

/// Combine vertical and horizontal RANSAC results
fn combine_vh_results(v: &RansacResult, h: &RansacResult) -> (f64, f32, bool) {
    let v_valid = v.inlier_count >= 2 && v.confidence > 0.2;
    let h_valid = h.inlier_count >= 2 && h.confidence > 0.2;

    // Both valid - check agreement
    if v_valid && h_valid {
        let agreement = (v.angle - h.angle).abs() < VH_AGREEMENT_THRESHOLD_DEG;

        if agreement {
            // Weighted combination favoring vertical (walls more reliable)
            let angle = v.angle * 0.65 + h.angle * 0.35;
            let confidence = ((v.confidence + h.confidence) / 2.0 + 0.10).min(0.85);
            return (angle, confidence, true);
        } else {
            // Disagreement - use more confident one with penalty
            if v.confidence > h.confidence * 1.3 {
                return (v.angle, v.confidence * 0.8, false);
            } else if h.confidence > v.confidence * 1.3 {
                return (h.angle, h.confidence * 0.8, false);
            } else {
                // Similar confidence but disagreement - very uncertain
                return (v.angle, (v.confidence * 0.5).min(0.25), false);
            }
        }
    }

    // Only vertical valid
    if v_valid {
        return (v.angle, v.confidence * 0.9, false);
    }

    // Only horizontal valid
    if h_valid {
        return (h.angle, h.confidence * 0.85, false);
    }

    // Neither valid
    (0.0, 0.0, false)
}

// ============================================================================
// Confidence Computation
// ============================================================================

/// Compute spatial diversity score (0-1)
fn spatial_diversity_score(inliers: &[&ClassifiedLine], img_dims: (u32, u32)) -> f64 {
    if inliers.is_empty() {
        return 0.0;
    }

    let (width, height) = img_dims;
    let cell_width = f64::from(width) / 4.0;
    let cell_height = f64::from(height) / 4.0;

    // 4x4 grid
    let mut grid = [[0.0f64; 4]; 4];
    let mut total_weight = 0.0;

    for line in inliers {
        let (mid_x, mid_y) = line.segment.midpoint();
        let gx = ((mid_x / cell_width) as usize).min(3);
        let gy = ((mid_y / cell_height) as usize).min(3);
        grid[gy][gx] += line.weight;
        total_weight += line.weight;
    }

    if total_weight == 0.0 {
        return 0.0;
    }

    // Count cells with significant contribution (>5% of total)
    let threshold = total_weight * 0.05;
    let active_cells = grid
        .iter()
        .flatten()
        .filter(|&&w| w > threshold)
        .count();

    // Score = sqrt(active_cells / 16)
    (active_cells as f64 / 16.0).sqrt()
}

/// Compute final confidence score
fn compute_final_confidence(
    base_confidence: f32,
    vh_agreement: bool,
    diversity: f64,
    angle: f64,
    lines_used: usize,
) -> f32 {
    let mut conf = base_confidence as f64;

    // Diversity bonus (up to +0.15)
    conf += diversity * 0.15;

    // V/H agreement bonus
    if vh_agreement {
        conf += 0.10;
    }

    // Line count factor
    if lines_used < 3 {
        conf *= 0.6;
    } else if lines_used < 5 {
        conf *= 0.8;
    }

    // Large angle penalty
    let angle_abs = angle.abs();
    if angle_abs > 5.0 {
        conf *= 0.70;
    } else if angle_abs > 3.0 {
        conf *= 0.85;
    }

    conf.clamp(0.0, 0.90) as f32
}

/// Apply safety limits to angle and confidence
fn apply_safety_limits(angle: f64, confidence: f32) -> (f64, f32) {
    // Determine max rotation based on confidence
    let max_rotation = if confidence >= 0.70 {
        MAX_ROTATION_DEG
    } else if confidence >= 0.55 {
        4.0
    } else if confidence >= 0.40 {
        2.5
    } else if confidence >= 0.25 {
        1.5
    } else {
        0.8
    };

    let mut final_angle = angle;
    let mut final_confidence = confidence;

    // Cap angle if exceeds limit
    if angle.abs() > max_rotation {
        final_angle = angle.signum() * max_rotation;
        final_confidence *= 0.8;
    }

    // Skip tiny rotations
    if final_angle.abs() < MIN_ROTATION_THRESHOLD_DEG {
        return (0.0, confidence);
    }

    (final_angle, final_confidence)
}

/// Return result indicating no correction needed
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

    #[test]
    fn test_line_type_classification() {
        // Vertical line (0 degrees from vertical)
        let v = LineSegment::new(100.0, 0.0, 100.0, 100.0);
        assert_eq!(classify_line_type(&v), LineType::Vertical);

        // Horizontal line (0 degrees from horizontal)
        let h = LineSegment::new(0.0, 100.0, 100.0, 100.0);
        assert_eq!(classify_line_type(&h), LineType::Horizontal);

        // 45 degree diagonal
        let d = LineSegment::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(classify_line_type(&d), LineType::Diagonal);
    }

    #[test]
    fn test_vh_combination_agreement() {
        let v = RansacResult {
            angle: 1.5,
            confidence: 0.6,
            inlier_count: 5,
            inlier_weight: 100.0,
            total_weight: 150.0,
            angle_stddev: 0.5,
        };
        let h = RansacResult {
            angle: 1.3,
            confidence: 0.5,
            inlier_count: 4,
            inlier_weight: 80.0,
            total_weight: 120.0,
            angle_stddev: 0.6,
        };

        let (angle, conf, agreement) = combine_vh_results(&v, &h);
        assert!(agreement);
        assert!(conf > 0.5);
        assert!((angle - 1.43).abs() < 0.1); // Weighted average
    }

    #[test]
    fn test_safety_limits() {
        // High confidence allows larger rotation
        let (angle, _) = apply_safety_limits(5.0, 0.75);
        assert!((angle - 5.0).abs() < 0.1);

        // Low confidence caps rotation
        let (angle, conf) = apply_safety_limits(5.0, 0.20);
        assert!(angle.abs() <= 0.8);
        assert!(conf < 0.20); // Penalty applied
    }
}
