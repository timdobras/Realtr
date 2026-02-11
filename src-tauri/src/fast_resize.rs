//! SIMD-accelerated image resizing using `fast_image_resize`.
//!
//! Provides 14-23x faster resizing compared to the `image` crate's built-in resize,
//! using SSE4.1/AVX2 SIMD instructions on x86_64 and NEON on ARM.
//!
//! Used for thumbnail generation, preview resizing, and preprocessing downscaling.

use fast_image_resize as fir;
use image::{DynamicImage, RgbaImage};

/// Resize an image to fit within `max_dim` x `max_dim`, preserving aspect ratio.
/// Uses SIMD-accelerated bilinear filtering for optimal speed/quality balance.
///
/// Falls back to the `image` crate if `fast_image_resize` fails.
pub fn resize_to_fit(img: &DynamicImage, max_dim: u32) -> DynamicImage {
    let (src_w, src_h) = (img.width(), img.height());
    let max_src = src_w.max(src_h);

    if max_src <= max_dim {
        return img.clone();
    }

    let scale = max_dim as f64 / max_src as f64;
    let dst_w = ((src_w as f64) * scale).round().max(1.0) as u32;
    let dst_h = ((src_h as f64) * scale).round().max(1.0) as u32;

    resize_exact(img, dst_w, dst_h)
}

/// Resize an image to exact dimensions using SIMD-accelerated bilinear filtering.
///
/// Falls back to the `image` crate if `fast_image_resize` fails.
pub fn resize_exact(img: &DynamicImage, dst_w: u32, dst_h: u32) -> DynamicImage {
    match try_fast_resize(img, dst_w, dst_h) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[fast_resize] Failed, falling back to image crate: {e}");
            img.resize_exact(dst_w, dst_h, image::imageops::FilterType::Triangle)
        }
    }
}

/// Internal: attempt fast resize using SIMD.
fn try_fast_resize(img: &DynamicImage, dst_w: u32, dst_h: u32) -> Result<DynamicImage, String> {
    let rgba = img.to_rgba8();
    let (src_w, src_h) = rgba.dimensions();

    // Create source image view
    let src_image =
        fir::images::Image::from_vec_u8(src_w, src_h, rgba.into_raw(), fir::PixelType::U8x4)
            .map_err(|e| format!("Failed to create source image: {e}"))?;

    // Create destination buffer
    let mut dst_image = fir::images::Image::new(dst_w, dst_h, fir::PixelType::U8x4);

    // Resize with bilinear filter (good quality, fast)
    let mut resizer = fir::Resizer::new();
    resizer
        .resize(
            &src_image,
            &mut dst_image,
            &fir::ResizeOptions::new()
                .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear)),
        )
        .map_err(|e| format!("Resize failed: {e}"))?;

    // Convert back to image crate type
    let result = RgbaImage::from_raw(dst_w, dst_h, dst_image.into_vec())
        .ok_or_else(|| "Failed to create RgbaImage from resize result".to_string())?;

    Ok(DynamicImage::ImageRgba8(result))
}
