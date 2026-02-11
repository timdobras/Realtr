//! Line segment detection and RANSAC-based angle estimation for image straightening.
//!
//! Uses GPU gradient histogram (replacing OpenCV LSD) to find dominant line angles,
//! then applies RANSAC to find the dominant vertical angle.

use crate::gpu::ImageProcessor;
use crate::perspective::{
    PerspectiveAnalysis, VanishingPoint, VanishingPointType, CONFIDENCE_THRESHOLD,
    MAX_ANGLE_STDDEV_DEG, MAX_ROTATION_DEG, MIN_INLIER_COUNT, MIN_LINE_LENGTH_RATIO,
    MIN_ROTATION_THRESHOLD_DEG, RANSAC_INLIER_THRESHOLD_DEG, RANSAC_ITERATIONS,
    VERTICAL_TOLERANCE_DEG,
};
use image::{DynamicImage, GenericImageView};
use rand::Rng;

/// Gradient magnitude threshold for histogram voting
const GRADIENT_MAGNITUDE_THRESHOLD: f32 = 30.0;

/// Minimum peak prominence as fraction of max peak
const PEAK_PROMINENCE_RATIO: f64 = 0.05;

/// Peak detection window half-width in bins (0.1 degree per bin)
const PEAK_WINDOW_HALF: usize = 20; // +/- 2 degrees

/// A detected line segment (synthesized from gradient histogram peaks)
#[derive(Debug, Clone)]
struct LineSegment {
    /// Start X coordinate
    x1: f64,
    /// Start Y coordinate
    y1: f64,
    /// End X coordinate
    x2: f64,
    /// End Y coordinate
    y2: f64,
    /// Angle from vertical (in radians, 0 = perfectly vertical)
    angle_from_vertical: f64,
    /// Length of the line segment
    length: f64,
}

impl LineSegment {
    /// Create a new line segment and calculate its properties
    fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let length = (dx * dx + dy * dy).sqrt();

        let (norm_dx, norm_dy) = if dy >= 0.0 { (dx, dy) } else { (-dx, -dy) };
        let angle_from_vertical = norm_dx.atan2(norm_dy);

        Self {
            x1,
            y1,
            x2,
            y2,
            angle_from_vertical,
            length,
        }
    }
}

/// Detect vertical line segments using GPU gradient histogram.
///
/// Replaces the old OpenCV LSD approach. Computes the gradient histogram
/// on the grayscale image, finds peaks near 0/180 degrees (which correspond
/// to vertical edges), and synthesizes virtual line segments.
fn detect_vertical_lines_from_histogram(
    gray: &image::GrayImage,
    processor: &ImageProcessor,
) -> Result<Vec<LineSegment>, String> {
    let (width, height) = gray.dimensions();

    // Compute gradient histogram
    let histogram =
        processor.gradient_histogram(gray.as_raw(), width, height, GRADIENT_MAGNITUDE_THRESHOLD)?;

    // Find peaks
    let peaks = find_histogram_peaks(&histogram);

    if peaks.is_empty() {
        return Ok(Vec::new());
    }

    // Only keep peaks that correspond to vertical edges
    // (gradient near 0 or 180 degrees = horizontal gradient = vertical edge)
    let vertical_tolerance_rad = VERTICAL_TOLERANCE_DEG.to_radians();
    let min_length = f64::from(height) * MIN_LINE_LENGTH_RATIO;

    // Central 50% of image width
    let center_margin = f64::from(width) * 0.25;
    let left_bound = center_margin;
    let right_bound = f64::from(width) - center_margin;

    let mut segments = Vec::new();

    for peak in &peaks {
        // Only consider vertical-edge peaks
        if !peak.is_vertical_edge {
            continue;
        }

        // Check if the deviation is within vertical tolerance
        if peak.deviation_from_vertical_rad.abs() > vertical_tolerance_rad {
            continue;
        }

        // Synthesize multiple lines across the image
        let relative_weight = peak.weight / peaks.iter().map(|p| p.weight).fold(0.0_f64, f64::max);
        let num_lines = (relative_weight * 10.0).round().max(3.0) as usize;

        for i in 0..num_lines {
            let frac = (i as f64 + 0.5) / num_lines as f64;
            let x = left_bound + (right_bound - left_bound) * frac;
            let line_len = f64::from(height) * 0.6;
            let y_center = f64::from(height) * 0.5;

            let tilt = peak.deviation_from_vertical_rad;
            let dx = (line_len / 2.0) * tilt.sin();
            let dy = (line_len / 2.0) * tilt.cos();

            let seg = LineSegment::new(x - dx, y_center - dy, x + dx, y_center + dy);
            if seg.length >= min_length {
                segments.push(seg);
            }
        }
    }

    eprintln!(
        "[detection] gradient histogram found {} vertical-edge peaks -> {} virtual lines",
        peaks.iter().filter(|p| p.is_vertical_edge).count(),
        segments.len()
    );

    Ok(segments)
}

/// A detected peak in the gradient histogram
#[derive(Debug, Clone)]
struct HistogramPeak {
    /// Center angle in degrees
    angle_deg: f64,
    /// Weight/magnitude
    weight: f64,
    /// Whether this peak corresponds to a vertical edge
    is_vertical_edge: bool,
    /// Deviation from exact vertical (in radians, signed)
    deviation_from_vertical_rad: f64,
}

/// Find peaks in the 3600-bin gradient histogram
fn find_histogram_peaks(histogram: &[f32]) -> Vec<HistogramPeak> {
    let num_bins = histogram.len();
    if num_bins == 0 {
        return Vec::new();
    }

    let max_val = histogram.iter().copied().fold(0.0_f32, f32::max);
    if max_val < 1.0 {
        return Vec::new();
    }

    let min_prominence = max_val as f64 * PEAK_PROMINENCE_RATIO;

    // Find local maxima
    let mut raw_peaks: Vec<(usize, f64)> = Vec::new();
    for i in 0..num_bins {
        let val = histogram[i] as f64;
        if val < min_prominence {
            continue;
        }

        let mut is_peak = true;
        for offset in 1..=PEAK_WINDOW_HALF {
            let left = (i + num_bins - offset) % num_bins;
            let right = (i + offset) % num_bins;
            if histogram[left] as f64 > val || histogram[right] as f64 > val {
                is_peak = false;
                break;
            }
        }

        if is_peak {
            raw_peaks.push((i, val));
        }
    }

    // Merge nearby peaks (within 2 degrees = 20 bins)
    let mut merged: Vec<HistogramPeak> = Vec::new();
    let mut used = vec![false; raw_peaks.len()];

    for i in 0..raw_peaks.len() {
        if used[i] {
            continue;
        }

        let (bin_i, weight_i) = raw_peaks[i];
        let mut total_weight = weight_i;
        let mut weighted_angle_sum = (bin_i as f64 * 0.1) * weight_i;
        used[i] = true;

        for j in (i + 1)..raw_peaks.len() {
            if used[j] {
                continue;
            }
            let (bin_j, weight_j) = raw_peaks[j];
            let dist = circular_bin_distance(bin_i, bin_j, num_bins);
            if dist <= PEAK_WINDOW_HALF * 2 {
                weighted_angle_sum += (bin_j as f64 * 0.1) * weight_j;
                total_weight += weight_j;
                used[j] = true;
            }
        }

        let center_angle = weighted_angle_sum / total_weight;

        // Classify: gradient near 0/180 = vertical edge, near 90/270 = horizontal edge
        let angle = center_angle.rem_euclid(360.0);
        let dev_from_0 = if angle > 180.0 { 360.0 - angle } else { angle };
        let dev_from_180 = (angle - 180.0).abs();
        let min_v_dev = dev_from_0.min(dev_from_180);

        let is_vertical = min_v_dev <= VERTICAL_TOLERANCE_DEG;

        // Signed deviation: gradient slightly above 0 -> line tilts right (positive)
        let signed_dev_deg = if angle <= 90.0 {
            angle
        } else if angle >= 270.0 {
            angle - 360.0
        } else if angle <= 180.0 {
            angle - 180.0
        } else {
            angle - 180.0
        };

        merged.push(HistogramPeak {
            angle_deg: center_angle,
            weight: total_weight,
            is_vertical_edge: is_vertical,
            deviation_from_vertical_rad: signed_dev_deg.to_radians(),
        });
    }

    merged.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged.truncate(20);

    merged
}

/// Compute circular distance between two bin indices
fn circular_bin_distance(a: usize, b: usize, total: usize) -> usize {
    let diff = if a > b { a - b } else { b - a };
    diff.min(total - diff)
}

/// Filter line segments to keep only near-vertical lines
fn filter_vertical_lines(lines: &[LineSegment]) -> Vec<LineSegment> {
    let vertical_tolerance_rad = VERTICAL_TOLERANCE_DEG.to_radians();

    lines
        .iter()
        .filter(|line| line.angle_from_vertical.abs() <= vertical_tolerance_rad)
        .cloned()
        .collect()
}

/// RANSAC result with additional statistics for quality assessment
struct RansacResult {
    /// Refined dominant angle from vertical (radians)
    angle: f64,
    /// Confidence (ratio of inlier weight to total weight)
    confidence: f32,
    /// Number of inlier lines
    inlier_count: usize,
    /// Standard deviation of inlier angles (degrees)
    angle_stddev: f64,
}

/// Find the dominant vertical angle using weighted RANSAC
fn find_dominant_angle_ransac(lines: &[LineSegment]) -> RansacResult {
    if lines.is_empty() {
        return RansacResult {
            angle: 0.0,
            confidence: 0.0,
            inlier_count: 0,
            angle_stddev: 0.0,
        };
    }

    if lines.len() == 1 {
        return RansacResult {
            angle: lines[0].angle_from_vertical,
            confidence: 1.0,
            inlier_count: 1,
            angle_stddev: 0.0,
        };
    }

    let mut rng = rand::thread_rng();
    let mut best_angle = 0.0;
    let mut best_weighted_count = 0.0;
    let mut best_inlier_count = 0;
    let inlier_threshold_rad = RANSAC_INLIER_THRESHOLD_DEG.to_radians();

    let total_weight: f64 = lines.iter().map(|l| l.length * l.length).sum();

    for _ in 0..RANSAC_ITERATIONS {
        let sample_idx = rng.gen_range(0..lines.len());
        let hypothesis_angle = lines[sample_idx].angle_from_vertical;

        let mut weighted_count = 0.0;
        let mut inlier_count = 0;

        for line in lines {
            let angle_diff = (line.angle_from_vertical - hypothesis_angle).abs();
            if angle_diff < inlier_threshold_rad {
                weighted_count += line.length * line.length;
                inlier_count += 1;
            }
        }

        if weighted_count > best_weighted_count {
            best_weighted_count = weighted_count;
            best_angle = hypothesis_angle;
            best_inlier_count = inlier_count;
        }
    }

    // Refine angle with weighted average of inliers
    let mut inlier_angles: Vec<f64> = Vec::new();
    let mut inlier_weights: Vec<f64> = Vec::new();

    for line in lines {
        let angle_diff = (line.angle_from_vertical - best_angle).abs();
        if angle_diff < inlier_threshold_rad {
            inlier_angles.push(line.angle_from_vertical);
            inlier_weights.push(line.length * line.length);
        }
    }

    let refined_weight_sum: f64 = inlier_weights.iter().sum();
    let refined_angle = if refined_weight_sum > 0.0 {
        inlier_angles
            .iter()
            .zip(inlier_weights.iter())
            .map(|(a, w)| a * w)
            .sum::<f64>()
            / refined_weight_sum
    } else {
        best_angle
    };

    let variance = if refined_weight_sum > 0.0 && inlier_angles.len() > 1 {
        inlier_angles
            .iter()
            .zip(inlier_weights.iter())
            .map(|(a, w)| {
                let diff = a - refined_angle;
                diff * diff * w
            })
            .sum::<f64>()
            / refined_weight_sum
    } else {
        0.0
    };
    let angle_stddev = variance.sqrt().to_degrees();

    let confidence = if total_weight > 0.0 {
        (best_weighted_count / total_weight) as f32
    } else {
        0.0
    };

    eprintln!(
        "[detection/ransac] {} inliers, confidence={:.2}, stddev={:.2} deg",
        best_inlier_count, confidence, angle_stddev
    );

    RansacResult {
        angle: refined_angle,
        confidence,
        inlier_count: best_inlier_count,
        angle_stddev,
    }
}

/// Main entry point - analyze image for perspective correction using gradient histogram + RANSAC
pub fn analyze_perspective(
    img: &DynamicImage,
    processor: &ImageProcessor,
) -> Result<PerspectiveAnalysis, String> {
    let (width, height) = img.dimensions();

    eprintln!("\n=== Perspective Analysis ===");
    eprintln!("Image size: {}x{}", width, height);

    // 1. Convert to grayscale
    let gray = img.to_luma8();

    // 2. Detect vertical lines using gradient histogram
    let all_lines = detect_vertical_lines_from_histogram(&gray, processor)?;

    // 3. Filter for near-vertical lines
    let vertical_lines = filter_vertical_lines(&all_lines);

    eprintln!(
        "Found {} vertical lines out of {} total",
        vertical_lines.len(),
        all_lines.len()
    );

    if vertical_lines.is_empty() {
        eprintln!("No vertical lines found - skipping correction");
        return Ok(no_correction_needed());
    }

    // 4. Find dominant angle using weighted RANSAC
    let result = find_dominant_angle_ransac(&vertical_lines);

    // 5. Calculate rotation needed (negative to correct the tilt)
    let rotation_deg = -result.angle.to_degrees();

    eprintln!(
        "Dominant angle: {:.2} deg, rotation needed: {:.2} deg",
        result.angle.to_degrees(),
        rotation_deg
    );

    // 6. Quality checks

    if result.inlier_count < MIN_INLIER_COUNT {
        eprintln!(
            "REJECT: Only {} inliers (need at least {})",
            result.inlier_count, MIN_INLIER_COUNT
        );
        return Ok(no_correction_needed());
    }

    if result.confidence < CONFIDENCE_THRESHOLD {
        eprintln!(
            "REJECT: Confidence {:.2} below threshold {:.2}",
            result.confidence, CONFIDENCE_THRESHOLD
        );
        return Ok(no_correction_needed());
    }

    if result.angle_stddev > MAX_ANGLE_STDDEV_DEG {
        eprintln!(
            "REJECT: Angle stddev {:.2} deg exceeds max {:.2} deg - detection ambiguous",
            result.angle_stddev, MAX_ANGLE_STDDEV_DEG
        );
        return Ok(no_correction_needed());
    }

    if rotation_deg.abs() < MIN_ROTATION_THRESHOLD_DEG {
        eprintln!(
            "SKIP: Rotation {:.2} deg below minimum threshold {:.2} deg",
            rotation_deg, MIN_ROTATION_THRESHOLD_DEG
        );
        return Ok(already_straight(result.confidence, result.inlier_count));
    }

    if rotation_deg.abs() > MAX_ROTATION_DEG {
        eprintln!(
            "REJECT: Rotation {:.2} deg exceeds maximum {:.2} deg - needs manual review",
            rotation_deg, MAX_ROTATION_DEG
        );
        return Ok(needs_manual_review());
    }

    eprintln!(
        "ACCEPT: Applying {:.2} deg rotation (confidence={:.2}, inliers={}, stddev={:.2} deg)",
        rotation_deg, result.confidence, result.inlier_count, result.angle_stddev
    );

    let center_x = f64::from(width) / 2.0;
    let vp = VanishingPoint {
        x: center_x + result.angle.tan() * f64::from(height) * 10.0,
        y: -f64::from(height) * 10.0,
        confidence: result.confidence,
        vp_type: VanishingPointType::Vertical,
    };

    Ok(PerspectiveAnalysis {
        vanishing_points: vec![vp],
        suggested_rotation: rotation_deg,
        confidence: result.confidence,
        needs_correction: true,
        lines_detected: result.inlier_count,
    })
}

/// Return analysis indicating no correction needed
fn no_correction_needed() -> PerspectiveAnalysis {
    PerspectiveAnalysis {
        vanishing_points: vec![],
        suggested_rotation: 0.0,
        confidence: 0.0,
        needs_correction: false,
        lines_detected: 0,
    }
}

/// Return analysis indicating image is already straight
fn already_straight(confidence: f32, lines_detected: usize) -> PerspectiveAnalysis {
    PerspectiveAnalysis {
        vanishing_points: vec![],
        suggested_rotation: 0.0,
        confidence,
        needs_correction: false,
        lines_detected,
    }
}

/// Return analysis indicating image needs manual review
fn needs_manual_review() -> PerspectiveAnalysis {
    PerspectiveAnalysis {
        vanishing_points: vec![],
        suggested_rotation: 0.0,
        confidence: 0.0,
        needs_correction: false,
        lines_detected: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_segment_angle() {
        // Perfectly vertical line
        let vertical = LineSegment::new(100.0, 0.0, 100.0, 100.0);
        assert!(vertical.angle_from_vertical.abs() < 0.01);

        // 45 degree line
        let diagonal = LineSegment::new(0.0, 0.0, 100.0, 100.0);
        assert!((diagonal.angle_from_vertical.abs() - std::f64::consts::FRAC_PI_4).abs() < 0.01);
    }

    #[test]
    fn test_filter_vertical_lines() {
        let lines = vec![
            LineSegment::new(100.0, 0.0, 100.0, 100.0), // Vertical
            LineSegment::new(0.0, 0.0, 100.0, 0.0),     // Horizontal
            LineSegment::new(100.0, 0.0, 110.0, 100.0), // Near vertical (~6 deg)
        ];

        let filtered = filter_vertical_lines(&lines);
        assert_eq!(filtered.len(), 2); // Vertical and near-vertical should pass
    }

    #[test]
    fn test_circular_bin_distance() {
        assert_eq!(circular_bin_distance(10, 20, 3600), 10);
        assert_eq!(circular_bin_distance(3590, 10, 3600), 20);
        assert_eq!(circular_bin_distance(0, 3599, 3600), 1);
    }
}
