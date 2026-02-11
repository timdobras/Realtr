//! Tauri commands for perspective correction feature.
//!
//! Uses GPU gradient histogram + RANSAC for automatic image straightening.

use crate::gpu::ImageProcessor;
use crate::perspective::detection::analyze_perspective;
use crate::perspective::model::{cleanup_temp_files, ensure_temp_dir_for_property};
use crate::perspective::rectification::{apply_correction, generate_correction_preview};
use crate::perspective::{AcceptedCorrection, CorrectionResult, PerspectiveCommandResult};
use image::{DynamicImage, GenericImageView};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

/// Supported image extensions
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp"];

/// Preview size for before/after display
const PREVIEW_MAX_SIZE: u32 = 800;

/// Get the property base path using the new folder configuration
async fn get_property_base_path(
    app: &tauri::AppHandle,
    folder_path: &str,
    status: &str,
) -> Result<PathBuf, String> {
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?;
    let config = config.ok_or("App configuration not found")?;

    // Get the base folder path based on status
    let base_folder = match status.to_uppercase().as_str() {
        "NEW" => {
            if config.new_folder_path.is_empty() {
                return Err("NEW folder path not configured in Settings".to_string());
            }
            &config.new_folder_path
        }
        "DONE" => {
            if config.done_folder_path.is_empty() {
                return Err("DONE folder path not configured in Settings".to_string());
            }
            &config.done_folder_path
        }
        "ARCHIVE" => {
            if config.archive_folder_path.is_empty() {
                return Err("ARCHIVE folder path not configured in Settings".to_string());
            }
            &config.archive_folder_path
        }
        "NOT_FOUND" => {
            if config.not_found_folder_path.is_empty() {
                return Err("NOT_FOUND folder path not configured in Settings".to_string())
            }
            &config.not_found_folder_path
        }
        _ => return Err(format!("Unknown status: {status}")),
    };

    Ok(PathBuf::from(base_folder).join(folder_path))
}

/// Process all images in the INTERNET folder for perspective correction
/// Returns a list of correction results with before/after previews
#[tauri::command]
pub async fn process_images_for_perspective(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    property_id: i64,
) -> Result<Vec<CorrectionResult>, String> {
    // Get the INTERNET folder path
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let internet_path = property_path.join("INTERNET");

    if !internet_path.exists() {
        return Err("INTERNET folder not found. Please copy images to INTERNET first.".to_string());
    }

    // Ensure temp directory exists for this property
    let temp_dir = ensure_temp_dir_for_property(&app, property_id)?;

    // List all images in INTERNET folder
    let mut images: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&internet_path).map_err(|e| format!("Failed to read INTERNET folder: {e}"))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    images.push(path);
                }
            }
        }
    }

    images.sort();

    // Process images in parallel with limited concurrency
    // to prevent RAM spikes (each image loads ~50-100MB at full resolution)
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .map_err(|e| format!("Failed to create thread pool: {e}"))?;

    println!("Processing {} images from: {} (parallel, 4 threads)", images.len(), internet_path.display());

    // Get GPU image processor from Tauri state
    let processor = app.state::<Arc<ImageProcessor>>();
    let processor_ref = processor.inner().clone();

    let results: Vec<CorrectionResult> = pool.install(|| {
        images
            .par_iter()
            .enumerate()
            .map(|(idx, image_path)| {
                let filename = image_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                println!("Processing image {}/{}: {}", idx + 1, images.len(), filename);

                match process_single_image(image_path, &temp_dir, &processor_ref) {
                    Ok(result) => {
                        println!(
                            "  -> Success: rotation={:.2}, needs_correction={}",
                            result.rotation_applied, result.needs_correction
                        );
                        CorrectionResult {
                            original_filename: filename,
                            original_path: image_path.to_string_lossy().to_string(),
                            ..result
                        }
                    }
                    Err(e) => {
                        eprintln!("  -> Failed to process {filename}: {e}");
                        CorrectionResult {
                            original_filename: filename,
                            original_path: image_path.to_string_lossy().to_string(),
                            corrected_temp_path: String::new(),
                            confidence: 0.0,
                            rotation_applied: 0.0,
                            needs_correction: false,
                            corrected_preview_base64: None,
                        }
                    }
                }
            })
            .collect()
    });

    println!("Finished processing. {} results.", results.len());
    Ok(results)
}

/// Process a single image for perspective correction
fn process_single_image(
    image_path: &PathBuf,
    temp_dir: &PathBuf,
    processor: &ImageProcessor,
) -> Result<CorrectionResult, String> {
    // Load the image using turbojpeg for JPEG files
    let img = crate::turbo::load_image(image_path)
        .map_err(|e| format!("Failed to open image: {e}"))?;

    // Analyze for perspective distortion using gradient histogram + RANSAC
    let analysis = analyze_perspective(&img, processor)?;

    // Generate corrected image (or use original if no correction needed)
    let corrected = if analysis.needs_correction {
        apply_correction(&img, &analysis)?
    } else {
        img.clone()
    };

    // Save corrected image to temp directory
    let filename = image_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("image.jpg");
    let temp_path = temp_dir.join(format!("corrected_{filename}"));

    corrected
        .save(&temp_path)
        .map_err(|e| format!("Failed to save corrected image: {e}"))?;

    // Generate base64 preview of corrected image
    let preview = generate_correction_preview(&corrected, &analysis, PREVIEW_MAX_SIZE)?;
    let preview_base64 = encode_image_to_base64(&preview)?;

    Ok(CorrectionResult {
        original_filename: filename.to_string(),
        original_path: image_path.to_string_lossy().to_string(),
        corrected_temp_path: temp_path.to_string_lossy().to_string(),
        confidence: analysis.confidence,
        rotation_applied: analysis.suggested_rotation,
        needs_correction: analysis.needs_correction,
        corrected_preview_base64: Some(preview_base64),
    })
}

/// Encode an image to base64 JPEG using turbojpeg
fn encode_image_to_base64(img: &DynamicImage) -> Result<String, String> {
    crate::turbo::encode_jpeg_base64(&img.to_rgb8(), 85)
}

/// Accept selected corrections - overwrite originals with corrected versions
#[tauri::command]
pub async fn accept_perspective_corrections(
    app: tauri::AppHandle,
    corrections: Vec<AcceptedCorrection>,
) -> Result<PerspectiveCommandResult, String> {
    let mut success_count = 0;
    let mut errors = Vec::new();

    for correction in &corrections {
        let temp_path = PathBuf::from(&correction.corrected_temp_path);
        let original_path = PathBuf::from(&correction.original_path);

        if !temp_path.exists() {
            errors.push(format!(
                "Temp file not found: {}",
                correction.corrected_temp_path
            ));
            continue;
        }

        // Copy corrected image over original (using copy then delete for safety)
        match fs::copy(&temp_path, &original_path) {
            Ok(_) => {
                success_count += 1;
                // Clean up temp file
                let _ = fs::remove_file(&temp_path);
            }
            Err(e) => {
                errors.push(format!(
                    "Failed to save {}: {e}",
                    original_path.display()
                ));
            }
        }
    }

    // Clean up any remaining temp files
    let _ = cleanup_temp_files(&app);

    if errors.is_empty() {
        Ok(PerspectiveCommandResult::success(&format!(
            "Successfully applied {success_count} corrections"
        )))
    } else {
        Ok(PerspectiveCommandResult {
            success: success_count > 0,
            error: Some(errors.join("; ")),
            message: Some(format!(
                "Applied {success_count} corrections with {} errors",
                errors.len()
            )),
        })
    }
}

/// Clean up temporary perspective correction files
#[tauri::command]
pub async fn cleanup_perspective_temp(app: tauri::AppHandle) -> Result<(), String> {
    cleanup_temp_files(&app)
}

/// Get the original image as base64 for before/after comparison
#[tauri::command]
pub async fn get_original_image_for_comparison(
    image_path: String,
) -> Result<String, String> {
    let path = PathBuf::from(&image_path);

    if !path.exists() {
        return Err(format!("Image not found: {image_path}"));
    }

    let img = crate::turbo::load_image(&path)
        .map_err(|e| format!("Failed to open image: {e}"))?;

    // Resize for preview
    let (width, height) = img.dimensions();
    let scale = f64::from(PREVIEW_MAX_SIZE) / f64::from(width.max(height));

    let preview = if scale < 1.0 {
        crate::fast_resize::resize_to_fit(&img, PREVIEW_MAX_SIZE)
    } else {
        img
    };

    encode_image_to_base64(&preview)
}
