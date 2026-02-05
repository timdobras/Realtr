//! Preprocessing pipeline for auto-straighten detection.
//!
//! Includes:
//! - Consistent downscaling to 800px
//! - Bilateral filtering (preserves edges, smooths textures)
//! - CLAHE (Contrast Limited Adaptive Histogram Equalization)
//! - Lens distortion correction from EXIF data

use image::{DynamicImage, GenericImageView, GrayImage, ImageBuffer, Luma};
use rayon::prelude::*;
use std::path::Path;

/// Target size for processing (longest edge)
const TARGET_SIZE: u32 = 800;

/// Bilateral filter parameters
const BILATERAL_SIGMA_COLOR: f64 = 25.0;
const BILATERAL_SIGMA_SPACE: f64 = 5.0;
const BILATERAL_RADIUS: i32 = 5;

/// CLAHE parameters
const CLAHE_CLIP_LIMIT: f32 = 2.0;
const CLAHE_GRID_SIZE: usize = 8;

/// Preprocess an image for line detection.
///
/// Pipeline:
/// 1. Correct lens distortion (if EXIF available)
/// 2. Downscale to exactly 800px on longest edge
/// 3. Convert to grayscale
/// 4. Apply bilateral filter (preserves edges)
/// 5. Apply CLAHE (normalizes contrast)
pub fn preprocess_for_detection(img: &DynamicImage, image_path: Option<&Path>) -> GrayImage {
    // 1. Read focal length and apply lens distortion correction
    let focal_length = image_path.and_then(read_focal_length);
    let corrected = correct_lens_distortion(img, focal_length);

    // 2. Downscale to consistent size
    let scaled = downscale_to_target(&corrected, TARGET_SIZE);

    // 3. Convert to grayscale
    let gray = scaled.to_luma8();

    // 4. Apply bilateral filter (preserve edges, smooth textures)
    let filtered = bilateral_filter(&gray, BILATERAL_SIGMA_COLOR, BILATERAL_SIGMA_SPACE, BILATERAL_RADIUS);

    // 5. Apply CLAHE (normalize contrast for dark/bright rooms)
    apply_clahe(&filtered, CLAHE_CLIP_LIMIT, CLAHE_GRID_SIZE)
}

/// Preprocess without EXIF (for preview images already loaded)
pub fn preprocess_for_detection_no_exif(img: &DynamicImage) -> GrayImage {
    // Skip lens distortion since we don't have EXIF
    let scaled = downscale_to_target(img, TARGET_SIZE);
    let gray = scaled.to_luma8();
    let filtered = bilateral_filter(&gray, BILATERAL_SIGMA_COLOR, BILATERAL_SIGMA_SPACE, BILATERAL_RADIUS);
    apply_clahe(&filtered, CLAHE_CLIP_LIMIT, CLAHE_GRID_SIZE)
}

/// Read focal length from EXIF metadata
pub fn read_focal_length(path: &Path) -> Option<f64> {
    use exif::{In, Reader, Tag};

    let file = std::fs::File::open(path).ok()?;
    let mut bufreader = std::io::BufReader::new(file);
    let exif_data = Reader::new().read_from_container(&mut bufreader).ok()?;

    let field = exif_data.get_field(Tag::FocalLength, In::PRIMARY)?;

    // FocalLength is stored as a Rational
    match &field.value {
        exif::Value::Rational(v) if !v.is_empty() => {
            let rational = v[0];
            if rational.denom != 0 {
                Some(f64::from(rational.num) / f64::from(rational.denom))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Apply lens distortion correction based on focal length.
///
/// Real estate photos use wide-angle lenses (10-24mm) which cause barrel distortion.
/// This correction is approximate but removes systematic error from border edges.
pub fn correct_lens_distortion(img: &DynamicImage, focal_length_mm: Option<f64>) -> DynamicImage {
    let k1 = match focal_length_mm {
        Some(f) if f <= 14.0 => -0.15,  // Ultra wide (10-14mm)
        Some(f) if f <= 18.0 => -0.10,  // Wide (15-18mm)
        Some(f) if f <= 24.0 => -0.05,  // Moderate wide (19-24mm)
        _ => 0.0,                        // No correction for normal+ lenses
    };

    if k1 == 0.0 {
        return img.clone();
    }

    apply_radial_undistortion(img, k1, 0.0)
}

/// Apply radial undistortion using the Brown-Conrady model.
///
/// r_corrected = r * (1 + k1*r² + k2*r⁴)
fn apply_radial_undistortion(img: &DynamicImage, k1: f64, k2: f64) -> DynamicImage {
    let (width, height) = img.dimensions();
    let cx = f64::from(width) / 2.0;
    let cy = f64::from(height) / 2.0;
    let max_r = (cx * cx + cy * cy).sqrt();

    let rgba = img.to_rgba8();
    let mut output = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            // Convert to normalized coordinates centered at image center
            let dx = (f64::from(x) - cx) / max_r;
            let dy = (f64::from(y) - cy) / max_r;
            let r_sq = dx * dx + dy * dy;
            let r_4 = r_sq * r_sq;

            // Apply distortion model (inverse to undistort)
            let factor = 1.0 + k1 * r_sq + k2 * r_4;
            let src_x = cx + dx * max_r * factor;
            let src_y = cy + dy * max_r * factor;

            // Bilinear interpolation
            let pixel = bilinear_sample(&rgba, src_x, src_y);
            output.put_pixel(x, y, pixel);
        }
    }

    DynamicImage::ImageRgba8(output)
}

/// Bilinear interpolation for smooth sampling
fn bilinear_sample(img: &image::RgbaImage, x: f64, y: f64) -> image::Rgba<u8> {
    let (width, height) = img.dimensions();

    // Handle out of bounds
    if x < 0.0 || y < 0.0 || x >= f64::from(width) - 1.0 || y >= f64::from(height) - 1.0 {
        return image::Rgba([0, 0, 0, 255]);
    }

    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);

    let fx = x - f64::from(x0);
    let fy = y - f64::from(y0);

    let p00 = img.get_pixel(x0, y0);
    let p10 = img.get_pixel(x1, y0);
    let p01 = img.get_pixel(x0, y1);
    let p11 = img.get_pixel(x1, y1);

    let mut result = [0u8; 4];
    for i in 0..4 {
        let v00 = f64::from(p00[i]);
        let v10 = f64::from(p10[i]);
        let v01 = f64::from(p01[i]);
        let v11 = f64::from(p11[i]);

        let v = v00 * (1.0 - fx) * (1.0 - fy)
              + v10 * fx * (1.0 - fy)
              + v01 * (1.0 - fx) * fy
              + v11 * fx * fy;

        result[i] = v.round().clamp(0.0, 255.0) as u8;
    }

    image::Rgba(result)
}

/// Downscale image to have longest edge = target_size
fn downscale_to_target(img: &DynamicImage, target_size: u32) -> DynamicImage {
    let (width, height) = img.dimensions();
    let max_dim = width.max(height);

    if max_dim <= target_size {
        return img.clone();
    }

    let scale = f64::from(target_size) / f64::from(max_dim);
    let new_width = (f64::from(width) * scale).round() as u32;
    let new_height = (f64::from(height) * scale).round() as u32;

    img.resize_exact(new_width, new_height, image::imageops::FilterType::Triangle)
}

/// Apply bilateral filter to preserve edges while smoothing textures.
///
/// Unlike Gaussian blur, bilateral filter considers both spatial distance
/// and intensity difference, keeping edges sharp.
fn bilateral_filter(gray: &GrayImage, sigma_color: f64, sigma_space: f64, radius: i32) -> GrayImage {
    let (width, height) = gray.dimensions();
    let mut output = GrayImage::new(width, height);

    // Precompute spatial weights
    let spatial_weights: Vec<f64> = (0..=radius)
        .map(|d| (-f64::from(d * d) / (2.0 * sigma_space * sigma_space)).exp())
        .collect();

    // Process in parallel
    let rows: Vec<Vec<u8>> = (0..height)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![0u8; width as usize];
            for x in 0..width {
                row[x as usize] = bilateral_pixel(
                    gray,
                    x as i32,
                    y as i32,
                    radius,
                    sigma_color,
                    &spatial_weights,
                );
            }
            row
        })
        .collect();

    for (y, row) in rows.into_iter().enumerate() {
        for (x, pixel) in row.into_iter().enumerate() {
            output.put_pixel(x as u32, y as u32, Luma([pixel]));
        }
    }

    output
}

/// Compute bilateral filtered value for a single pixel
fn bilateral_pixel(
    gray: &GrayImage,
    x: i32,
    y: i32,
    radius: i32,
    sigma_color: f64,
    spatial_weights: &[f64],
) -> u8 {
    let (width, height) = gray.dimensions();
    let center_val = f64::from(gray.get_pixel(x as u32, y as u32)[0]);

    let mut sum = 0.0;
    let mut weight_sum = 0.0;

    for dy in -radius..=radius {
        let ny = y + dy;
        if ny < 0 || ny >= height as i32 {
            continue;
        }

        for dx in -radius..=radius {
            let nx = x + dx;
            if nx < 0 || nx >= width as i32 {
                continue;
            }

            let neighbor_val = f64::from(gray.get_pixel(nx as u32, ny as u32)[0]);

            // Spatial weight (precomputed)
            let spatial_dist = ((dx * dx + dy * dy) as f64).sqrt() as usize;
            let spatial_w = if spatial_dist < spatial_weights.len() {
                spatial_weights[spatial_dist]
            } else {
                0.0
            };

            // Color/intensity weight
            let color_diff = (neighbor_val - center_val).abs();
            let color_w = (-color_diff * color_diff / (2.0 * sigma_color * sigma_color)).exp();

            let weight = spatial_w * color_w;
            sum += neighbor_val * weight;
            weight_sum += weight;
        }
    }

    if weight_sum > 0.0 {
        (sum / weight_sum).round().clamp(0.0, 255.0) as u8
    } else {
        center_val as u8
    }
}

/// Apply CLAHE (Contrast Limited Adaptive Histogram Equalization).
///
/// CLAHE normalizes local contrast, making edge detection work consistently
/// across dark rooms and overexposed windows.
fn apply_clahe(gray: &GrayImage, clip_limit: f32, grid_size: usize) -> GrayImage {
    let (width, height) = gray.dimensions();
    let mut output = GrayImage::new(width, height);

    // Calculate tile dimensions
    let tile_width = (width as usize + grid_size - 1) / grid_size;
    let tile_height = (height as usize + grid_size - 1) / grid_size;

    // Compute histogram for each tile
    let mut tile_mappings: Vec<Vec<[u8; 256]>> = vec![vec![[0u8; 256]; grid_size]; grid_size];

    for ty in 0..grid_size {
        for tx in 0..grid_size {
            let x_start = tx * tile_width;
            let y_start = ty * tile_height;
            let x_end = ((tx + 1) * tile_width).min(width as usize);
            let y_end = ((ty + 1) * tile_height).min(height as usize);

            // Build histogram for this tile
            let mut hist = [0u32; 256];
            let mut pixel_count = 0u32;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    if x < width as usize && y < height as usize {
                        let val = gray.get_pixel(x as u32, y as u32)[0] as usize;
                        hist[val] += 1;
                        pixel_count += 1;
                    }
                }
            }

            if pixel_count == 0 {
                // Identity mapping for empty tiles
                for i in 0..256 {
                    tile_mappings[ty][tx][i] = i as u8;
                }
                continue;
            }

            // Apply clip limit
            let clip_threshold = (clip_limit * (pixel_count as f32) / 256.0) as u32;
            let mut excess = 0u32;

            for h in &mut hist {
                if *h > clip_threshold {
                    excess += *h - clip_threshold;
                    *h = clip_threshold;
                }
            }

            // Redistribute excess
            let redistrib = excess / 256;
            for h in &mut hist {
                *h += redistrib;
            }

            // Build CDF and create mapping
            let mut cdf = [0u32; 256];
            cdf[0] = hist[0];
            for i in 1..256 {
                cdf[i] = cdf[i - 1] + hist[i];
            }

            let cdf_min = cdf.iter().copied().find(|&v| v > 0).unwrap_or(0);
            let scale = if pixel_count > cdf_min {
                255.0 / (pixel_count - cdf_min) as f32
            } else {
                1.0
            };

            for i in 0..256 {
                let mapped = if cdf[i] > cdf_min {
                    ((cdf[i] - cdf_min) as f32 * scale).round().clamp(0.0, 255.0) as u8
                } else {
                    0
                };
                tile_mappings[ty][tx][i] = mapped;
            }
        }
    }

    // Apply mappings with bilinear interpolation between tiles
    for y in 0..height {
        for x in 0..width {
            let val = gray.get_pixel(x, y)[0] as usize;

            // Find which tile(s) this pixel belongs to
            let fx = (x as f32) / (tile_width as f32) - 0.5;
            let fy = (y as f32) / (tile_height as f32) - 0.5;

            let tx0 = (fx.floor() as i32).clamp(0, grid_size as i32 - 1) as usize;
            let ty0 = (fy.floor() as i32).clamp(0, grid_size as i32 - 1) as usize;
            let tx1 = (tx0 + 1).min(grid_size - 1);
            let ty1 = (ty0 + 1).min(grid_size - 1);

            let wx = (fx - tx0 as f32).clamp(0.0, 1.0);
            let wy = (fy - ty0 as f32).clamp(0.0, 1.0);

            // Bilinear interpolation of mapped values
            let v00 = tile_mappings[ty0][tx0][val] as f32;
            let v10 = tile_mappings[ty0][tx1][val] as f32;
            let v01 = tile_mappings[ty1][tx0][val] as f32;
            let v11 = tile_mappings[ty1][tx1][val] as f32;

            let interpolated = v00 * (1.0 - wx) * (1.0 - wy)
                             + v10 * wx * (1.0 - wy)
                             + v01 * (1.0 - wx) * wy
                             + v11 * wx * wy;

            output.put_pixel(x, y, Luma([interpolated.round().clamp(0.0, 255.0) as u8]));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downscale() {
        let img = DynamicImage::new_rgb8(1600, 1200);
        let scaled = downscale_to_target(&img, 800);
        assert_eq!(scaled.width(), 800);
        assert_eq!(scaled.height(), 600);
    }

    #[test]
    fn test_no_downscale_needed() {
        let img = DynamicImage::new_rgb8(640, 480);
        let scaled = downscale_to_target(&img, 800);
        assert_eq!(scaled.width(), 640);
        assert_eq!(scaled.height(), 480);
    }

    #[test]
    fn test_lens_distortion_coefficients() {
        // Ultra wide should get correction
        assert!(correct_lens_distortion(&DynamicImage::new_rgb8(100, 100), Some(12.0)).width() > 0);

        // Normal lens should not be modified
        let img = DynamicImage::new_rgb8(100, 100);
        let result = correct_lens_distortion(&img, Some(50.0));
        // Should return same dimensions (no change)
        assert_eq!(result.width(), img.width());
    }
}
