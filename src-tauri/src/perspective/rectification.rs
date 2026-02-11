//! Image rectification - applying rotation transforms to straighten images.
//!
//! Uses simple rotation around image center with auto-cropping to remove
//! black corners introduced by rotation.

use crate::perspective::PerspectiveAnalysis;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use imageproc::geometric_transformations::{warp, Interpolation, Projection};
use nalgebra::Matrix3;

/// Apply perspective correction to an image based on analysis
pub fn apply_correction(
    img: &DynamicImage,
    analysis: &PerspectiveAnalysis,
) -> Result<DynamicImage, String> {
    if !analysis.needs_correction {
        // No correction needed, return clone
        return Ok(img.clone());
    }

    // Apply simple rotation based on the suggested rotation
    apply_rotation(img, analysis.suggested_rotation)
}

/// Apply a simple rotation to an image
fn apply_rotation(img: &DynamicImage, angle_degrees: f64) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();
    let cx = f64::from(width) / 2.0;
    let cy = f64::from(height) / 2.0;

    let angle_radians = angle_degrees.to_radians();
    let rotation = compute_rotation_matrix(-angle_radians, cx, cy);

    apply_homography(img, &rotation)
}

/// Compute a 2D rotation matrix centered at (cx, cy)
fn compute_rotation_matrix(angle_radians: f64, cx: f64, cy: f64) -> Matrix3<f64> {
    let cos_a = angle_radians.cos();
    let sin_a = angle_radians.sin();

    // Rotation around center: T(cx,cy) * R * T(-cx,-cy)
    Matrix3::new(
        cos_a,
        -sin_a,
        cx * (1.0 - cos_a) + cy * sin_a,
        sin_a,
        cos_a,
        cy * (1.0 - cos_a) - cx * sin_a,
        0.0,
        0.0,
        1.0,
    )
}

/// Apply a homography transformation to an image
fn apply_homography(img: &DynamicImage, homography: &Matrix3<f64>) -> Result<DynamicImage, String> {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Compute the inverse homography for backward mapping
    let inv_homography = homography
        .try_inverse()
        .ok_or("Failed to invert homography matrix")?;

    // Convert to imageproc Projection format
    let projection = Projection::from_matrix([
        inv_homography[(0, 0)] as f32,
        inv_homography[(0, 1)] as f32,
        inv_homography[(0, 2)] as f32,
        inv_homography[(1, 0)] as f32,
        inv_homography[(1, 1)] as f32,
        inv_homography[(1, 2)] as f32,
        inv_homography[(2, 0)] as f32,
        inv_homography[(2, 1)] as f32,
        inv_homography[(2, 2)] as f32,
    ])
    .ok_or("Invalid projection matrix")?;

    // Apply the warp with Lanczos interpolation for better quality
    let default_pixel = Rgba([0, 0, 0, 0]);
    let warped = warp(&rgba, &projection, Interpolation::Bilinear, default_pixel);

    // Auto-crop to remove black borders
    let cropped = auto_crop_black_borders(&warped, width, height)?;

    Ok(DynamicImage::ImageRgba8(cropped))
}

/// Auto-crop an image to remove black (transparent) borders
/// Uses the largest inscribed rectangle approach
fn auto_crop_black_borders(
    img: &RgbaImage,
    original_width: u32,
    original_height: u32,
) -> Result<RgbaImage, String> {
    let (width, height) = img.dimensions();

    // Find the bounding box of non-transparent pixels
    let mut min_x = width;
    let mut max_x = 0u32;
    let mut min_y = height;
    let mut max_y = 0u32;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            // Consider pixels with alpha > 200 and not pure black as content
            if pixel[3] > 200 && (pixel[0] > 5 || pixel[1] > 5 || pixel[2] > 5) {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }

    // Check if we found any content
    if min_x >= max_x || min_y >= max_y {
        // No valid content found, return original
        return Ok(img.clone());
    }

    // Add small margin to avoid edge artifacts
    let margin = 2;
    min_x = min_x.saturating_sub(margin);
    min_y = min_y.saturating_sub(margin);
    max_x = (max_x + margin).min(width - 1);
    max_y = (max_y + margin).min(height - 1);

    let new_width = max_x - min_x + 1;
    let new_height = max_y - min_y + 1;

    // Safety check: don't crop more than 30% of original image
    let original_area = original_width * original_height;
    let new_area = new_width * new_height;

    if new_area < original_area * 70 / 100 {
        // Cropping would remove more than 30%, likely an error
        // Return the original rotated image without cropping
        println!(
            "Warning: Cropping would remove >30% of image ({}% remaining), skipping crop",
            new_area * 100 / original_area
        );
        return Ok(img.clone());
    }

    // Create cropped image
    let mut cropped = RgbaImage::new(new_width, new_height);
    for y in 0..new_height {
        for x in 0..new_width {
            let src_pixel = img.get_pixel(x + min_x, y + min_y);
            cropped.put_pixel(x, y, *src_pixel);
        }
    }

    Ok(cropped)
}

/// Generate a preview of the corrected image (scaled down for UI)
/// Note: This function expects an already-corrected image and just resizes it for preview
pub fn generate_correction_preview(
    corrected_img: &DynamicImage,
    _analysis: &PerspectiveAnalysis,
    max_size: u32,
) -> Result<DynamicImage, String> {
    // Scale down for preview (image is already corrected by caller)
    let (width, height) = corrected_img.dimensions();
    let scale = f64::from(max_size) / f64::from(width.max(height));

    if scale < 1.0 {
        Ok(crate::fast_resize::resize_to_fit(corrected_img, max_size))
    } else {
        Ok(corrected_img.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Vector3;

    #[test]
    fn test_rotation_matrix_identity() {
        let matrix = compute_rotation_matrix(0.0, 100.0, 100.0);
        let identity = Matrix3::<f64>::identity();
        for i in 0..3 {
            for j in 0..3 {
                let diff: f64 = matrix[(i, j)] - identity[(i, j)];
                assert!(diff.abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_small_rotation_preserves_center() {
        let matrix = compute_rotation_matrix(0.1, 100.0, 100.0);
        // Transform center point
        let center = Vector3::new(100.0, 100.0, 1.0);
        let transformed = matrix * center;
        let tx = transformed[0] / transformed[2];
        let ty = transformed[1] / transformed[2];
        assert!((tx - 100.0).abs() < 1e-10);
        assert!((ty - 100.0).abs() < 1e-10);
    }
}
