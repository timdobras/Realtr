//! Set commands — completing a property batch into a zip + DB rows,
//! and the read/list/delete/open helpers used by the Sets page.
//! Also owns the `add_directory_to_zip` helper since it's only used here.
//!
//! Extracted from database.rs in the database-module split.

use std::fs;
use std::path::PathBuf;

use serde_json::json;
use sqlx::Row;

use crate::database::types::{CommandResult, CompleteSetResult, Property, Set, SetProperty};
use crate::database::{folder_path_to_pathbuf, get_base_path_for_status, get_database_pool};

/// Recursively add a directory to a ZIP file. Used by `complete_set`.
fn add_directory_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir_path: &std::path::Path,
    base_path: &std::path::Path,
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    use walkdir::WalkDir;

    for entry in WalkDir::new(dir_path) {
        let entry = entry.map_err(|e| format!("Failed to walk directory: {}", e))?;
        let path = entry.path();

        let relative_path = path
            .strip_prefix(base_path)
            .map_err(|e| format!("Failed to strip prefix: {}", e))?;

        let relative_path_str = relative_path
            .to_str()
            .ok_or("Invalid path encoding")?
            .replace('\\', "/");

        if path.is_dir() {
            if !relative_path_str.is_empty() {
                let dir_name = format!("{}/", relative_path_str);
                zip.add_directory(&dir_name, options)
                    .map_err(|e| format!("Failed to add directory to ZIP: {}", e))?;
            }
        } else {
            zip.start_file(&relative_path_str, options)
                .map_err(|e| format!("Failed to start file in ZIP: {}", e))?;

            let file_content = std::fs::read(path)
                .map_err(|e| format!("Failed to read file {}: {}", path.display(), e))?;

            use std::io::Write;
            zip.write_all(&file_content)
                .map_err(|e| format!("Failed to write file to ZIP: {}", e))?;
        }
    }

    Ok(())
}

/// Complete a set: ZIP all DONE properties with codes, move to ARCHIVE,
/// move properties without codes to NOT_FOUND
#[tauri::command]
pub async fn complete_set(app: tauri::AppHandle) -> Result<CompleteSetResult, String> {
    let pool = get_database_pool(&app)?;

    // Load config
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("App configuration not found")?;

    // Validate sets folder path is configured
    if config.sets_folder_path.is_empty() {
        return Err("Sets folder path is not configured. Please configure it in Settings.".to_string());
    }

    let sets_folder = PathBuf::from(&config.sets_folder_path);
    let sets_folder_clone = sets_folder.clone();
    tokio::task::spawn_blocking(move || {
        if !sets_folder_clone.exists() {
            std::fs::create_dir_all(&sets_folder_clone)
                .map_err(|e| format!("Failed to create sets folder: {}", e))?;
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    // Get all DONE properties
    let done_properties: Vec<Property> = sqlx::query_as::<_, (
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        i64,
    )>(
        "SELECT id, name, city, status, folder_path, notes, code, created_at, updated_at
         FROM properties WHERE status = 'DONE'"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch DONE properties: {}", e))?
    .into_iter()
    .map(|(id, name, city, status, folder_path, notes, code, created_at, updated_at)| Property {
        id: Some(id),
        name,
        city,
        status,
        folder_path,
        notes,
        code,
        created_at: chrono::DateTime::from_timestamp_millis(created_at)
            .unwrap_or_else(chrono::Utc::now),
        updated_at: chrono::DateTime::from_timestamp_millis(updated_at)
            .unwrap_or_else(chrono::Utc::now),
        completed: None,
    })
    .collect();

    // Separate properties by whether they have a code
    let (with_code, without_code): (Vec<_>, Vec<_>) = done_properties
        .into_iter()
        .partition(|p| p.code.as_ref().is_some_and(|c| !c.is_empty()));

    if with_code.is_empty() {
        return Err("No DONE properties with codes found to create a set.".to_string());
    }

    // Create ZIP file (heavy I/O — runs on blocking thread)
    let now = chrono::Local::now();
    let set_name = format!("Done - {}", now.format("%Y-%m-%d %H-%M-%S"));
    let zip_filename = format!("{}.zip", set_name);
    let zip_path = sets_folder.join(&zip_filename);

    let done_base_path = get_base_path_for_status(&config, "DONE")?;
    {
        let zip_path = zip_path.clone();
        let done_base_path = done_base_path.clone();
        let with_code_ref: Vec<_> = with_code.iter().map(|p| (p.folder_path.clone(), p.city.clone())).collect();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::create(&zip_path)
                .map_err(|e| format!("Failed to create ZIP file: {}", e))?;

            let mut zip = zip::ZipWriter::new(file);
            // Use Stored (no compression) instead of Deflated for speed
            // Photos are already compressed (JPEG/PNG), so deflate provides minimal benefit
            // but takes much longer. Stored mode is ~10x faster with minimal size increase.
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            // Add each property to the ZIP
            for (folder_path, city) in &with_code_ref {
                let property_path = done_base_path.join(folder_path_to_pathbuf(folder_path));

                if property_path.exists() {
                    let city_folder = format!("{}/", city);
                    let _ = zip.add_directory(&city_folder, options);

                    add_directory_to_zip(
                        &mut zip,
                        &property_path,
                        &done_base_path,
                        options,
                    )?;
                }
            }

            zip.finish()
                .map_err(|e| format!("Failed to finish ZIP file: {}", e))?;
            Ok::<_, String>(())
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))??;
    }

    // Insert set record into database
    let now_timestamp = chrono::Utc::now().timestamp_millis();
    let zip_path_str = zip_path.to_string_lossy().to_string();

    let set_id = sqlx::query(
        "INSERT INTO sets (name, zip_path, property_count, created_at) VALUES (?, ?, ?, ?)"
    )
    .bind(&set_name)
    .bind(&zip_path_str)
    .bind(with_code.len() as i64)
    .bind(now_timestamp)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to insert set record: {}", e))?
    .last_insert_rowid();

    // Insert set_properties records
    for property in &with_code {
        sqlx::query(
            "INSERT INTO set_properties (set_id, property_id, property_name, property_city, property_code)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(set_id)
        .bind(property.id)
        .bind(&property.name)
        .bind(&property.city)
        .bind(&property.code)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to insert set_property record: {}", e))?;
    }

    // Update DB statuses for properties with code -> ARCHIVE
    let archive_base_path = get_base_path_for_status(&config, "ARCHIVE")?;
    let properties_archived = with_code.len();
    for property in &with_code {
        if let Some(property_id) = property.id {
            let now_ts = chrono::Utc::now().timestamp_millis();
            sqlx::query("UPDATE properties SET status = 'ARCHIVE', updated_at = ? WHERE id = ?")
                .bind(now_ts)
                .bind(property_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update property status: {}", e))?;
        }
    }

    // Update DB statuses for properties without code -> NOT_FOUND
    let not_found_base_path = get_base_path_for_status(&config, "NOT_FOUND")?;
    let properties_moved_to_not_found = without_code.len();
    for property in &without_code {
        if let Some(property_id) = property.id {
            let now_ts = chrono::Utc::now().timestamp_millis();
            sqlx::query("UPDATE properties SET status = 'NOT_FOUND', updated_at = ? WHERE id = ?")
                .bind(now_ts)
                .bind(property_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update property status: {}", e))?;
        }
    }

    // Move folders on disk (blocking I/O on dedicated thread)
    {
        let with_code_paths: Vec<_> = with_code.iter().map(|p| p.folder_path.clone()).collect();
        let without_code_paths: Vec<_> = without_code.iter().map(|p| p.folder_path.clone()).collect();
        let done_base = done_base_path.clone();
        let archive_base = archive_base_path.clone();
        let not_found_base = not_found_base_path.clone();

        tokio::task::spawn_blocking(move || {
            // Move properties with code to ARCHIVE
            for fp in &with_code_paths {
                let folder_path_buf = folder_path_to_pathbuf(fp);
                let old_path = done_base.join(&folder_path_buf);
                let new_path = archive_base.join(&folder_path_buf);

                if old_path.exists() && old_path != new_path {
                    if let Some(parent) = new_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                    }
                    std::fs::rename(&old_path, &new_path)
                        .map_err(|e| format!("Failed to move folder to archive: {}", e))?;
                }
            }

            // Move properties without code to NOT_FOUND
            for fp in &without_code_paths {
                let folder_path_buf = folder_path_to_pathbuf(fp);
                let old_path = done_base.join(&folder_path_buf);
                let new_path = not_found_base.join(&folder_path_buf);

                if old_path.exists() && old_path != new_path {
                    if let Some(parent) = new_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                    }
                    std::fs::rename(&old_path, &new_path)
                        .map_err(|e| format!("Failed to move folder to not found: {}", e))?;
                }
            }

            Ok::<_, String>(())
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))??;
    }

    Ok(CompleteSetResult {
        set_id,
        set_name,
        zip_path: zip_path_str,
        properties_archived,
        properties_moved_to_not_found,
    })
}

/// Get all sets
#[tauri::command]
pub async fn get_sets(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let sets: Vec<Set> = sqlx::query_as::<_, (i64, String, String, i64, i64)>(
        "SELECT id, name, zip_path, property_count, created_at FROM sets ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch sets: {}", e))?
    .into_iter()
    .map(|(id, name, zip_path, property_count, created_at)| Set {
        id: Some(id),
        name,
        zip_path,
        property_count,
        created_at: chrono::DateTime::from_timestamp_millis(created_at)
            .unwrap_or_else(chrono::Utc::now),
    })
    .collect();

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(sets).map_err(|e| e.to_string())?),
    })
}

/// Get properties that were included in a specific set
#[tauri::command]
pub async fn get_set_properties(
    app: tauri::AppHandle,
    set_id: i64,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let set_properties: Vec<SetProperty> = sqlx::query_as::<_, (i64, i64, Option<i64>, String, String, Option<String>)>(
        "SELECT id, set_id, property_id, property_name, property_city, property_code
         FROM set_properties WHERE set_id = ?"
    )
    .bind(set_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch set properties: {}", e))?
    .into_iter()
    .map(|(id, set_id, property_id, property_name, property_city, property_code)| SetProperty {
        id: Some(id),
        set_id,
        property_id,
        property_name,
        property_city,
        property_code,
    })
    .collect();

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(set_properties).map_err(|e| e.to_string())?),
    })
}

/// Open the sets folder in file explorer
#[tauri::command]
pub async fn open_sets_folder(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("App configuration not found")?;

    if config.sets_folder_path.is_empty() {
        return Ok(CommandResult {
            success: false,
            error: Some("Sets folder path is not configured".to_string()),
            data: None,
        });
    }

    let sets_folder = PathBuf::from(&config.sets_folder_path);

    tokio::task::spawn_blocking(move || {
        if !sets_folder.exists() {
            return Ok(CommandResult {
                success: false,
                error: Some("Sets folder does not exist".to_string()),
                data: None,
            });
        }

        match opener::open(&sets_folder) {
            Ok(()) => Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "opened_path": sets_folder.to_string_lossy()
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

/// Delete a set record and optionally the ZIP file
#[tauri::command]
pub async fn delete_set(
    app: tauri::AppHandle,
    set_id: i64,
    delete_zip: bool,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    // Get the set info first
    let set_row = sqlx::query("SELECT zip_path FROM sets WHERE id = ?")
        .bind(set_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to fetch set: {}", e))?;

    let Some(set_row) = set_row else {
        return Ok(CommandResult {
            success: false,
            error: Some("Set not found".to_string()),
            data: None,
        });
    };

    let zip_path: String = set_row.get("zip_path");

    // Delete the ZIP file if requested
    if delete_zip {
        let zip_path_clone = zip_path.clone();
        tokio::task::spawn_blocking(move || {
            let zip_file = PathBuf::from(&zip_path_clone);
            if zip_file.exists() {
                std::fs::remove_file(&zip_file)
                    .map_err(|e| format!("Failed to delete ZIP file: {}", e))?;
            }
            Ok::<_, String>(())
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))??;
    }

    // Delete set_properties records (CASCADE should handle this, but be explicit)
    sqlx::query("DELETE FROM set_properties WHERE set_id = ?")
        .bind(set_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete set properties: {}", e))?;

    // Delete the set record
    sqlx::query("DELETE FROM sets WHERE id = ?")
        .bind(set_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete set: {}", e))?;

    Ok(CommandResult {
        success: true,
        error: None,
        data: None,
    })
}
