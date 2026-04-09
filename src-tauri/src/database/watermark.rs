//! Watermark application pipeline.
//! All commands and helpers for: copying property images into the
//! WATERMARK / WATERMARK/AGGELIA folders with watermarks applied;
//! generating live previews for the settings page; listing /
//! clearing the watermark output folders.
//!
//! Extracted from database.rs in the database-module split. Heavy
//! image-processing helpers (apply_watermark_with_config, blend,
//! tile, single placement, size computation) live here too because
//! nothing outside this module ever called them.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use rayon::prelude::*;
use tauri::Manager;

use crate::database::get_property_base_path;
use crate::database::types::CommandResult;
use crate::gpu::ImageProcessor;

/// Delete every image file (jpg/jpeg/png/bmp/gif/heic/webp) directly inside
/// `folder_path`. Used by `clear_watermark_folders` to wipe a previous
/// watermarking pass before re-running it.
fn clear_folder_images(folder_path: &PathBuf) -> Result<usize, String> {
    let mut deleted_count = 0;

    for entry in fs::read_dir(folder_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                    deleted_count += 1;
                }
            }
        }
    }

    Ok(deleted_count)
}

#[tauri::command]
pub async fn copy_and_watermark_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    // Load watermark from app data
    let watermark_path = crate::config::get_watermark_from_app_data(app.clone())
        .await
        .map_err(|e| e.to_string())?;
    let watermark_img_path = match watermark_path {
        Some(path) => PathBuf::from(path),
        None => {
            return Ok(CommandResult {
                success: false,
                error: Some(
                    "Watermark image not configured. Please set it in settings first.".to_string(),
                ),
                data: None,
            })
        }
    };

    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    // Get GPU processor for accelerated watermark blending
    let processor = app.state::<Arc<ImageProcessor>>();
    let processor_ref = processor.inner().clone();
    let wm_config = config.watermark_config.clone();

    // All filesystem + image processing runs on a blocking thread
    let (processed_count, errors) = tokio::task::spawn_blocking(move || {
        let internet_path = property_path.join("INTERNET");
        let aggelia_path = internet_path.join("AGGELIA");
        let watermark_path = property_path.join("WATERMARK");
        let watermark_aggelia_path = watermark_path.join("AGGELIA");

        // Ensure WATERMARK folders exist
        fs::create_dir_all(&watermark_path)
            .map_err(|e| format!("Failed to create WATERMARK folder: {}", e))?;
        fs::create_dir_all(&watermark_aggelia_path)
            .map_err(|e| format!("Failed to create WATERMARK/AGGELIA folder: {}", e))?;

        // Load watermark image once (turbojpeg for JPEG, image crate for others)
        let watermark_img = crate::turbo::load_image(&watermark_img_path)
            .map_err(|e| format!("Failed to load watermark image: {}", e))?;

        let mut processed_count = 0usize;
        let mut errors = Vec::new();

        // Process INTERNET folder -> WATERMARK folder
        if internet_path.exists() {
            match copy_and_process_folder_with_config(
                &internet_path,
                &watermark_path,
                &watermark_img,
                &wm_config,
                &processor_ref,
            ) {
                Ok(count) => processed_count += count,
                Err(e) => errors.push(format!("INTERNET folder: {}", e)),
            }
        }

        // Process INTERNET/AGGELIA folder -> WATERMARK/AGGELIA folder
        if aggelia_path.exists() {
            match copy_and_process_folder_with_config(
                &aggelia_path,
                &watermark_aggelia_path,
                &watermark_img,
                &wm_config,
                &processor_ref,
            ) {
                Ok(count) => processed_count += count,
                Err(e) => errors.push(format!("AGGELIA folder: {}", e)),
            }
        }

        Ok::<_, String>((processed_count, errors))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "processed_count": processed_count,
                "message": format!("Successfully processed and watermarked {} images", processed_count)
            })),
        })
    } else {
        Ok(CommandResult {
            success: processed_count > 0,
            error: Some(format!(
                "Processed {} images but encountered errors: {}",
                processed_count,
                errors.join(", ")
            )),
            data: Some(serde_json::json!({
                "processed_count": processed_count,
                "errors": errors
            })),
        })
    }
}

fn copy_and_process_folder_with_config(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    watermark_img: &DynamicImage,
    config: &crate::config::WatermarkConfig,
    processor: &Arc<ImageProcessor>,
) -> Result<usize, String> {
    use std::collections::HashMap;

    // Collect all image files first
    let image_files: Vec<(PathBuf, PathBuf)> = fs::read_dir(source_path)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_lc = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "bmp", "gif", "webp"].contains(&ext_lc.as_str()) {
                        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                            return Some((path.clone(), dest_path.join(filename)));
                        }
                    }
                }
            }
            None
        })
        .collect();

    let processed_count = AtomicUsize::new(0);
    let errors: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    // Cache resized watermarks by target (base image) dimensions.
    // Real estate photos from the same camera are all the same size, so this
    // eliminates N-1 redundant watermark resizes (typically 19/20+ are cache hits).
    let wm_cache: std::sync::Mutex<HashMap<(u32, u32), RgbaImage>> =
        std::sync::Mutex::new(HashMap::new());

    // Process images in parallel using rayon
    let processor = processor.clone();
    image_files.par_iter().for_each(
        |(source, dest)| match apply_watermark_to_image_with_cached_wm(
            source,
            dest,
            watermark_img,
            config,
            &processor,
            &wm_cache,
        ) {
            Ok(_) => {
                processed_count.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                if let Some(filename) = source.file_name().and_then(|s| s.to_str()) {
                    if let Ok(mut errs) = errors.lock() {
                        errs.push(format!("Failed to process {}: {}", filename, e));
                    }
                }
            }
        },
    );

    // Check for errors
    if let Ok(errs) = errors.lock() {
        if !errs.is_empty() {
            return Err(errs.join("; "));
        }
    }

    Ok(processed_count.load(Ordering::Relaxed))
}

/// Apply watermark with a shared cache for the resized watermark.
/// The cache avoids redundant SIMD resize operations when all images have the same dimensions.
fn apply_watermark_to_image_with_cached_wm(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    watermark_img: &DynamicImage,
    config: &crate::config::WatermarkConfig,
    processor: &Arc<ImageProcessor>,
    wm_cache: &std::sync::Mutex<std::collections::HashMap<(u32, u32), RgbaImage>>,
) -> Result<(), String> {
    // Load source image using turbojpeg
    let mut base_img = crate::turbo::load_image(source_path)
        .map_err(|e| format!("Failed to open source image: {}", e))?
        .to_rgba8();

    let base_dims = base_img.dimensions();

    // Check cache for pre-resized watermark matching this base image size
    let cached_wm = {
        let guard = wm_cache.lock().map_err(|e| format!("Lock error: {e}"))?;
        guard.get(&base_dims).cloned()
    };

    if let Some(resized_wm) = cached_wm {
        // Cache hit — apply directly without resize
        if config.size_mode == "tile" {
            apply_tiled_watermark(&mut base_img, &resized_wm, config, processor)?;
        } else {
            apply_single_watermark(&mut base_img, &resized_wm, config, processor)?;
        }
    } else {
        // Cache miss — resize, cache, and apply
        apply_watermark_with_config(&mut base_img, watermark_img, config, processor)?;

        // Cache the resized watermark for future images with same dimensions
        // We need to compute the resized version again for caching (it's computed inside
        // apply_watermark_with_config). For simplicity, compute the target size and cache.
        let (wm_width, wm_height) = watermark_img.dimensions();
        let (new_wm_width, new_wm_height) =
            compute_watermark_size(base_dims, (wm_width, wm_height), config);
        let resized_wm =
            crate::fast_resize::resize_exact(watermark_img, new_wm_width, new_wm_height).to_rgba8();
        let mut guard = wm_cache.lock().map_err(|e| format!("Lock error: {e}"))?;
        guard.entry(base_dims).or_insert(resized_wm);
    }

    // Save watermarked image
    let ext = dest_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "jpg" || ext == "jpeg" {
        // Use turbojpeg for fast JPEG encoding
        let rgb_img: image::RgbImage = image::DynamicImage::ImageRgba8(base_img).to_rgb8();
        crate::turbo::save_jpeg(&rgb_img, dest_path, 92)?;
    } else {
        base_img
            .save(dest_path)
            .map_err(|e| format!("Failed to save watermarked image: {}", e))?;
    }

    Ok(())
}

// Unused legacy entry point kept until database.rs is split into modules,
// at which point it will be deleted. Marked dead_code rather than removed
// in this commit to keep the diff focused.
#[tauri::command]
pub async fn generate_watermark_preview(
    app: tauri::AppHandle,
    sample_image_base64: Option<String>,
) -> Result<String, String> {
    // Load watermark config
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("No configuration found".into()),
    };

    // Load watermark image from app data
    let watermark_path = crate::config::get_watermark_from_app_data(app.clone())
        .await
        .map_err(|e| e.to_string())?;
    let watermark_path = match watermark_path {
        Some(path) => PathBuf::from(path),
        None => return Err("No watermark image configured".into()),
    };

    let processor = app.state::<Arc<ImageProcessor>>();
    let processor_ref = processor.inner().clone();
    let wm_config = config.watermark_config.clone();

    // All image I/O + processing runs on a blocking thread
    let base64_result = tokio::task::spawn_blocking(move || {
        let watermark_img = crate::turbo::load_image(&watermark_path)
            .map_err(|e| format!("Failed to load watermark: {}", e))?;

        // Create or use sample image
        let mut base_img = if let Some(base64_data) = sample_image_base64 {
            // Decode base64 and load as image
            let image_data = general_purpose::STANDARD
                .decode(base64_data)
                .map_err(|e| format!("Failed to decode base64: {}", e))?;
            image::load_from_memory(&image_data)
                .map_err(|e| format!("Failed to load image from memory: {}", e))?
                .to_rgba8()
        } else {
            // Create a sample gray image (800x600)
            let mut img = RgbaImage::new(800, 600);
            for pixel in img.pixels_mut() {
                *pixel = image::Rgba([200, 200, 200, 255]);
            }
            img
        };

        // Apply watermark using current config (GPU-accelerated)
        apply_watermark_with_config(&mut base_img, &watermark_img, &wm_config, &processor_ref)?;

        // Encode result as base64
        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        base_img
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| format!("Failed to encode image: {}", e))?;

        Ok::<_, String>(general_purpose::STANDARD.encode(&buffer))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    Ok(base64_result)
}

/// Compute watermark target dimensions based on config and base image size.
/// Extracted so it can be used for both the apply function and the cache key.
fn compute_watermark_size(
    base_dims: (u32, u32),
    wm_dims: (u32, u32),
    config: &crate::config::WatermarkConfig,
) -> (u32, u32) {
    let (base_width, base_height) = base_dims;
    let (wm_width, wm_height) = wm_dims;

    match config.size_mode.as_str() {
        "proportional" => {
            let reference_size = match config.relative_to.as_str() {
                "longest-side" => base_width.max(base_height),
                "shortest-side" => base_width.min(base_height),
                "width" => base_width,
                "height" => base_height,
                _ => base_width.max(base_height),
            };

            let max_size = (reference_size as f32 * config.size_percentage) as u32;
            let scale_x = max_size as f32 / wm_width as f32;
            let scale_y = max_size as f32 / wm_height as f32;
            let scale = scale_x.min(scale_y);

            (
                (wm_width as f32 * scale) as u32,
                (wm_height as f32 * scale) as u32,
            )
        }
        "fit" => {
            let scale_x = base_width as f32 / wm_width as f32;
            let scale_y = base_height as f32 / wm_height as f32;
            let scale = scale_x.min(scale_y).min(1.0);

            (
                (wm_width as f32 * scale) as u32,
                (wm_height as f32 * scale) as u32,
            )
        }
        "stretch" => (base_width, base_height),
        "tile" => (wm_width, wm_height),
        _ => {
            let max_size = (base_width.max(base_height) as f32 * config.size_percentage) as u32;
            let scale = max_size as f32 / wm_width.max(wm_height) as f32;
            (
                (wm_width as f32 * scale) as u32,
                (wm_height as f32 * scale) as u32,
            )
        }
    }
}

fn apply_watermark_with_config(
    base_img: &mut RgbaImage,
    watermark_img: &DynamicImage,
    config: &crate::config::WatermarkConfig,
    processor: &Arc<ImageProcessor>,
) -> Result<(), String> {
    let base_dims = base_img.dimensions();
    let wm_dims = watermark_img.dimensions();

    let (new_wm_width, new_wm_height) = compute_watermark_size(base_dims, wm_dims, config);

    // Resize watermark using SIMD-accelerated fast_image_resize
    let resized_watermark =
        crate::fast_resize::resize_exact(watermark_img, new_wm_width, new_wm_height).to_rgba8();

    // Apply watermark based on mode (GPU-accelerated)
    if config.size_mode == "tile" {
        apply_tiled_watermark(base_img, &resized_watermark, config, processor)?;
    } else {
        apply_single_watermark(base_img, &resized_watermark, config, processor)?;
    }

    Ok(())
}

fn apply_single_watermark(
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    config: &crate::config::WatermarkConfig,
    processor: &Arc<ImageProcessor>,
) -> Result<(), String> {
    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark.dimensions();

    // Calculate position based on anchor
    let (base_x, base_y) = match config.position_anchor.as_str() {
        "top-left" => (0, 0),
        "top-center" => ((base_width - wm_width) / 2, 0),
        "top-right" => (base_width - wm_width, 0),
        "center-left" => (0, (base_height - wm_height) / 2),
        "center" => ((base_width - wm_width) / 2, (base_height - wm_height) / 2),
        "center-right" => (base_width - wm_width, (base_height - wm_height) / 2),
        "bottom-left" => (0, base_height - wm_height),
        "bottom-center" => ((base_width - wm_width) / 2, base_height - wm_height),
        "bottom-right" => (base_width - wm_width, base_height - wm_height),
        _ => ((base_width - wm_width) / 2, (base_height - wm_height) / 2),
    };

    // Apply offsets
    let pos_x = (base_x as i32 + config.offset_x)
        .max(0)
        .min(base_width as i32 - wm_width as i32) as u32;
    let pos_y = (base_y as i32 + config.offset_y)
        .max(0)
        .min(base_height as i32 - wm_height as i32) as u32;

    // Apply watermark with opacity (GPU-accelerated)
    processor.blend_watermark(
        base_img,
        watermark,
        pos_x,
        pos_y,
        config.opacity,
        config.use_alpha_channel,
    );

    Ok(())
}

fn apply_tiled_watermark(
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    config: &crate::config::WatermarkConfig,
    processor: &Arc<ImageProcessor>,
) -> Result<(), String> {
    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark.dimensions();

    let mut y = 0;
    while y < base_height {
        let mut x = 0;
        while x < base_width {
            processor.blend_watermark(
                base_img,
                watermark,
                x,
                y,
                config.opacity,
                config.use_alpha_channel,
            );
            x += wm_width + config.offset_x.unsigned_abs();
        }
        y += wm_height + config.offset_y.unsigned_abs();
    }

    Ok(())
}

#[allow(dead_code)]
fn blend_watermark(
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    pos_x: u32,
    pos_y: u32,
    opacity: f32,
    use_alpha: bool,
) {
    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark.dimensions();

    for y in 0..wm_height {
        for x in 0..wm_width {
            let base_x = pos_x + x;
            let base_y = pos_y + y;

            if base_x < base_width && base_y < base_height {
                let base_pixel = base_img.get_pixel_mut(base_x, base_y);
                let wm_pixel = watermark.get_pixel(x, y);

                let wm_alpha = if use_alpha {
                    (wm_pixel[3] as f32 / 255.0 * opacity).min(1.0)
                } else {
                    opacity
                };

                // Alpha blend
                for c in 0..3 {
                    let base_val = base_pixel[c] as f32 / 255.0;
                    let wm_val = wm_pixel[c] as f32 / 255.0;
                    let blended = base_val * (1.0 - wm_alpha) + wm_val * wm_alpha;
                    base_pixel[c] = (blended * 255.0) as u8;
                }
            }
        }
    }
}

#[allow(dead_code)]
fn apply_watermark_to_image(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    watermark_img: &DynamicImage,
    opacity: f32,
) -> Result<(), String> {
    // Load source image
    let mut base_img = crate::turbo::load_image(source_path)
        .map_err(|e| format!("Failed to open source image: {}", e))?
        .to_rgba8();

    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark_img.dimensions();

    // Calculate scale to fit watermark (max 50% of image width/height)
    let max_wm_width = base_width / 2;
    let max_wm_height = base_height / 2;

    let scale_x = max_wm_width as f32 / wm_width as f32;
    let scale_y = max_wm_height as f32 / wm_height as f32;
    let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

    let new_wm_width = (wm_width as f32 * scale) as u32;
    let new_wm_height = (wm_height as f32 * scale) as u32;

    // Resize watermark using SIMD-accelerated fast_image_resize
    let resized_watermark =
        crate::fast_resize::resize_exact(&watermark_img, new_wm_width, new_wm_height).to_rgba8();

    // Calculate center position
    let pos_x = (base_width - new_wm_width) / 2;
    let pos_y = (base_height - new_wm_height) / 2;

    // Apply watermark with opacity
    for y in 0..new_wm_height {
        for x in 0..new_wm_width {
            let base_x = pos_x + x;
            let base_y = pos_y + y;

            if base_x < base_width && base_y < base_height {
                let base_pixel = base_img.get_pixel_mut(base_x, base_y);
                let wm_pixel = resized_watermark.get_pixel(x, y);

                // Apply opacity to watermark alpha
                let wm_alpha = (wm_pixel[3] as f32 / 255.0 * opacity).min(1.0);

                // Alpha blend
                for c in 0..3 {
                    let base_val = base_pixel[c] as f32 / 255.0;
                    let wm_val = wm_pixel[c] as f32 / 255.0;
                    let blended = base_val * (1.0 - wm_alpha) + wm_val * wm_alpha;
                    base_pixel[c] = (blended * 255.0) as u8;
                }
            }
        }
    }

    // Save watermarked image - convert to RGB for JPEG (doesn't support alpha)
    let ext = dest_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "jpg" || ext == "jpeg" {
        // Convert RGBA to RGB for JPEG format
        let rgb_img: image::RgbImage = image::DynamicImage::ImageRgba8(base_img).to_rgb8();
        rgb_img
            .save(dest_path)
            .map_err(|e| format!("Failed to save watermarked image: {}", e))?;
    } else {
        base_img
            .save(dest_path)
            .map_err(|e| format!("Failed to save watermarked image: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn list_watermark_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    tokio::task::spawn_blocking(move || {
        let watermark_path = property_path.join("WATERMARK");

        if !watermark_path.exists() {
            return Ok(Vec::new());
        }

        let mut images = Vec::new();
        for entry in fs::read_dir(watermark_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_lc = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"]
                        .contains(&ext_lc.as_str())
                    {
                        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                            images.push(filename.to_string());
                        }
                    }
                }
            }
        }

        images.sort();
        Ok(images)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn list_watermark_aggelia_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    tokio::task::spawn_blocking(move || {
        let watermark_aggelia_path = property_path.join("WATERMARK").join("AGGELIA");

        if !watermark_aggelia_path.exists() {
            return Ok(Vec::new());
        }

        let mut images = Vec::new();
        for entry in fs::read_dir(watermark_aggelia_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_lc = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"]
                        .contains(&ext_lc.as_str())
                    {
                        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                            images.push(filename.to_string());
                        }
                    }
                }
            }
        }

        images.sort();
        Ok(images)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn clear_watermark_folders(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    tokio::task::spawn_blocking(move || {
        let watermark_path = property_path.join("WATERMARK");
        let mut deleted_count = 0;
        let mut errors = Vec::new();

        if watermark_path.exists() {
            match clear_folder_images(&watermark_path) {
                Ok(count) => deleted_count += count,
                Err(e) => errors.push(format!("WATERMARK folder: {}", e)),
            }

            let aggelia_path = watermark_path.join("AGGELIA");
            if aggelia_path.exists() {
                match clear_folder_images(&aggelia_path) {
                    Ok(count) => deleted_count += count,
                    Err(e) => errors.push(format!("WATERMARK/AGGELIA folder: {}", e)),
                }
            }
        }

        if errors.is_empty() {
            Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "deleted_count": deleted_count,
                    "message": format!("Successfully deleted {} images from WATERMARK folders", deleted_count)
                })),
            })
        } else {
            Ok(CommandResult {
                success: deleted_count > 0,
                error: Some(format!(
                    "Deleted {} images but encountered errors: {}",
                    deleted_count,
                    errors.join(", ")
                )),
                data: None,
            })
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}
