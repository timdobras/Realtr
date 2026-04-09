//! External-editor / open-in-folder commands. These wrap the OS-level
//! "open this file/folder in another application" calls (custom editor
//! paths from config, plus cmd / open / xdg-open fallbacks).
//!
//! Extracted from database.rs in the database-module split.

use tauri::Manager;
use tokio::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x080_0000;

use crate::database::types::CommandResult;
use crate::database::get_property_base_path;

#[tauri::command]
pub async fn open_images_in_folder(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    selected_image: String,
) -> Result<CommandResult, String> {
    // Get the full absolute path using the property base path
    let full_folder_path = get_property_base_path(&app, &folder_path, &status).await?;

    // Collect image paths on a blocking thread
    let full_folder_clone = full_folder_path.clone();
    let mut image_paths = tokio::task::spawn_blocking(move || {
        if !full_folder_clone.exists() || !full_folder_clone.is_dir() {
            return Err(format!(
                "Folder path does not exist: {}",
                full_folder_clone.display()
            ));
        }

        let mut paths = Vec::new();
        for entry in std::fs::read_dir(&full_folder_clone).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                    let ext = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext.as_str()) {
                        paths.push(path);
                    }
                }
            }
        }
        Ok(paths)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    if image_paths.is_empty() {
        return Ok(CommandResult {
            success: false,
            error: Some("No images found in folder".into()),
            data: None,
        });
    }

    // Sort paths and prioritize the selected image
    image_paths.sort();
    let selected_path = full_folder_path.join(&selected_image);

    // Reorder so selected image is first
    let mut ordered_paths = Vec::new();
    if image_paths.contains(&selected_path) {
        ordered_paths.push(selected_path.clone());
    }
    for path in &image_paths {
        if *path != selected_path {
            ordered_paths.push(path.clone());
        }
    }

    // Convert paths to strings
    let paths_strs: Vec<String> = ordered_paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect();

    if paths_strs.is_empty() {
        return Ok(CommandResult {
            success: false,
            error: Some("Failed to process image paths".into()),
            data: None,
        });
    }

    // Open images based on operating system
    let result = if cfg!(target_os = "windows") {
        // For Windows, first unblock the file to remove Zone.Identifier (security warning trigger)
        // Then open it with the default application
        let file_path = &paths_strs[0];

        // Unblock the file using PowerShell (removes "downloaded from internet" marking)
        let _ = Command::new("powershell")
            .args(["-Command", &format!("Unblock-File -Path \"{}\"", file_path)])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .await; // Run and ignore result (file might not be blocked)

        // Now open with default application using start command
        Command::new("cmd")
            .args(["/C", "start", "", file_path])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    } else if cfg!(target_os = "macos") {
        // macOS can handle multiple files
        Command::new("open").args(&paths_strs).spawn()
    } else {
        // Linux - open just the selected image
        Command::new("xdg-open").arg(&paths_strs[0]).spawn()
    };

    match result {
        Ok(_) => Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "opened_images": paths_strs.len(),
                "selected_image": selected_image
            })),
        }),
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to open images: {}", e)),
            data: None,
        }),
    }
}

#[tauri::command]
pub async fn open_image_in_editor(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
    is_from_internet: bool,
) -> Result<CommandResult, String> {
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let image_path = if is_from_internet {
        property_path.join("INTERNET").join(&filename)
    } else {
        property_path.join(&filename)
    };

    let img_path = image_path.clone();
    let exists = tokio::task::spawn_blocking(move || img_path.exists())
        .await
        .map_err(|e| format!("Task join error: {e}"))?;

    if !exists {
        return Ok(CommandResult {
            success: false,
            error: Some(format!("Image file not found: {}", image_path.display())),
            data: None,
        });
    }

    // On Windows, unblock the file to remove security warning
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("powershell")
            .args(["-Command", &format!("Unblock-File -Path \"{}\"", image_path.display())])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .await;
    }

    // Use configured fast editor or system default. PathBuf implements
    // AsRef<OsStr> so we pass it directly and avoid lossy to_str() unwraps
    // on non-UTF8 Windows paths.
    let result = if let Some(editor_path) = &config.fast_editor_path {
        // Use custom fast editor
        Command::new(editor_path).arg(&image_path).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(&image_path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(&image_path).spawn()
    } else {
        Command::new("xdg-open").arg(&image_path).spawn()
    };

    match result {
        Ok(_) => Ok(CommandResult {
            success: true,
            error: None,
            data: None,
        }),
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to open image in editor: {}", e)),
            data: None,
        }),
    }
}

#[tauri::command]
pub async fn open_image_in_advanced_editor(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
    from_aggelia: bool,
) -> Result<CommandResult, String> {
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let image_path = if from_aggelia {
        property_path
            .join("INTERNET")
            .join("AGGELIA")
            .join(&filename)
    } else {
        property_path
            .join("INTERNET")
            .join(&filename)
    };

    let img_path = image_path.clone();
    let exists = tokio::task::spawn_blocking(move || img_path.exists())
        .await
        .map_err(|e| format!("Task join error: {e}"))?;

    if !exists {
        return Ok(CommandResult {
            success: false,
            error: Some(format!("Image file not found: {}", image_path.display())),
            data: None,
        });
    }

    // On Windows, unblock the file to remove security warning
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("powershell")
            .args(["-Command", &format!("Unblock-File -Path \"{}\"", image_path.display())])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .await;
    }

    // Use configured complex editor or system default. See note in
    // open_image_in_editor — passing &PathBuf avoids to_str() unwraps.
    let result = if let Some(editor_path) = &config.complex_editor_path {
        Command::new(editor_path).arg(&image_path).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(&image_path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(&image_path).spawn()
    } else {
        Command::new("xdg-open").arg(&image_path).spawn()
    };

    match result {
        Ok(_) => Ok(CommandResult {
            success: true,
            error: None,
            data: None,
        }),
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to open image in advanced editor: {}", e)),
            data: None,
        }),
    }
}

#[tauri::command]
pub async fn open_property_folder(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let full_path = get_property_base_path(&app, &folder_path, &status).await?;

    println!("Attempting to open: {}", full_path.display());

    tokio::task::spawn_blocking(move || {
        match opener::open(&full_path) {
            Ok(_) => Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "opened_path": full_path.to_string_lossy()
                })),
            }),
            Err(e) => Ok(CommandResult {
                success: false,
                error: Some(format!("Failed to open folder: {}", e)),
                data: None,
            }),
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn get_full_property_path(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let full_path = get_property_base_path(&app, &folder_path, &status).await?;

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::json!({
            "full_path": full_path.to_string_lossy(),
            "relative_path": folder_path
        })),
    })
}
