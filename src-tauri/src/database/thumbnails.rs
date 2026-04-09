//! Thumbnail generation + serving via the Tauri asset protocol.
//! Frontend uses convertFileSrc(get_gallery_thumbnail_path(...)) for
//! every thumbnail in the property list and step grids — no base64.
//!
//! Extracted from database.rs in the database-module split.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;
use tauri::Manager;

use crate::database::types::{
    CommandResult, ThumbnailBatchRequest, ThumbnailBatchResult,
};
use crate::database::get_property_base_path;

// generate_thumbnail is private to this module — only the thumbnail commands
// in this file call it. Made pub(super) so the rest of the database module
// (specifically pregenerate_gallery_thumbnails callers via tests, if any)
// could reach it, but currently nothing else does.

// Helper function to generate a thumbnail from an image.
// For JPEG files, uses DCT-scaled decoding (turbojpeg) which decodes at reduced
// resolution directly from the frequency domain — dramatically faster than full decode + resize.
// For non-JPEG files, falls back to full decode + SIMD resize.
fn generate_thumbnail(
    source_path: &PathBuf,
    thumbnail_path: &PathBuf,
    max_size: u32,
) -> Result<(), String> {
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();

    let img = if ext == "jpg" || ext == "jpeg" {
        // DCT-scaled decode: for a 6000x4000 JPEG targeting 400px, decodes at 1/4 scale
        // (1500x1000) in ~15ms instead of full decode (~200ms). The remaining resize
        // from 1500→400 is trivial.
        crate::turbo::load_jpeg_scaled(source_path.as_path(), max_size)
            .map_err(|e| format!("Failed to load JPEG thumbnail: {}", e))?
    } else {
        crate::turbo::load_image(source_path)
            .map_err(|e| format!("Failed to open image: {}", e))?
    };

    // Resize to exact target using SIMD-accelerated fast_image_resize
    // (for DCT-scaled JPEGs this is a small resize; for others it's the full resize)
    let thumbnail = crate::fast_resize::resize_to_fit(&img, max_size);

    // Save the thumbnail as JPEG using turbojpeg
    crate::turbo::save_jpeg(&thumbnail.to_rgb8(), thumbnail_path, 85)?;

    Ok(())
}

#[tauri::command]
pub async fn list_thumbnails(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    // List original images and return their names as .jpg (thumbnail format)
    // The thumbnails will be generated on-demand when requested
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    tokio::task::spawn_blocking(move || {
        let mut thumbnails = Vec::new();
        for entry in fs::read_dir(&property_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_lc = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            thumbnails.push(format!("{}.jpg", stem));
                        }
                    }
                }
            }
        }

        thumbnails.sort();
        Ok(thumbnails)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

/// Get a gallery-sized thumbnail for workflow step displays.
/// This is larger than the property list thumbnails (400px vs 100px)
/// and supports different subfolders (INTERNET, AGGELIA, WATERMARK, etc.)
///
/// Deprecated: frontend now uses get_gallery_thumbnail_path + convertFileSrc.
/// Get the filesystem path to a gallery-sized thumbnail (generating it if needed).
/// Returns the absolute path string instead of base64 data — the frontend uses
/// `convertFileSrc(path)` to serve the file directly via Tauri's asset protocol,
/// avoiding base64 encoding/decoding overhead entirely.
#[tauri::command]
pub async fn get_gallery_thumbnail_path(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    subfolder: String,
    filename: String,
    max_dimension: Option<u32>,
) -> Result<String, String> {
    let max_size = max_dimension.unwrap_or(400);

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;

    let thumbnails_base = app_data_dir.join("thumbnails").join(format!("gallery_{max_size}"));
    let safe_folder_name = folder_path.replace('/', "_").replace('\\', "_");
    let safe_subfolder = if subfolder.is_empty() {
        "root".to_string()
    } else {
        subfolder.replace('/', "_").replace('\\', "_")
    };
    let thumbnails_dir = thumbnails_base.join(&safe_folder_name).join(&safe_subfolder);
    let thumbnail_path = thumbnails_dir.join(&filename).with_extension("jpg");

    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    tokio::task::spawn_blocking(move || {
        let source_dir = if subfolder.is_empty() {
            property_path
        } else {
            property_path.join(&subfolder)
        };
        let source_path = source_dir.join(&filename);

        if !source_path.exists() {
            return Err(format!("Source image not found: {}", source_path.display()));
        }

        // Check if we need to regenerate the thumbnail
        let needs_regeneration = if !thumbnail_path.exists() {
            true
        } else {
            let source_modified = fs::metadata(&source_path)
                .and_then(|m| m.modified())
                .ok();
            let thumb_modified = fs::metadata(&thumbnail_path)
                .and_then(|m| m.modified())
                .ok();

            match (source_modified, thumb_modified) {
                (Some(src_time), Some(thumb_time)) => src_time > thumb_time,
                _ => true,
            }
        };

        if needs_regeneration {
            fs::create_dir_all(&thumbnails_dir)
                .map_err(|e| format!("Failed to create thumbnails directory: {e}"))?;

            generate_thumbnail(&source_path, &thumbnail_path, max_size)
                .map_err(|e| format!("Failed to generate gallery thumbnail: {e}"))?;
        }

        // Return the absolute path — frontend will use convertFileSrc() to serve it
        Ok(thumbnail_path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

/// Batch-resolve thumbnail paths for the properties list.
/// For each property, lists original images and returns up to `limit` thumbnail paths,
/// generating thumbnails on-demand if they don't exist. Returns paths instead of base64
/// so the frontend can use `convertFileSrc()` for zero-copy serving.
#[tauri::command]
pub async fn get_thumbnail_paths_batch(
    app: tauri::AppHandle,
    properties: Vec<ThumbnailBatchRequest>,
) -> Result<Vec<ThumbnailBatchResult>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let thumbnails_base = app_data_dir.join("thumbnails");

    // Resolve all property base paths (needs async config access)
    let mut resolved: Vec<(ThumbnailBatchRequest, PathBuf)> = Vec::with_capacity(properties.len());
    for prop in properties {
        match get_property_base_path(&app, &prop.folder_path, &prop.status).await {
            Ok(base_path) => resolved.push((prop, base_path)),
            Err(_) => {
                // Skip properties whose base path can't be resolved
            }
        }
    }

    tokio::task::spawn_blocking(move || {
        let supported_exts = ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"];

        let results: Vec<ThumbnailBatchResult> = resolved
            .into_iter()
            .map(|(prop, property_path)| {
                let limit = prop.limit.unwrap_or(6);
                let safe_folder_name = prop.folder_path.replace('/', "_").replace('\\', "_");
                let thumb_dir = thumbnails_base.join(&safe_folder_name);

                // List original images
                let mut originals: Vec<(String, PathBuf)> = Vec::new();
                if let Ok(entries) = fs::read_dir(&property_path) {
                    for entry in entries.filter_map(Result::ok) {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if supported_exts.contains(&ext.to_lowercase().as_str()) {
                                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                        originals.push((stem.to_string(), path.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                originals.sort_by(|a, b| a.0.cmp(&b.0));
                let total_count = originals.len();

                // Generate/resolve up to `limit` thumbnail paths
                let mut paths: Vec<String> = Vec::new();
                for (stem, source_path) in originals.into_iter().take(limit) {
                    let thumb_filename = format!("{stem}.jpg");
                    let thumbnail_path = thumb_dir.join(&thumb_filename);

                    if !thumbnail_path.exists() {
                        let _ = fs::create_dir_all(&thumb_dir);
                        if generate_thumbnail(&source_path, &thumbnail_path, 100).is_err() {
                            continue;
                        }
                    }

                    paths.push(thumbnail_path.to_string_lossy().to_string());
                }

                ThumbnailBatchResult {
                    folder_path: prop.folder_path,
                    total_count,
                    paths,
                }
            })
            .collect();

        Ok(results)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

/// Pre-generate gallery thumbnails for all images in a subfolder.
/// This runs in parallel for faster generation.
#[tauri::command]
pub async fn pregenerate_gallery_thumbnails(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    subfolder: String,
    max_dimension: Option<u32>,
) -> Result<CommandResult, String> {
    let max_size = max_dimension.unwrap_or(400);

    // Get app data directory for gallery thumbnails
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    // Get the source directory
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let source_dir = if subfolder.is_empty() {
        property_path.clone()
    } else {
        property_path.join(&subfolder)
    };

    // All filesystem + thumbnail generation runs on a blocking thread
    tokio::task::spawn_blocking(move || {
        if !source_dir.exists() {
            return Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({"generated": 0, "cached": 0})),
            });
        }

        // Get list of image files
        let supported_extensions = ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"];
        let mut filenames: Vec<String> = Vec::new();

        if let Ok(entries) = fs::read_dir(&source_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if supported_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                        if let Some(name) = path.file_name() {
                            filenames.push(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        if filenames.is_empty() {
            return Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({"generated": 0, "cached": 0})),
            });
        }

        // Setup thumbnail directory
        let thumbnails_base = app_data_dir.join("thumbnails").join(format!("gallery_{}", max_size));
        let safe_folder_name = folder_path.replace('/', "_").replace('\\', "_");
        let safe_subfolder = if subfolder.is_empty() {
            "root".to_string()
        } else {
            subfolder.replace('/', "_").replace('\\', "_")
        };
        let thumbnails_dir = thumbnails_base.join(&safe_folder_name).join(&safe_subfolder);
        fs::create_dir_all(&thumbnails_dir)
            .map_err(|e| format!("Failed to create thumbnails directory: {}", e))?;

        // Filter to only files that need generation (new or modified)
        let mut to_generate: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut cached_count = 0;

        for filename in &filenames {
            let thumbnail_path = thumbnails_dir.join(filename).with_extension("jpg");
            let source_path = source_dir.join(filename);

            if !source_path.exists() {
                continue;
            }

            // Check if thumbnail exists and is up-to-date
            let needs_generation = if !thumbnail_path.exists() {
                true
            } else {
                // Compare modification times
                let source_modified = fs::metadata(&source_path)
                    .and_then(|m| m.modified())
                    .ok();
                let thumb_modified = fs::metadata(&thumbnail_path)
                    .and_then(|m| m.modified())
                    .ok();

                match (source_modified, thumb_modified) {
                    (Some(src_time), Some(thumb_time)) => src_time > thumb_time,
                    _ => true,
                }
            };

            if needs_generation {
                to_generate.push((source_path, thumbnail_path));
            } else {
                cached_count += 1;
            }
        }

        if to_generate.is_empty() {
            return Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({"generated": 0, "cached": cached_count})),
            });
        }

        // Generate thumbnails in parallel using rayon (already on blocking thread)
        let generated_count = AtomicUsize::new(0);
        to_generate.par_iter().for_each(|(source_path, thumbnail_path)| {
            if generate_thumbnail(source_path, thumbnail_path, max_size).is_ok() {
                generated_count.fetch_add(1, Ordering::Relaxed);
            }
        });

        let generated_count = generated_count.load(Ordering::Relaxed);

        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "generated": generated_count,
                "cached": cached_count,
                "total": filenames.len()
            })),
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}
