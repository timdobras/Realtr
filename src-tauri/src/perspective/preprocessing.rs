//! Preprocessing pipeline for auto-straighten detection.
//!
//! Includes:
//! - Consistent downscaling to 800px
//! - Bilateral filtering (preserves edges, smooths textures) — GPU-accelerated
//! - CLAHE (Contrast Limited Adaptive Histogram Equalization) — GPU-accelerated
//! - Lens distortion correction from EXIF data — GPU-accelerated

use crate::gpu::ImageProcessor;
use image::{DynamicImage, GenericImageView, GrayImage};
use std::path::Path;

/// Target size for processing (longest edge)
const TARGET_SIZE: u32 = 800;

/// Preprocess an image for line detection.
///
/// Pipeline:
/// 1. Correct lens distortion (GPU if available, else CPU)
/// 2. Downscale to exactly 800px on longest edge
/// 3. Convert to grayscale
/// 4. Apply bilateral filter on GPU (preserves edges, smooths textures)
/// 5. Apply CLAHE on GPU (normalizes contrast)
pub fn preprocess_for_detection(
    img: &DynamicImage,
    image_path: Option<&Path>,
    processor: &ImageProcessor,
) -> GrayImage {
    // 1. Read focal length and apply lens distortion correction (GPU-accelerated)
    let focal_length = image_path.and_then(read_focal_length);
    let corrected = correct_lens_distortion(img, focal_length, processor);

    // 2. Downscale to consistent size
    let scaled = downscale_to_target(&corrected, TARGET_SIZE);

    // 3. Convert to grayscale
    let gray = scaled.to_luma8();

    // 4+5. Apply bilateral filter + CLAHE via GPU (or CPU fallback)
    apply_bilateral_and_clahe(&gray, processor)
}

/// Preprocess without EXIF (for preview images already loaded)
pub fn preprocess_for_detection_no_exif(
    img: &DynamicImage,
    processor: &ImageProcessor,
) -> GrayImage {
    // Skip lens distortion since we don't have EXIF
    let scaled = downscale_to_target(img, TARGET_SIZE);
    let gray = scaled.to_luma8();
    apply_bilateral_and_clahe(&gray, processor)
}

/// Apply bilateral filter followed by CLAHE, using GPU when available.
/// Both operations work on grayscale pixel buffers — the GPU versions accept/return
/// flat `&[u8]` / `Vec<u8>` so we convert GrayImage ↔ raw bytes.
fn apply_bilateral_and_clahe(gray: &GrayImage, processor: &ImageProcessor) -> GrayImage {
    let (width, height) = gray.dimensions();
    let raw_pixels = gray.as_raw().as_slice();

    // Bilateral filter (GPU: compute shader 16x16 workgroups; CPU: rayon parallel rows)
    let filtered = processor
        .bilateral_filter(raw_pixels, width, height)
        .unwrap_or_else(|e| {
            eprintln!("[preprocess] bilateral_filter failed: {e}, using unfiltered");
            raw_pixels.to_vec()
        });

    // CLAHE (GPU: two-pass histogram+apply shader; CPU: tile-based with bilinear interp)
    let equalized = processor
        .clahe(&filtered, width, height)
        .unwrap_or_else(|e| {
            eprintln!("[preprocess] clahe failed: {e}, using bilateral-only output");
            filtered
        });

    GrayImage::from_raw(width, height, equalized)
        .expect("CLAHE output size must match input dimensions")
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
/// Uses GPU-accelerated undistortion when available, with CPU fallback.
pub fn correct_lens_distortion(
    img: &DynamicImage,
    focal_length_mm: Option<f64>,
    processor: &ImageProcessor,
) -> DynamicImage {
    let k1 = match focal_length_mm {
        Some(f) if f <= 14.0 => -0.15_f32, // Ultra wide (10-14mm)
        Some(f) if f <= 18.0 => -0.10_f32, // Wide (15-18mm)
        Some(f) if f <= 24.0 => -0.05_f32, // Moderate wide (19-24mm)
        _ => 0.0_f32,                      // No correction for normal+ lenses
    };

    if k1 == 0.0 {
        return img.clone();
    }

    // GPU undistort handles the Brown-Conrady model with bilinear interpolation
    processor.undistort(img, k1).unwrap_or_else(|e| {
        eprintln!("[preprocess] GPU undistort failed: {e}, skipping lens correction");
        img.clone()
    })
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

    crate::fast_resize::resize_exact(&img, new_width, new_height)
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
        let cpu_processor = ImageProcessor::Cpu;

        // Ultra wide should get correction
        assert!(
            correct_lens_distortion(
                &DynamicImage::new_rgb8(100, 100),
                Some(12.0),
                &cpu_processor
            )
            .width()
                > 0
        );

        // Normal lens should not be modified
        let img = DynamicImage::new_rgb8(100, 100);
        let result = correct_lens_distortion(&img, Some(50.0), &cpu_processor);
        // Should return same dimensions (no change)
        assert_eq!(result.width(), img.width());
    }
}
