//! Tauri commands for perspective correction feature.
//!
//! Uses LSD (Line Segment Detection) + RANSAC for automatic image straightening.

use crate::perspective::detection::analyze_perspective;
use crate::perspective::model::{cleanup_temp_files, ensure_temp_dir_for_property};
use crate::perspective::rectification::{apply_correction, generate_correction_preview};
use crate::perspective::{AcceptedCorrection, CorrectionResult, PerspectiveCommandResult};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use image::{DynamicImage, GenericImageView};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

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
    let config = crate::config::load_config(app.clone())
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

    // Process each image
    let mut results = Vec::new();

    println!("Processing {} images from: {}", images.len(), internet_path.display());

    for (idx, image_path) in images.iter().enumerate() {
        let filename = image_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        println!("Processing image {}/{}: {}", idx + 1, images.len(), filename);

        match process_single_image(image_path, &temp_dir) {
            Ok(result) => {
                println!("  -> Success: rotation={:.2}°, needs_correction={}",
                    result.rotation_applied, result.needs_correction);
                results.push(CorrectionResult {
                    original_filename: filename,
                    original_path: image_path.to_string_lossy().to_string(),
                    ..result
                });
            },
            Err(e) => {
                // Log error but continue with other images
                eprintln!("  -> Failed to process {filename}: {e}");
                results.push(CorrectionResult {
                    original_filename: filename,
                    original_path: image_path.to_string_lossy().to_string(),
                    corrected_temp_path: String::new(),
                    confidence: 0.0,
                    rotation_applied: 0.0,
                    needs_correction: false,
                    corrected_preview_base64: None,
                });
            }
        }
    }

    println!("Finished processing. {} results.", results.len());
    Ok(results)
}

/// Process a single image for perspective correction
fn process_single_image(
    image_path: &PathBuf,
    temp_dir: &PathBuf,
) -> Result<CorrectionResult, String> {
    // Load the image
    let img = image::open(image_path)
        .map_err(|e| format!("Failed to open image: {e}"))?;

    // Analyze for perspective distortion using LSD + RANSAC
    let analysis = analyze_perspective(&img)?;

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

/// Encode an image to base64 JPEG
fn encode_image_to_base64(img: &DynamicImage) -> Result<String, String> {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode image: {e}"))?;

    Ok(BASE64_STANDARD.encode(&buffer))
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

    let img = image::open(&path)
        .map_err(|e| format!("Failed to open image: {e}"))?;

    // Resize for preview
    let (width, height) = img.dimensions();
    let scale = f64::from(PREVIEW_MAX_SIZE) / f64::from(width.max(height));

    let preview = if scale < 1.0 {
        let new_width = (f64::from(width) * scale) as u32;
        let new_height = (f64::from(height) * scale) as u32;
        img.resize_exact(
            new_width.max(1),
            new_height.max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };

    encode_image_to_base64(&preview)
}
