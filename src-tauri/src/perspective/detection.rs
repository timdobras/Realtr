//! Line segment detection and RANSAC-based angle estimation for image straightening.
//!
//! Uses OpenCV's Line Segment Detector (LSD) to find line segments,
//! then applies RANSAC to find the dominant vertical angle.

use crate::perspective::{
    PerspectiveAnalysis, VanishingPoint, VanishingPointType,
    CONFIDENCE_THRESHOLD, MAX_ANGLE_STDDEV_DEG, MAX_ROTATION_DEG,
    MIN_INLIER_COUNT, MIN_LINE_LENGTH_RATIO, MIN_ROTATION_THRESHOLD_DEG,
    RANSAC_INLIER_THRESHOLD_DEG, RANSAC_ITERATIONS, VERTICAL_TOLERANCE_DEG,
};
use image::{DynamicImage, GenericImageView};
use opencv::core::{Mat, Scalar, CV_8UC1};
use opencv::imgproc;
use opencv::prelude::{LineSegmentDetectorTrait, MatTraitConst, MatTrait};
use rand::Rng;

/// A detected line segment
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

        // Calculate angle from vertical (0 = vertical line)
        // IMPORTANT: Normalize direction so we always measure from lower-y to higher-y point
        // This ensures consistent angle sign regardless of which endpoint LSD reports first
        let (norm_dx, norm_dy) = if dy >= 0.0 {
            (dx, dy)
        } else {
            (-dx, -dy)
        };
        // atan2(dx, dy) gives angle from vertical axis
        // Positive = tilts right, Negative = tilts left
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

/// Detect line segments using OpenCV's LSD
fn detect_line_segments_lsd(gray: &image::GrayImage) -> Result<Vec<LineSegment>, String> {
    let (width, height) = gray.dimensions();

    // Convert image::GrayImage to OpenCV Mat
    let mat = gray_image_to_mat(gray)?;

    // Create LSD detector with default parameters for best detection
    let mut lsd = imgproc::create_line_segment_detector_def()
        .map_err(|e| format!("Failed to create LSD detector: {e}"))?;

    // Detect lines - need all 5 arguments, but we only use lines
    let mut lines = Mat::default();
    let mut width_out = Mat::default();
    let mut prec_out = Mat::default();
    let mut nfa_out = Mat::default();
    lsd.detect(&mat, &mut lines, &mut width_out, &mut prec_out, &mut nfa_out)
        .map_err(|e| format!("LSD detection failed: {e}"))?;

    println!("LSD raw output: rows={}, cols={}, type={}", lines.rows(), lines.cols(), lines.typ());

    // Convert to LineSegment structs
    // LSD output is a Mat of shape (N, 1) with type CV_32FC4 (each element is [x1,y1,x2,y2])
    let min_length = f64::from(height) * MIN_LINE_LENGTH_RATIO;
    let mut segments = Vec::new();

    // Only consider lines in the central 50% of the image width
    // This avoids edge distortion and furniture at sides
    let center_margin = f64::from(width) * 0.25;
    let left_bound = center_margin;
    let right_bound = f64::from(width) - center_margin;

    let num_lines = lines.rows();
    println!("Processing {} detected lines (center zone: {:.0}-{:.0}px)", num_lines, left_bound, right_bound);

    for i in 0..num_lines {
        // Each row contains a Vec4f (x1, y1, x2, y2)
        let line: &opencv::core::Vec4f = lines.at(i)
            .map_err(|e| format!("Failed to get line {}: {e}", i))?;

        let x1 = f64::from(line[0]);
        let y1 = f64::from(line[1]);
        let x2 = f64::from(line[2]);
        let y2 = f64::from(line[3]);

        // Check if line is in center zone (both endpoints or midpoint)
        let mid_x = (x1 + x2) / 2.0;
        let in_center = mid_x >= left_bound && mid_x <= right_bound;

        if !in_center {
            continue;
        }

        let segment = LineSegment::new(x1, y1, x2, y2);

        // Filter by minimum length
        if segment.length >= min_length {
            segments.push(segment);
        }
    }

    println!("LSD detected {} lines, {} after length+center filtering (min_length: {:.1}px)",
        num_lines, segments.len(), min_length);

    Ok(segments)
}

/// Convert image::GrayImage to OpenCV Mat
fn gray_image_to_mat(gray: &image::GrayImage) -> Result<Mat, String> {
    let (width, height) = gray.dimensions();

    // Create empty Mat with correct dimensions
    let mut mat = Mat::new_rows_cols_with_default(
        height as i32,
        width as i32,
        CV_8UC1,
        Scalar::all(0.0),
    ).map_err(|e| format!("Failed to create Mat: {e}"))?;

    // Copy pixel data row by row (more efficient than pixel-by-pixel)
    let raw_data = gray.as_raw();
    for y in 0..height as i32 {
        let row_start = (y as usize) * (width as usize);
        let row_end = row_start + (width as usize);
        let row_data = &raw_data[row_start..row_end];

        for (x, &pixel) in row_data.iter().enumerate() {
            *mat.at_2d_mut::<u8>(y, x as i32)
                .map_err(|e| format!("Failed to set pixel at ({},{}): {e}", x, y))? = pixel;
        }
    }

    println!("Created Mat: {}x{}, type: {}, channels: {}",
        mat.cols(), mat.rows(), mat.typ(), mat.channels());

    Ok(mat)
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
/// Uses length² weighting to heavily favor long architectural lines
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

    // Total weight for confidence calculation (length squared)
    let total_weight: f64 = lines.iter().map(|l| l.length * l.length).sum();

    for _ in 0..RANSAC_ITERATIONS {
        // Random sample
        let sample_idx = rng.gen_range(0..lines.len());
        let hypothesis_angle = lines[sample_idx].angle_from_vertical;

        // Count weighted inliers (length² weighting)
        let mut weighted_count = 0.0;
        let mut inlier_count = 0;

        for line in lines {
            let angle_diff = (line.angle_from_vertical - hypothesis_angle).abs();
            if angle_diff < inlier_threshold_rad {
                weighted_count += line.length * line.length;  // Weight by length²
                inlier_count += 1;
            }
        }

        if weighted_count > best_weighted_count {
            best_weighted_count = weighted_count;
            best_angle = hypothesis_angle;
            best_inlier_count = inlier_count;
        }
    }

    // Collect inlier angles and weights for refinement and stddev
    let mut inlier_angles: Vec<f64> = Vec::new();
    let mut inlier_weights: Vec<f64> = Vec::new();

    for line in lines {
        let angle_diff = (line.angle_from_vertical - best_angle).abs();
        if angle_diff < inlier_threshold_rad {
            inlier_angles.push(line.angle_from_vertical);
            inlier_weights.push(line.length * line.length);
        }
    }

    // Refine angle by taking weighted average of inliers
    let refined_weight_sum: f64 = inlier_weights.iter().sum();
    let refined_angle = if refined_weight_sum > 0.0 {
        inlier_angles.iter()
            .zip(inlier_weights.iter())
            .map(|(a, w)| a * w)
            .sum::<f64>() / refined_weight_sum
    } else {
        best_angle
    };

    // Calculate weighted standard deviation of inlier angles
    let variance = if refined_weight_sum > 0.0 && inlier_angles.len() > 1 {
        inlier_angles.iter()
            .zip(inlier_weights.iter())
            .map(|(a, w)| {
                let diff = a - refined_angle;
                diff * diff * w
            })
            .sum::<f64>() / refined_weight_sum
    } else {
        0.0
    };
    let angle_stddev = variance.sqrt().to_degrees();

    // Confidence = ratio of inlier weight to total weight
    let confidence = if total_weight > 0.0 {
        (best_weighted_count / total_weight) as f32
    } else {
        0.0
    };

    println!("RANSAC: {} inliers, confidence={:.2}, stddev={:.2}°",
        best_inlier_count, confidence, angle_stddev);

    RansacResult {
        angle: refined_angle,
        confidence,
        inlier_count: best_inlier_count,
        angle_stddev,
    }
}

/// Main entry point - analyze image for straightening using LSD + RANSAC
pub fn analyze_perspective(img: &DynamicImage) -> Result<PerspectiveAnalysis, String> {
    let (width, height) = img.dimensions();

    println!("\n=== Perspective Analysis ===");
    println!("Image size: {}x{}", width, height);

    // 1. Convert to grayscale for LSD
    let gray = img.to_luma8();

    // 2. Detect all line segments using LSD
    let all_lines = detect_line_segments_lsd(&gray)?;

    // 3. Filter for near-vertical lines
    let vertical_lines = filter_vertical_lines(&all_lines);

    println!("Found {} vertical lines out of {} after length filter",
        vertical_lines.len(), all_lines.len());

    if vertical_lines.is_empty() {
        println!("No vertical lines found - skipping correction");
        return Ok(no_correction_needed());
    }

    // 4. Find dominant angle using weighted RANSAC
    let result = find_dominant_angle_ransac(&vertical_lines);

    // 5. Calculate rotation needed (negative because we want to correct the tilt)
    let rotation_deg = -result.angle.to_degrees();

    println!("Dominant angle: {:.2}°, rotation needed: {:.2}°",
        result.angle.to_degrees(), rotation_deg);

    // 6. Quality checks - be conservative to avoid making images worse

    // Check minimum inlier count
    if result.inlier_count < MIN_INLIER_COUNT {
        println!("REJECT: Only {} inliers (need at least {})",
            result.inlier_count, MIN_INLIER_COUNT);
        return Ok(no_correction_needed());
    }

    // Check confidence threshold
    if result.confidence < CONFIDENCE_THRESHOLD {
        println!("REJECT: Confidence {:.2} below threshold {:.2}",
            result.confidence, CONFIDENCE_THRESHOLD);
        return Ok(no_correction_needed());
    }

    // Check angle variance (high variance = ambiguous detection)
    if result.angle_stddev > MAX_ANGLE_STDDEV_DEG {
        println!("REJECT: Angle stddev {:.2}° exceeds max {:.2}° - detection ambiguous",
            result.angle_stddev, MAX_ANGLE_STDDEV_DEG);
        return Ok(no_correction_needed());
    }

    // Check minimum rotation threshold
    if rotation_deg.abs() < MIN_ROTATION_THRESHOLD_DEG {
        println!("SKIP: Rotation {:.2}° below minimum threshold {:.2}°",
            rotation_deg, MIN_ROTATION_THRESHOLD_DEG);
        return Ok(already_straight(result.confidence, result.inlier_count));
    }

    // Check maximum rotation
    if rotation_deg.abs() > MAX_ROTATION_DEG {
        println!("REJECT: Rotation {:.2}° exceeds maximum {:.2}° - needs manual review",
            rotation_deg, MAX_ROTATION_DEG);
        return Ok(needs_manual_review());
    }

    println!("ACCEPT: Applying {:.2}° rotation (confidence={:.2}, inliers={}, stddev={:.2}°)",
        rotation_deg, result.confidence, result.inlier_count, result.angle_stddev);

    // Create a synthetic vertical VP for compatibility with rectification code
    let center_x = f64::from(width) / 2.0;
    let vp = VanishingPoint {
        x: center_x + result.angle.tan() * f64::from(height) * 10.0,
        y: -f64::from(height) * 10.0,  // Far above image
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

/// Return analysis indicating no correction needed (no vertical lines detected)
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

/// Return analysis indicating image needs manual review (extreme rotation)
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
            LineSegment::new(100.0, 0.0, 100.0, 100.0),  // Vertical
            LineSegment::new(0.0, 0.0, 100.0, 0.0),      // Horizontal
            LineSegment::new(100.0, 0.0, 110.0, 100.0),  // Near vertical (~6°)
        ];

        let filtered = filter_vertical_lines(&lines);
        assert_eq!(filtered.len(), 2);  // Vertical and near-vertical should pass
    }
}
