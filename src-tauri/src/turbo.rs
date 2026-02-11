//! Fast JPEG decode/encode using libjpeg-turbo via the `turbojpeg` crate.
//!
//! Provides 3-5x faster JPEG operations compared to the `image` crate's pure-Rust decoder.
//! Falls back to the `image` crate for non-JPEG formats (PNG, WebP, BMP, GIF).

use image::{DynamicImage, RgbImage};
use std::path::Path;

/// Load an image from disk. Uses turbojpeg for JPEG files (3-5x faster),
/// falls back to `image::open()` for other formats.
/// Accepts any type that can be converted to a Path reference.
pub fn load_image<P: AsRef<Path>>(path: P) -> Result<DynamicImage, String> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();

    if ext == "jpg" || ext == "jpeg" {
        load_jpeg(path)
    } else {
        image::open(path).map_err(|e| format!("Failed to open image: {e}"))
    }
}

/// Load a JPEG file using turbojpeg (3-5x faster than `image` crate).
fn load_jpeg(path: &Path) -> Result<DynamicImage, String> {
    let jpeg_data =
        std::fs::read(path).map_err(|e| format!("Failed to read file {}: {e}", path.display()))?;

    let rgb: image::RgbImage = turbojpeg::decompress_image(&jpeg_data)
        .map_err(|e| format!("turbojpeg decode failed for {}: {e}", path.display()))?;

    Ok(DynamicImage::ImageRgb8(rgb))
}

/// Encode an `RgbImage` to JPEG bytes using turbojpeg (3-5x faster).
/// Returns the raw JPEG bytes.
pub fn encode_jpeg(img: &RgbImage, quality: i32) -> Result<Vec<u8>, String> {
    let buf = turbojpeg::compress_image(img, quality, turbojpeg::Subsamp::Sub2x2)
        .map_err(|e| format!("turbojpeg encode failed: {e}"))?;
    Ok(buf.to_vec())
}

/// Encode an `RgbImage` to JPEG bytes and then to base64 string.
/// Replaces the old `encode_to_base64_jpeg` pattern.
pub fn encode_jpeg_base64(img: &RgbImage, quality: i32) -> Result<String, String> {
    use base64::Engine;
    let jpeg_bytes = encode_jpeg(img, quality)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes))
}

/// Save an `RgbImage` to disk as JPEG using turbojpeg (3-5x faster encoding).
pub fn save_jpeg<P: AsRef<Path>>(img: &RgbImage, path: P, quality: i32) -> Result<(), String> {
    let path = path.as_ref();
    let jpeg_bytes = encode_jpeg(img, quality)?;
    std::fs::write(path, &jpeg_bytes)
        .map_err(|e| format!("Failed to write JPEG to {}: {e}", path.display()))
}

/// Load a JPEG at reduced resolution using DCT-scaled decoding.
/// This is dramatically faster for thumbnail generation — the JPEG decoder
/// only computes partial IDCT, so a 1/4 scale decode of a 6000x4000 image
/// produces a 1500x1000 image in ~15ms instead of the full 200ms decode.
///
/// `max_size` is the target maximum dimension. The function picks the best
/// scaling factor that produces an image >= max_size on its longest edge.
pub fn load_jpeg_scaled(path: &Path, max_size: u32) -> Result<DynamicImage, String> {
    let jpeg_data =
        std::fs::read(path).map_err(|e| format!("Failed to read file {}: {e}", path.display()))?;

    let mut decompressor =
        turbojpeg::Decompressor::new().map_err(|e| format!("turbojpeg init failed: {e}"))?;

    let header = decompressor
        .read_header(&jpeg_data)
        .map_err(|e| format!("Failed to read JPEG header: {e}"))?;

    let max_dim = header.width.max(header.height);

    // Choose the smallest scaling factor that gives us >= max_size
    let scaling = if max_dim <= max_size as usize {
        turbojpeg::ScalingFactor::ONE
    } else {
        // Available scaling factors from smallest to largest
        let candidates = [
            turbojpeg::ScalingFactor::ONE_EIGHTH,  // 1/8
            turbojpeg::ScalingFactor::ONE_QUARTER, // 1/4
            turbojpeg::ScalingFactor::ONE_HALF,    // 1/2
            turbojpeg::ScalingFactor::ONE,         // 1x
        ];
        candidates
            .iter()
            .find(|s| s.scale(max_dim) >= max_size as usize)
            .copied()
            .unwrap_or(turbojpeg::ScalingFactor::ONE)
    };

    decompressor
        .set_scaling_factor(scaling)
        .map_err(|e| format!("Failed to set scaling: {e}"))?;

    let scaled = header.scaled(scaling);

    // Decode at reduced resolution (DCT-domain scaling — very fast)
    let mut image = turbojpeg::Image {
        pixels: vec![0u8; 3 * scaled.width * scaled.height],
        width: scaled.width,
        pitch: 3 * scaled.width,
        height: scaled.height,
        format: turbojpeg::PixelFormat::RGB,
    };

    decompressor
        .decompress(&jpeg_data, image.as_deref_mut())
        .map_err(|e| format!("turbojpeg scaled decode failed: {e}"))?;

    // Convert to image::RgbImage
    let rgb = image::RgbImage::from_raw(scaled.width as u32, scaled.height as u32, image.pixels)
        .ok_or_else(|| "Failed to construct RgbImage from turbojpeg output".to_string())?;

    Ok(DynamicImage::ImageRgb8(rgb))
}
