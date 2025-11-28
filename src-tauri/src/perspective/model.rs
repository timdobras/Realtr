//! Temporary file management for perspective correction.
//!
//! Handles creation and cleanup of temp directories used during
//! the perspective correction workflow.

use std::path::PathBuf;
use tauri::Manager;

/// Get the temp directory for perspective corrections
pub fn get_perspective_temp_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    Ok(app_data_dir.join("perspective_temp"))
}

/// Clean up temporary perspective correction files
pub fn cleanup_temp_files(app: &tauri::AppHandle) -> Result<(), String> {
    let temp_dir = get_perspective_temp_dir(app)?;

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to clean up temp directory: {e}"))?;
    }

    Ok(())
}

/// Ensure the temp directory exists and is clean for a property
pub fn ensure_temp_dir_for_property(
    app: &tauri::AppHandle,
    property_id: i64,
) -> Result<PathBuf, String> {
    let temp_dir = get_perspective_temp_dir(app)?;
    let property_temp_dir = temp_dir.join(property_id.to_string());

    // Clean up existing temp files for this property
    if property_temp_dir.exists() {
        std::fs::remove_dir_all(&property_temp_dir)
            .map_err(|e| format!("Failed to clean existing temp files: {e}"))?;
    }

    // Create fresh temp directory
    std::fs::create_dir_all(&property_temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {e}"))?;

    Ok(property_temp_dir)
}
