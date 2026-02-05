//! Vanishing point estimation for validation of straightening detection.
//!
//! Interior photos almost always have a dominant vanishing point where parallel
//! walls converge in perspective. The offset of this vanishing point from the
//! image center directly encodes camera tilt.
//!
//! This provides geometric validation independent from line-angle averaging.

use crate::perspective::straighten::{ClassifiedLine, RansacResult};

/// Estimated vanishing point
#[derive(Debug, Clone)]
pub struct VPEstimate {
    /// Position (can be outside image bounds)
    pub x: f64,
    pub y: f64,
    /// Implied tilt angle in degrees
    pub tilt_angle: f64,
    /// Confidence score 0-1
    pub confidence: f32,
    /// Number of line pairs that contributed
    pub supporting_pairs: usize,
}

/// Weighted intersection point
#[derive(Debug, Clone)]
struct WeightedIntersection {
    x: f64,
    y: f64,
    weight: f64,
}

/// Estimate the vertical vanishing point from classified lines.
///
/// For vertical lines (walls), the VP is typically far above or below the image.
/// The horizontal offset of this VP from image center indicates camera tilt.
pub fn estimate_vertical_vp(
    vertical_lines: &[ClassifiedLine],
    img_dims: (u32, u32),
) -> Option<VPEstimate> {
    if vertical_lines.len() < 2 {
        return None;
    }

    let (width, height) = img_dims;
    let center_x = f64::from(width) / 2.0;
    let max_vp_distance = f64::from(height) * 20.0; // VP can be far away

    // 1. Compute pairwise intersections
    let intersections = compute_pairwise_intersections(vertical_lines, max_vp_distance);

    if intersections.is_empty() {
        return None;
    }

    // 2. Filter to keep only intersections above/below image (vertical VP)
    let valid_intersections: Vec<_> = intersections
        .into_iter()
        .filter(|i| {
            // VP for vertical lines should be above (y < 0) or below (y > height)
            let above = i.y < 0.0 && i.y > -max_vp_distance;
            let below = i.y > f64::from(height) && i.y < f64::from(height) + max_vp_distance;
            above || below
        })
        .collect();

    if valid_intersections.is_empty() {
        return None;
    }

    // 3. Cluster intersections using weighted mean-shift
    let cluster = cluster_intersections(&valid_intersections, f64::from(width) * 0.1);

    if cluster.weight < 1.0 {
        return None;
    }

    // 4. Calculate tilt angle from VP offset
    // If VP is at (vp_x, vp_y) and image center is (cx, cy):
    // tilt_angle = atan2(vp_x - cx, |vp_y|)
    let vp_offset_x = cluster.x - center_x;
    let tilt_angle = (vp_offset_x / cluster.y.abs()).atan().to_degrees();

    // 5. Compute confidence based on cluster quality
    let supporting_pairs = (cluster.weight / 10.0).min(20.0) as usize;
    let spread = compute_cluster_spread(&valid_intersections, cluster.x, cluster.y);
    let spread_factor = (1.0 - spread / (f64::from(width) * 0.2)).max(0.0);

    let confidence = ((supporting_pairs as f32 / 10.0).min(1.0) * spread_factor as f32 * 0.7)
        .clamp(0.0, 0.6);

    Some(VPEstimate {
        x: cluster.x,
        y: cluster.y,
        tilt_angle,
        confidence,
        supporting_pairs,
    })
}

/// Estimate horizontal vanishing point from horizontal lines.
///
/// For horizontal lines (floor/ceiling edges), the VP is typically
/// to the left or right of the image.
pub fn estimate_horizontal_vp(
    horizontal_lines: &[ClassifiedLine],
    img_dims: (u32, u32),
) -> Option<VPEstimate> {
    if horizontal_lines.len() < 2 {
        return None;
    }

    let (width, height) = img_dims;
    let center_y = f64::from(height) / 2.0;
    let max_vp_distance = f64::from(width) * 20.0;

    // 1. Compute pairwise intersections
    let intersections = compute_pairwise_intersections(horizontal_lines, max_vp_distance);

    if intersections.is_empty() {
        return None;
    }

    // 2. Filter to keep only intersections to left/right of image
    let valid_intersections: Vec<_> = intersections
        .into_iter()
        .filter(|i| {
            let left = i.x < 0.0 && i.x > -max_vp_distance;
            let right = i.x > f64::from(width) && i.x < f64::from(width) + max_vp_distance;
            // Also check it's roughly at image height level
            let near_center_y = (i.y - center_y).abs() < f64::from(height);
            (left || right) && near_center_y
        })
        .collect();

    if valid_intersections.is_empty() {
        return None;
    }

    // 3. Cluster intersections
    let cluster = cluster_intersections(&valid_intersections, f64::from(height) * 0.1);

    if cluster.weight < 1.0 {
        return None;
    }

    // 4. Calculate tilt from VP offset (vertical offset from center indicates tilt)
    let vp_offset_y = cluster.y - center_y;
    let tilt_angle = (vp_offset_y / cluster.x.abs()).atan().to_degrees();

    // 5. Confidence
    let supporting_pairs = (cluster.weight / 10.0).min(20.0) as usize;
    let spread = compute_cluster_spread(&valid_intersections, cluster.x, cluster.y);
    let spread_factor = (1.0 - spread / (f64::from(height) * 0.2)).max(0.0);

    let confidence = ((supporting_pairs as f32 / 10.0).min(1.0) * spread_factor as f32 * 0.6)
        .clamp(0.0, 0.5);

    Some(VPEstimate {
        x: cluster.x,
        y: cluster.y,
        tilt_angle,
        confidence,
        supporting_pairs,
    })
}

/// Compute pairwise line intersections
fn compute_pairwise_intersections(
    lines: &[ClassifiedLine],
    max_distance: f64,
) -> Vec<WeightedIntersection> {
    let mut intersections = Vec::new();

    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            if let Some((x, y)) = line_intersection(&lines[i].segment, &lines[j].segment) {
                // Check if intersection is within reasonable bounds
                if x.abs() < max_distance && y.abs() < max_distance {
                    // Weight by product of line weights and lengths
                    let weight = (lines[i].weight * lines[j].weight).sqrt()
                        * (lines[i].segment.length * lines[j].segment.length).sqrt()
                        / 1000.0;

                    intersections.push(WeightedIntersection { x, y, weight });
                }
            }
        }
    }

    intersections
}

/// Compute intersection point of two line segments (extended to infinity)
fn line_intersection(
    l1: &crate::perspective::straighten::LineSegment,
    l2: &crate::perspective::straighten::LineSegment,
) -> Option<(f64, f64)> {
    // Line 1: from (x1, y1) to (x2, y2)
    let dx1 = l1.x2 - l1.x1;
    let dy1 = l1.y2 - l1.y1;

    // Line 2: from (x3, y3) to (x4, y4)
    let dx2 = l2.x2 - l2.x1;
    let dy2 = l2.y2 - l2.y1;

    // Cross product for parallel check
    let cross = dx1 * dy2 - dy1 * dx2;

    // Lines are parallel (or nearly so)
    if cross.abs() < 1e-10 {
        return None;
    }

    // Solve for intersection using parametric form
    let t = ((l2.x1 - l1.x1) * dy2 - (l2.y1 - l1.y1) * dx2) / cross;

    let x = l1.x1 + t * dx1;
    let y = l1.y1 + t * dy1;

    Some((x, y))
}

/// Cluster intersections using weighted mean-shift
fn cluster_intersections(
    intersections: &[WeightedIntersection],
    bandwidth: f64,
) -> WeightedIntersection {
    if intersections.is_empty() {
        return WeightedIntersection {
            x: 0.0,
            y: 0.0,
            weight: 0.0,
        };
    }

    // Start from weighted centroid
    let total_weight: f64 = intersections.iter().map(|i| i.weight).sum();
    let mut center_x: f64 = intersections.iter().map(|i| i.x * i.weight).sum::<f64>() / total_weight;
    let mut center_y: f64 = intersections.iter().map(|i| i.y * i.weight).sum::<f64>() / total_weight;

    // Mean-shift iterations
    for _ in 0..20 {
        let mut new_x = 0.0;
        let mut new_y = 0.0;
        let mut weight_sum = 0.0;

        for i in intersections {
            let dist = ((i.x - center_x).powi(2) + (i.y - center_y).powi(2)).sqrt();

            // Gaussian kernel
            let kernel = (-dist * dist / (2.0 * bandwidth * bandwidth)).exp();
            let w = i.weight * kernel;

            new_x += i.x * w;
            new_y += i.y * w;
            weight_sum += w;
        }

        if weight_sum > 0.0 {
            let prev_x = center_x;
            let prev_y = center_y;
            center_x = new_x / weight_sum;
            center_y = new_y / weight_sum;

            // Convergence check
            let shift = ((center_x - prev_x).powi(2) + (center_y - prev_y).powi(2)).sqrt();
            if shift < bandwidth * 0.01 {
                break;
            }
        }
    }

    // Count weight of points near final center
    let cluster_weight: f64 = intersections
        .iter()
        .filter(|i| {
            let dist = ((i.x - center_x).powi(2) + (i.y - center_y).powi(2)).sqrt();
            dist < bandwidth * 2.0
        })
        .map(|i| i.weight)
        .sum();

    WeightedIntersection {
        x: center_x,
        y: center_y,
        weight: cluster_weight,
    }
}

/// Compute spread (standard deviation) of intersections around cluster center
fn compute_cluster_spread(
    intersections: &[WeightedIntersection],
    center_x: f64,
    center_y: f64,
) -> f64 {
    if intersections.is_empty() {
        return 0.0;
    }

    let total_weight: f64 = intersections.iter().map(|i| i.weight).sum();
    if total_weight == 0.0 {
        return 0.0;
    }

    let variance: f64 = intersections
        .iter()
        .map(|i| {
            let dist_sq = (i.x - center_x).powi(2) + (i.y - center_y).powi(2);
            dist_sq * i.weight
        })
        .sum::<f64>()
        / total_weight;

    variance.sqrt()
}

/// Validate RANSAC result using vanishing point estimation.
///
/// Returns adjusted (angle, confidence) based on VP agreement.
pub fn validate_with_vp(
    ransac: &RansacResult,
    vertical_lines: &[ClassifiedLine],
    horizontal_lines: &[ClassifiedLine],
    img_dims: (u32, u32),
) -> (f64, f32) {
    let v_vp = estimate_vertical_vp(vertical_lines, img_dims);
    let h_vp = estimate_horizontal_vp(horizontal_lines, img_dims);

    let mut angle = ransac.angle;
    let mut confidence = ransac.confidence;

    // Check vertical VP
    if let Some(vp) = &v_vp {
        let agreement = (vp.tilt_angle - ransac.angle).abs() < 1.5;

        if agreement {
            // VP agrees - confidence boost
            confidence += 0.15 * vp.confidence;
        } else if vp.confidence > ransac.confidence * 0.8 {
            // VP strongly disagrees and is confident - blend angles
            angle = angle * 0.7 + vp.tilt_angle * 0.3;
            confidence *= 0.85;
        } else {
            // VP disagrees but less confident - slight penalty
            confidence *= 0.9;
        }
    }

    // Check horizontal VP (less reliable, smaller adjustments)
    if let Some(vp) = &h_vp {
        let agreement = (vp.tilt_angle - angle).abs() < 2.0;

        if agreement {
            confidence += 0.08 * vp.confidence;
        } else {
            confidence *= 0.95;
        }
    }

    // Cross-validation: if both VPs exist and agree, extra boost
    if let (Some(v), Some(h)) = (&v_vp, &h_vp) {
        if (v.tilt_angle - h.tilt_angle).abs() < 1.5 {
            confidence += 0.10;
        }
    }

    (angle, confidence.clamp(0.0, 0.90))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perspective::straighten::{LineSegment, PositionType};

    fn make_classified_line(x1: f64, y1: f64, x2: f64, y2: f64) -> ClassifiedLine {
        ClassifiedLine {
            segment: LineSegment::new(x1, y1, x2, y2),
            line_type: LineType::Vertical,
            position: PositionType::Border,
            weight: 1.0,
        }
    }

    #[test]
    fn test_line_intersection() {
        // Two lines that intersect at (50, 50)
        let l1 = LineSegment::new(0.0, 0.0, 100.0, 100.0);
        let l2 = LineSegment::new(0.0, 100.0, 100.0, 0.0);

        let (x, y) = line_intersection(&l1, &l2).expect("should intersect");
        assert!((x - 50.0).abs() < 0.01);
        assert!((y - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parallel_lines_no_intersection() {
        let l1 = LineSegment::new(0.0, 0.0, 100.0, 0.0);
        let l2 = LineSegment::new(0.0, 10.0, 100.0, 10.0);

        assert!(line_intersection(&l1, &l2).is_none());
    }

    #[test]
    fn test_cluster_intersections() {
        let intersections = vec![
            WeightedIntersection {
                x: 100.0,
                y: -500.0,
                weight: 1.0,
            },
            WeightedIntersection {
                x: 105.0,
                y: -510.0,
                weight: 1.0,
            },
            WeightedIntersection {
                x: 98.0,
                y: -495.0,
                weight: 1.0,
            },
            // Outlier
            WeightedIntersection {
                x: 500.0,
                y: -100.0,
                weight: 0.5,
            },
        ];

        let cluster = cluster_intersections(&intersections, 50.0);

        // Should cluster around ~100, -500
        assert!((cluster.x - 100.0).abs() < 20.0);
        assert!((cluster.y - (-500.0)).abs() < 20.0);
    }
}
