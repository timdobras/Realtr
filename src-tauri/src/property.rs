use std::fs;
use std::path::{Path, PathBuf};

#[tauri::command]
pub async fn list_original_images(folder_path: String) -> Result<Vec<String>, String> {
    // Here, folder_path is relative to root folder selected by user
    // We'll get the root folder from config, then join with folder_path

    // Load config to get root path
    let app = tauri::AppHandle::current();
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(c) => PathBuf::from(c.root_path),
        None => return Err("App root folder not configured".into()),
    };

    let full_path = root_path.join(&folder_path);
    if !full_path.exists() || !full_path.is_dir() {
        return Err(format!("Folder not found: {}", full_path.display()));
    }

    let mut images = Vec::new();

    for entry in fs::read_dir(full_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            // Filter image file extensions (you can extend this list)
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ext_lc == "jpg" || ext_lc == "jpeg" || ext_lc == "png" || ext_lc == "bmp" || ext_lc == "gif" || ext_lc == "heic" {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        images.push(filename.to_string());
                    }
                }
            }
        }
    }

    Ok(images)
}