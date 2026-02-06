//! Built-in image editor module for cropping, rotation, and adjustments.
//!
//! Provides Tauri commands for non-destructive image editing with real-time preview.
//! Uses image caching and resize-first pipeline for <30ms preview response times.

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

// ============================================================================
// Image Cache for Real-Time Preview
// ============================================================================

/// Cached image data for fast preview generation.
/// Stores both the full image (for saving) and a pre-resized preview (for fast edits).
pub struct ImageCache {
    pub path: String,
    pub preview_image: DynamicImage, // Pre-resized to ~800px for fast processing
    pub preview_size: u32,
}

/// Type alias for the cached image state
pub type ImageCacheState = Mutex<Option<ImageCache>>;

/// Result from loading an image into the cache
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorLoadResult {
    pub width: u32,
    pub height: u32,
    pub preview_base64: String,
}

/// Parameters for image editing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditParams {
    // Crop (normalized coordinates 0-1)
    pub crop_enabled: bool,
    pub crop_x: f32,
    pub crop_y: f32,
    pub crop_width: f32,
    pub crop_height: f32,

    // Rotation
    pub fine_rotation: f32, // -45 to +45 degrees
    pub quarter_turns: u8,  // 0, 1, 2, or 3 (0°, 90°, 180°, 270°)

    // Adjustments (-100 to 100, default 0)
    pub brightness: i32,
    pub exposure: i32,
    pub contrast: i32,
    pub highlights: i32,
    pub shadows: i32,
}

impl Default for EditParams {
    fn default() -> Self {
        Self {
            crop_enabled: false,
            crop_x: 0.0,
            crop_y: 0.0,
            crop_width: 1.0,
            crop_height: 1.0,
            fine_rotation: 0.0,
            quarter_turns: 0,
            brightness: 0,
            exposure: 0,
            contrast: 0,
            highlights: 0,
            shadows: 0,
        }
    }
}

/// Result returned by editor commands
#[derive(Debug, Serialize)]
pub struct EditorCommandResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Auto-adjustment suggestions based on image analysis
#[derive(Debug, Serialize)]
pub struct AutoAdjustments {
    pub brightness: i32,
    pub exposure: i32,
    pub contrast: i32,
    pub highlights: i32,
    pub shadows: i32,
}

/// Auto-straighten result
#[derive(Debug, Serialize)]
pub struct AutoStraightenResult {
    pub angle: f32,      // Suggested rotation angle in degrees
    pub confidence: f32, // Confidence level 0-1
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the dimensions of an image
#[tauri::command]
pub async fn editor_get_dimensions(image_path: String) -> Result<(u32, u32), String> {
    let img = image::open(&image_path).map_err(|e| format!("Failed to open image: {e}"))?;
    Ok(img.dimensions())
}

/// Load an image into the cache for fast preview generation.
/// This should be called once when opening the editor.
/// Returns the original dimensions and initial preview.
#[tauri::command]
pub async fn editor_load_image(
    app: AppHandle,
    image_path: String,
    preview_size: u32,
) -> Result<EditorLoadResult, String> {
    // Load the original image from disk
    let img = image::open(&image_path).map_err(|e| format!("Failed to open image: {e}"))?;
    let (width, height) = img.dimensions();

    // Create pre-resized preview version for fast processing
    let preview_img = resize_for_preview(&img, preview_size);

    // Generate initial preview (no edits applied)
    let preview_base64 = encode_to_base64_jpeg(&preview_img)?;

    // Store in cache
    let cache = app.state::<ImageCacheState>();
    let mut guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    *guard = Some(ImageCache {
        path: image_path,
        preview_image: preview_img,
        preview_size,
    });

    Ok(EditorLoadResult {
        width,
        height,
        preview_base64,
    })
}

/// Generate a preview of the edited image using the cached preview image.
/// This is optimized for speed - processes the small preview image, not full resolution.
#[tauri::command]
pub async fn editor_generate_preview(
    app: AppHandle,
    params: EditParams,
) -> Result<String, String> {
    // Get the cached preview image
    let cache = app.state::<ImageCacheState>();
    let guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    let cached = guard.as_ref().ok_or("No image loaded. Call editor_load_image first.")?;

    // Apply edits to the pre-resized preview image (fast!)
    let edited = apply_all_edits_fast(&cached.preview_image, &params)?;

    // Encode to base64 JPEG
    encode_to_base64_jpeg(&edited)
}

/// Legacy preview generation that loads from disk (slower, but works without cache)
#[allow(dead_code)]
pub async fn editor_generate_preview_legacy(
    image_path: String,
    params: EditParams,
    preview_size: u32,
) -> Result<String, String> {
    // Load the original image
    let img = image::open(&image_path).map_err(|e| format!("Failed to open image: {e}"))?;

    // Apply all edits
    let edited = apply_all_edits(&img, &params)?;

    // Resize for preview
    let preview = resize_for_preview(&edited, preview_size);

    // Encode to base64 JPEG
    encode_to_base64_jpeg(&preview)
}

/// Save the edited image (replaces original)
#[tauri::command]
pub async fn editor_save_image(
    image_path: String,
    params: EditParams,
) -> Result<EditorCommandResult, String> {
    // Load the original image
    let img = image::open(&image_path).map_err(|e| format!("Failed to open image: {e}"))?;

    // Apply all edits at full resolution
    let edited = apply_all_edits(&img, &params)?;

    // Determine output format from original file extension
    let path = Path::new(&image_path);
    let format = get_image_format(path)?;

    // Save the edited image, replacing the original
    save_image(&edited, path, format)?;

    Ok(EditorCommandResult {
        success: true,
        error: None,
    })
}

/// Analyze the cached image and suggest auto-adjustments based on histogram analysis.
/// Uses percentile-based approach to determine optimal brightness, exposure, and contrast.
#[tauri::command]
pub async fn editor_analyze_image(app: AppHandle) -> Result<AutoAdjustments, String> {
    // Get the cached preview image
    let cache = app.state::<ImageCacheState>();
    let guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    let cached = guard
        .as_ref()
        .ok_or("No image loaded. Call editor_load_image first.")?;

    // Analyze the preview image (fast since it's pre-resized)
    let adjustments = analyze_image_histogram(&cached.preview_image);

    Ok(adjustments)
}

/// Analyze image histogram and calculate optimal adjustments
pub fn analyze_image_histogram(img: &DynamicImage) -> AutoAdjustments {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Sample pixels (every 4th pixel for speed on larger previews)
    let step = 4;
    let mut luminances: Vec<u8> = Vec::with_capacity((width * height / (step * step)) as usize);

    for y in (0..height).step_by(step as usize) {
        for x in (0..width).step_by(step as usize) {
            let pixel = rgba.get_pixel(x, y);
            // Calculate luminance: 0.299*R + 0.587*G + 0.114*B
            let luminance = (0.299 * f32::from(pixel[0])
                + 0.587 * f32::from(pixel[1])
                + 0.114 * f32::from(pixel[2])) as u8;
            luminances.push(luminance);
        }
    }

    if luminances.is_empty() {
        return AutoAdjustments {
            brightness: 0,
            exposure: 0,
            contrast: 0,
            highlights: 0,
            shadows: 0,
        };
    }

    // Sort for percentile calculation
    luminances.sort_unstable();

    let len = luminances.len();
    let p5_idx = len * 5 / 100;
    let p95_idx = (len * 95 / 100).min(len - 1);
    let p5 = luminances[p5_idx] as f32;
    let p95 = luminances[p95_idx] as f32;

    // Calculate mean brightness
    let sum: u64 = luminances.iter().map(|&v| u64::from(v)).sum();
    let mean = sum as f32 / len as f32;

    // Calculate dynamic range
    let dynamic_range = p95 - p5;

    // Target values for web-optimized real estate photos
    // Slight brightness boost without washing out
    let target_mean: f32 = 135.0; // Moderately above neutral (128)

    // Brightness adjustment: gentle correction with small base boost
    let base_brightness_boost: f32 = 5.0;
    let brightness_adj =
        (base_brightness_boost + (target_mean - mean) * 0.2).clamp(-20.0, 30.0) as i32;

    // Exposure adjustment: only for notably dark images
    let exposure_adj = if mean < 100.0 {
        // Notably dark image - increase exposure
        ((100.0 - mean) * 0.2).clamp(0.0, 20.0) as i32
    } else if p5 > 60.0 {
        // Already very bright/washed out - decrease
        ((60.0 - p5) * 0.25).clamp(-15.0, 0.0) as i32
    } else {
        0
    };

    // Contrast adjustment: slight reduction for softer web look
    let contrast_adj = if dynamic_range > 200.0 {
        // High contrast - reduce
        ((200.0 - dynamic_range) * 0.12).clamp(-25.0, 0.0) as i32
    } else if dynamic_range < 80.0 {
        // Very flat image - slight boost
        ((80.0 - dynamic_range) * 0.1).clamp(0.0, 10.0) as i32
    } else {
        // Normal range - minimal adjustment
        -5
    };

    // Highlights adjustment: recover blown highlights
    let highlights_adj = if p95 > 245.0 {
        // Highlights are blown - reduce them
        ((240.0 - p95) * 0.6).clamp(-25.0, 0.0) as i32
    } else if p95 < 180.0 {
        // Image is quite dark - slight highlight boost
        ((180.0 - p95) * 0.15).clamp(0.0, 10.0) as i32
    } else {
        0
    };

    // Shadows adjustment: lift only crushed shadows
    let shadows_adj = if p5 < 10.0 {
        // Shadows are crushed - lift them
        ((20.0 - p5) * 0.5).clamp(0.0, 25.0) as i32
    } else if p5 > 60.0 {
        // Image is washed out - add shadow depth
        ((60.0 - p5) * 0.3).clamp(-20.0, 0.0) as i32
    } else {
        0
    };

    AutoAdjustments {
        brightness: brightness_adj,
        exposure: exposure_adj,
        contrast: contrast_adj,
        highlights: highlights_adj,
        shadows: shadows_adj,
    }
}

/// Analyze the cached image and detect the optimal straightening angle.
/// Uses LSD + RANSAC + Vanishing Point validation for robust detection.
#[tauri::command]
pub async fn editor_auto_straighten(app: AppHandle) -> Result<AutoStraightenResult, String> {
    use crate::perspective::straighten::analyze_straighten;
    use std::path::Path;

    // Get the cached preview image
    let cache = app.state::<ImageCacheState>();
    let guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    let cached = guard
        .as_ref()
        .ok_or("No image loaded. Call editor_load_image first.")?;

    // Use the new LSD + RANSAC + VP straightening algorithm
    // Pass the original path for EXIF focal length extraction
    let image_path = Path::new(&cached.path);
    let result = analyze_straighten(&cached.preview_image, Some(image_path));

    Ok(AutoStraightenResult {
        angle: result.suggested_rotation as f32,
        confidence: result.confidence,
    })
}

// ============================================================================
// Batch Auto-Enhance Commands
// ============================================================================

use crate::perspective::{
    AdjustmentAnalysis, EnhanceAnalysisResult, EnhanceApplyResult, EnhanceRequest,
    StraightenAnalysis,
};

/// Analyze all images in a property's INTERNET folder for batch enhancement.
/// Returns analysis results with before/after previews for each image.
#[tauri::command]
pub async fn batch_analyze_for_enhance(
    app: AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<EnhanceAnalysisResult>, String> {
    use crate::perspective::straighten::analyze_straighten;

    // Build the INTERNET folder path - load config async
    let config = crate::config::load_config(app.clone())
        .await?
        .ok_or("App configuration not found. Please configure in Settings.")?;

    // Get the correct base folder path based on status
    let base_folder_path = match status.as_str() {
        "NEW" => &config.new_folder_path,
        "DONE" => &config.done_folder_path,
        "NOT_FOUND" => &config.not_found_folder_path,
        "ARCHIVE" => &config.archive_folder_path,
        _ => return Err(format!("Unknown status: {status}")),
    };

    if base_folder_path.is_empty() {
        return Err(format!(
            "Folder path for status '{status}' is not configured. Please set it in Settings."
        ));
    }

    let internet_path = std::path::Path::new(base_folder_path)
        .join(&folder_path)
        .join("INTERNET");

    if !internet_path.exists() {
        return Err(format!(
            "INTERNET folder not found: {}",
            internet_path.display()
        ));
    }

    // List all image files
    let image_extensions = ["jpg", "jpeg", "png", "webp", "bmp", "gif"];
    let mut image_paths: Vec<std::path::PathBuf> = std::fs::read_dir(&internet_path)
        .map_err(|e| format!("Failed to read INTERNET folder: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| image_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename for consistent ordering
    image_paths.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });

    // Process images in parallel using rayon
    let results: Vec<Result<EnhanceAnalysisResult, String>> = image_paths
        .par_iter()
        .map(|path| {
            // Load the image
            let img = image::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;

            // Analyze for straightening
            let straighten_result = analyze_straighten(&img, Some(path));

            // Analyze for auto-adjustments
            let adjustments = analyze_image_histogram(&img);

            // Calculate adjustment magnitude (normalized 0-1)
            let adj_magnitude = ((adjustments.brightness.abs() as f32 / 100.0).powi(2)
                + (adjustments.exposure.abs() as f32 / 100.0).powi(2)
                + (adjustments.contrast.abs() as f32 / 100.0).powi(2)
                + (adjustments.highlights.abs() as f32 / 100.0).powi(2)
                + (adjustments.shadows.abs() as f32 / 100.0).powi(2))
            .sqrt()
                / 2.24; // normalize by sqrt(5) for max possible value

            // Determine if enhancement is needed
            let needs_straighten =
                straighten_result.suggested_rotation.abs() > 0.3 && straighten_result.confidence > 0.4;
            let needs_adjustment = adjustments.brightness.abs() > 10
                || adjustments.exposure.abs() > 10
                || adjustments.contrast.abs() > 10
                || adjustments.highlights.abs() > 10
                || adjustments.shadows.abs() > 10;
            let needs_enhancement = needs_straighten || needs_adjustment;

            // Calculate combined confidence
            let rotation_weight = if straighten_result.suggested_rotation.abs() > 0.5 {
                0.6
            } else {
                0.3
            };
            let combined_confidence = straighten_result.confidence * rotation_weight
                + adj_magnitude.min(1.0) * (1.0 - rotation_weight);

            // Generate preview image with edits applied
            let preview_size: u32 = 600;
            let preview_img = resize_for_preview(&img, preview_size);

            // Build EditParams for preview
            let preview_params = EditParams {
                fine_rotation: straighten_result.suggested_rotation as f32,
                brightness: adjustments.brightness,
                exposure: adjustments.exposure,
                contrast: adjustments.contrast,
                highlights: adjustments.highlights,
                shadows: adjustments.shadows,
                ..EditParams::default()
            };

            // Apply edits to generate enhanced preview
            let enhanced_preview = apply_all_edits_fast(&preview_img, &preview_params)
                .map_err(|e| format!("Failed to generate preview: {e}"))?;

            // Encode both previews to base64
            let preview_base64 = encode_to_base64_jpeg(&enhanced_preview)?;
            let original_preview_base64 = encode_to_base64_jpeg(&preview_img)?;

            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            Ok(EnhanceAnalysisResult {
                filename,
                original_path: path.to_string_lossy().to_string(),
                straighten: StraightenAnalysis {
                    rotation: straighten_result.suggested_rotation,
                    confidence: straighten_result.confidence,
                    lines_used: straighten_result.lines_used,
                    vh_agreement: straighten_result.vh_agreement,
                },
                adjustments: AdjustmentAnalysis {
                    brightness: adjustments.brightness,
                    exposure: adjustments.exposure,
                    contrast: adjustments.contrast,
                    highlights: adjustments.highlights,
                    shadows: adjustments.shadows,
                    magnitude: adj_magnitude,
                },
                combined_confidence,
                needs_enhancement,
                preview_base64,
                original_preview_base64,
            })
        })
        .collect();

    // Collect results, filtering out errors but logging them
    let mut final_results = Vec::new();
    for result in results {
        match result {
            Ok(r) => final_results.push(r),
            Err(e) => eprintln!("Warning: Failed to analyze image: {e}"),
        }
    }

    Ok(final_results)
}

/// Apply enhancements to selected images, overwriting the originals.
#[tauri::command]
pub async fn batch_apply_enhancements(
    enhancements: Vec<EnhanceRequest>,
) -> Result<Vec<EnhanceApplyResult>, String> {
    let results: Vec<EnhanceApplyResult> = enhancements
        .par_iter()
        .map(|request| {
            let result = (|| -> Result<(), String> {
                // Load the original image at full resolution
                let img = image::open(&request.original_path)
                    .map_err(|e| format!("Failed to open image: {e}"))?;

                // Build EditParams
                let params = EditParams {
                    fine_rotation: request.rotation as f32,
                    brightness: request.brightness,
                    exposure: request.exposure,
                    contrast: request.contrast,
                    highlights: request.highlights,
                    shadows: request.shadows,
                    ..EditParams::default()
                };

                // Apply all edits at full resolution
                let edited = apply_all_edits(&img, &params)?;

                // Determine output format from original file extension
                let path = std::path::Path::new(&request.original_path);
                let format = get_image_format(path)?;

                // Save over the original
                save_image(&edited, path, format)?;

                Ok(())
            })();

            EnhanceApplyResult {
                filename: request.filename.clone(),
                success: result.is_ok(),
                error: result.err(),
            }
        })
        .collect();

    Ok(results)
}

// ============================================================================
// Image Processing Pipeline
// ============================================================================

/// Apply all edits in the correct order (used for saving at full resolution)
pub fn apply_all_edits(img: &DynamicImage, params: &EditParams) -> Result<DynamicImage, String> {
    let mut result = img.clone();

    // 1. Apply quarter-turn rotations (0°, 90°, 180°, 270°) - lossless
    result = apply_quarter_rotation(&result, params.quarter_turns);

    // 2. Apply fine rotation with auto-crop (if non-zero)
    if params.fine_rotation.abs() > 0.01 {
        result = apply_fine_rotation(&result, params.fine_rotation)?;
    }

    // 3. Apply crop (if enabled)
    if params.crop_enabled {
        result = apply_crop(&result, params)?;
    }

    // 4. Apply adjustments
    result = apply_adjustments(&result, params);

    Ok(result)
}

/// Fast version of apply_all_edits optimized for preview generation.
/// Uses parallel pixel processing for adjustments.
fn apply_all_edits_fast(img: &DynamicImage, params: &EditParams) -> Result<DynamicImage, String> {
    let mut result = img.clone();

    // 1. Apply quarter-turn rotations (0°, 90°, 180°, 270°) - lossless
    result = apply_quarter_rotation(&result, params.quarter_turns);

    // 2. Apply fine rotation with auto-crop (if non-zero)
    if params.fine_rotation.abs() > 0.01 {
        result = apply_fine_rotation(&result, params.fine_rotation)?;
    }

    // 3. Apply crop (if enabled)
    if params.crop_enabled {
        result = apply_crop(&result, params)?;
    }

    // 4. Apply adjustments using parallel processing
    result = apply_adjustments_parallel(&result, params);

    Ok(result)
}

// ============================================================================
// Rotation
// ============================================================================

/// Apply 90-degree rotation increments (lossless)
fn apply_quarter_rotation(img: &DynamicImage, turns: u8) -> DynamicImage {
    match turns % 4 {
        0 => img.clone(),
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => unreachable!(),
    }
}

/// Apply fine rotation (arbitrary angle) with mathematical auto-crop to remove black borders
/// Uses the same formula as the WebGL shader for consistent preview/save results
fn apply_fine_rotation(img: &DynamicImage, angle_degrees: f32) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();
    let aspect = width as f32 / height as f32;

    // Calculate the auto-crop scale factor (same formula as WebGL shader)
    // For a rectangle with aspect ratio 'a', rotated by angle θ:
    // cropScale = min(1/(cos + sin/a), 1/(cos + sin*a))
    let abs_angle = angle_degrees.abs().to_radians();
    let cos_a = abs_angle.cos();
    let sin_a = abs_angle.sin();

    let scale_from_width = 1.0 / (cos_a + sin_a / aspect);
    let scale_from_height = 1.0 / (cos_a + sin_a * aspect);
    let crop_scale = scale_from_width.min(scale_from_height);

    // Output image dimensions (preserves aspect ratio)
    let new_width = ((width as f32) * crop_scale).round() as u32;
    let new_height = ((height as f32) * crop_scale).round() as u32;

    // Ensure minimum dimensions
    let new_width = new_width.max(1);
    let new_height = new_height.max(1);

    // Create the output image at the cropped size
    let mut result = RgbaImage::new(new_width, new_height);

    let angle_radians = angle_degrees.to_radians();
    let cos_r = angle_radians.cos();
    let sin_r = angle_radians.sin();

    // Half dimensions for the inscribed rectangle in NORMALIZED coordinates
    // (same as WebGL: cropHalfWidth = aspect * cropScale * 0.5, cropHalfHeight = cropScale * 0.5)
    let crop_half_width = aspect * crop_scale * 0.5;
    let crop_half_height = crop_scale * 0.5;

    let src_cx = width as f32 / 2.0;
    let src_cy = height as f32 / 2.0;
    let dst_cx = new_width as f32 / 2.0;
    let dst_cy = new_height as f32 / 2.0;

    let rgba = img.to_rgba8();

    // For each pixel in the output, find the corresponding pixel in the rotated source
    for dst_y in 0..new_height {
        for dst_x in 0..new_width {
            // Convert output pixel to normalized coordinates within the inscribed rectangle
            // Output spans [0, new_width] x [0, new_height]
            // Normalized inscribed rect spans [-cropHalfWidth, +cropHalfWidth] x [-cropHalfHeight, +cropHalfHeight]
            let u = (dst_x as f32 - dst_cx) / dst_cx; // -1 to +1
            let v = (dst_y as f32 - dst_cy) / dst_cy; // -1 to +1

            let x_norm = u * crop_half_width;
            let y_norm = v * crop_half_height;

            // Apply inverse rotation in normalized space (same as WebGL shader)
            let x_rot = x_norm * cos_r + y_norm * sin_r;
            let y_rot = -x_norm * sin_r + y_norm * cos_r;

            // Convert normalized coordinates back to source pixel coordinates
            // In normalized space, source image spans [-aspect/2, +aspect/2] x [-0.5, +0.5]
            // So: src_x = (x_rot / (aspect/2)) * (width/2) + width/2 = x_rot/aspect * width + width/2
            let src_x = x_rot / aspect * (width as f32) + src_cx;
            let src_y = y_rot * (height as f32) + src_cy;

            // Bilinear interpolation
            if src_x >= 0.0
                && src_x < (width - 1) as f32
                && src_y >= 0.0
                && src_y < (height - 1) as f32
            {
                let pixel = bilinear_sample(&rgba, src_x, src_y);
                result.put_pixel(dst_x, dst_y, pixel);
            } else {
                // Out of bounds - shouldn't happen with proper auto-crop, but use black as fallback
                result.put_pixel(dst_x, dst_y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    Ok(DynamicImage::ImageRgba8(result))
}

/// Bilinear interpolation sampling
fn bilinear_sample(img: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = img.get_pixel(x0, y0);
    let p10 = img.get_pixel(x1, y0);
    let p01 = img.get_pixel(x0, y1);
    let p11 = img.get_pixel(x1, y1);

    let mut result = [0u8; 4];
    for i in 0..4 {
        let v00 = p00[i] as f32;
        let v10 = p10[i] as f32;
        let v01 = p01[i] as f32;
        let v11 = p11[i] as f32;

        let v = v00 * (1.0 - fx) * (1.0 - fy)
            + v10 * fx * (1.0 - fy)
            + v01 * (1.0 - fx) * fy
            + v11 * fx * fy;

        result[i] = v.round().clamp(0.0, 255.0) as u8;
    }

    Rgba(result)
}

// ============================================================================
// Cropping
// ============================================================================

/// Apply crop using normalized coordinates (0-1)
fn apply_crop(img: &DynamicImage, params: &EditParams) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();

    // Convert normalized coordinates to pixel coordinates
    let x = (params.crop_x * width as f32) as u32;
    let y = (params.crop_y * height as f32) as u32;
    let crop_width = (params.crop_width * width as f32) as u32;
    let crop_height = (params.crop_height * height as f32) as u32;

    // Ensure we don't exceed image bounds
    let x = x.min(width.saturating_sub(1));
    let y = y.min(height.saturating_sub(1));
    let crop_width = crop_width.min(width - x).max(1);
    let crop_height = crop_height.min(height - y).max(1);

    Ok(img.crop_imm(x, y, crop_width, crop_height))
}

// ============================================================================
// Adjustments
// ============================================================================

/// GLSL-style smoothstep function for smooth interpolation
#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Apply all image adjustments
fn apply_adjustments(img: &DynamicImage, params: &EditParams) -> DynamicImage {
    // Skip if all adjustments are zero
    if params.brightness == 0
        && params.exposure == 0
        && params.contrast == 0
        && params.highlights == 0
        && params.shadows == 0
    {
        return img.clone();
    }

    let mut rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Pre-compute adjustment factors (calibrated to match Windows 11 Photo Editor)
    // These values match the WebGL shader exactly
    let brightness_factor = params.brightness as f32 / 350.0;      // -0.29 to 0.29 (softer)
    let exposure_factor = 2.0_f32.powf(params.exposure as f32 / 130.0); // -0.77 to 0.77 f-stops
    let contrast_factor = (params.contrast as f32 + 170.0) / 170.0; // 0.41 to 1.59 (softer)
    let highlight_adjust = params.highlights as f32 / 180.0;       // -0.56 to 0.56 (softer)
    let shadow_adjust = params.shadows as f32 / 180.0;             // -0.56 to 0.56 (softer)

    for y in 0..height {
        for x in 0..width {
            let pixel = rgba.get_pixel_mut(x, y);

            // Convert to 0-1 range
            let mut r = pixel[0] as f32 / 255.0;
            let mut g = pixel[1] as f32 / 255.0;
            let mut b = pixel[2] as f32 / 255.0;

            // 1. Exposure: multiplicative (simulates f-stops) - apply first for most natural results
            r *= exposure_factor;
            g *= exposure_factor;
            b *= exposure_factor;

            // 2. Brightness: additive
            r += brightness_factor;
            g += brightness_factor;
            b += brightness_factor;

            // 3. Contrast: pivot around 0.5
            r = (r - 0.5) * contrast_factor + 0.5;
            g = (g - 0.5) * contrast_factor + 0.5;
            b = (b - 0.5) * contrast_factor + 0.5;

            // 4. Highlights/Shadows: luminance-based masking with smoothstep transition
            if highlight_adjust != 0.0 || shadow_adjust != 0.0 {
                let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                // Smooth mask transition for more natural look (matches WebGL shader)
                let highlight_mask = smoothstep(0.3, 0.7, luminance);
                let shadow_mask = 1.0 - highlight_mask;

                let adjustment = highlight_adjust * highlight_mask + shadow_adjust * shadow_mask;
                // Scale down for subtler effect (matches WebGL shader)
                r += adjustment * 0.5;
                g += adjustment * 0.5;
                b += adjustment * 0.5;
            }

            // Clamp and convert back to u8
            pixel[0] = (r.clamp(0.0, 1.0) * 255.0) as u8;
            pixel[1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
            pixel[2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
            // Alpha channel remains unchanged
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Adjustment factors pre-computed for parallel processing
struct AdjustmentFactors {
    brightness: f32,
    exposure: f32,
    contrast: f32,
    highlights: f32,
    shadows: f32,
}

impl AdjustmentFactors {
    fn from_params(params: &EditParams) -> Self {
        // Ranges calibrated to match Windows 11 Photo Editor behavior
        // These values match the WebGL shader exactly
        Self {
            brightness: params.brightness as f32 / 350.0,      // -0.29 to 0.29 (softer)
            exposure: 2.0_f32.powf(params.exposure as f32 / 130.0), // -0.77 to 0.77 f-stops (~0.59x to 1.7x)
            contrast: (params.contrast as f32 + 170.0) / 170.0, // 0.41 to 1.59 (softer)
            highlights: params.highlights as f32 / 180.0,      // -0.56 to 0.56 (softer)
            shadows: params.shadows as f32 / 180.0,            // -0.56 to 0.56 (softer)
        }
    }

    fn is_identity(&self, params: &EditParams) -> bool {
        params.brightness == 0
            && params.exposure == 0
            && params.contrast == 0
            && params.highlights == 0
            && params.shadows == 0
    }
}

/// Process a single pixel with the given adjustment factors
#[inline]
fn process_pixel(pixel: Rgba<u8>, factors: &AdjustmentFactors) -> Rgba<u8> {
    // Convert to 0-1 range
    let mut r = pixel[0] as f32 / 255.0;
    let mut g = pixel[1] as f32 / 255.0;
    let mut b = pixel[2] as f32 / 255.0;

    // 1. Exposure: multiplicative (simulates f-stops) - apply first for most natural results
    r *= factors.exposure;
    g *= factors.exposure;
    b *= factors.exposure;

    // 2. Brightness: additive
    r += factors.brightness;
    g += factors.brightness;
    b += factors.brightness;

    // 3. Contrast: pivot around 0.5
    r = (r - 0.5) * factors.contrast + 0.5;
    g = (g - 0.5) * factors.contrast + 0.5;
    b = (b - 0.5) * factors.contrast + 0.5;

    // 4. Highlights/Shadows: luminance-based masking with smoothstep transition
    if factors.highlights != 0.0 || factors.shadows != 0.0 {
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        // Smooth mask transition for more natural look (matches WebGL shader)
        let highlight_mask = smoothstep(0.3, 0.7, luminance);
        let shadow_mask = 1.0 - highlight_mask;

        let adjustment = factors.highlights * highlight_mask + factors.shadows * shadow_mask;
        // Scale down for subtler effect (matches WebGL shader)
        r += adjustment * 0.5;
        g += adjustment * 0.5;
        b += adjustment * 0.5;
    }

    // Clamp and convert back to u8
    Rgba([
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
        pixel[3], // Alpha unchanged
    ])
}

/// Apply all image adjustments using parallel processing (rayon)
fn apply_adjustments_parallel(img: &DynamicImage, params: &EditParams) -> DynamicImage {
    let factors = AdjustmentFactors::from_params(params);

    // Skip if all adjustments are zero
    if factors.is_identity(params) {
        return img.clone();
    }

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Process all pixels in parallel using rayon
    let pixels: Vec<Rgba<u8>> = rgba
        .pixels()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|p| process_pixel(**p, &factors))
        .collect();

    // Reconstruct the image
    let mut result = RgbaImage::new(width, height);
    for (i, pixel) in pixels.into_iter().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        result.put_pixel(x, y, pixel);
    }

    DynamicImage::ImageRgba8(result)
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Resize image for preview while maintaining aspect ratio
fn resize_for_preview(img: &DynamicImage, max_size: u32) -> DynamicImage {
    let (width, height) = img.dimensions();
    let max_dim = width.max(height);

    if max_dim <= max_size {
        return img.clone();
    }

    let scale = max_size as f32 / max_dim as f32;
    let new_width = (width as f32 * scale) as u32;
    let new_height = (height as f32 * scale) as u32;

    img.resize_exact(
        new_width.max(1),
        new_height.max(1),
        image::imageops::FilterType::Triangle,
    )
}

/// Encode image to base64 JPEG
fn encode_to_base64_jpeg(img: &DynamicImage) -> Result<String, String> {
    let mut buffer = Cursor::new(Vec::new());

    // Convert to RGB for JPEG (no alpha channel)
    let rgb = img.to_rgb8();

    rgb.write_to(&mut buffer, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode image: {e}"))?;

    Ok(STANDARD.encode(buffer.into_inner()))
}

/// Get the image format from file extension
fn get_image_format(path: &Path) -> Result<ImageFormat, String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .ok_or("Could not determine file extension")?;

    match extension.as_str() {
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        "gif" => Ok(ImageFormat::Gif),
        "webp" => Ok(ImageFormat::WebP),
        "bmp" => Ok(ImageFormat::Bmp),
        _ => Ok(ImageFormat::Jpeg), // Default to JPEG
    }
}

/// Save image to disk
fn save_image(img: &DynamicImage, path: &Path, format: ImageFormat) -> Result<(), String> {
    // For JPEG, convert to RGB (no alpha)
    if format == ImageFormat::Jpeg {
        let rgb = img.to_rgb8();
        rgb.save_with_format(path, format)
            .map_err(|e| format!("Failed to save image: {e}"))?;
    } else {
        img.save_with_format(path, format)
            .map_err(|e| format!("Failed to save image: {e}"))?;
    }

    Ok(())
}
