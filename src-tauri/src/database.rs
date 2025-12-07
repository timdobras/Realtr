use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::Manager;
use tokio::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Property {
    pub id: Option<i64>,
    pub name: String,
    pub city: String,
    pub status: String, // "NEW", "DONE", "NOT_FOUND", "ARCHIVE"
    pub folder_path: String,
    pub notes: Option<String>,
    pub code: Option<String>, // Website listing code (e.g., "45164")
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Legacy field for backward compatibility during migration
    #[serde(skip_serializing)]
    #[serde(default)]
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct City {
    pub id: Option<i64>,
    pub name: String,
    pub usage_count: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub found_properties: usize,
    pub new_properties: usize,
    pub existing_properties: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Set {
    pub id: Option<i64>,
    pub name: String,
    pub zip_path: String,
    pub property_count: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetProperty {
    pub id: Option<i64>,
    pub set_id: i64,
    pub property_id: Option<i64>,
    pub property_name: String,
    pub property_city: String,
    pub property_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSetResult {
    pub set_id: i64,
    pub set_name: String,
    pub zip_path: String,
    pub properties_archived: usize,
    pub properties_moved_to_not_found: usize,
}

// Helper function to safely get the database pool
fn get_database_pool(app: &tauri::AppHandle) -> Result<&SqlitePool, String> {
    match app.try_state::<SqlitePool>() {
        Some(pool) => Ok(pool.inner()),
        None => Err("Database not initialized. Please restart the application.".to_string()),
    }
}

// Helper function to get the base folder path for a given status
fn get_base_path_for_status(config: &crate::config::AppConfig, status: &str) -> Result<PathBuf, String> {
    let path_str = match status {
        "NEW" => &config.new_folder_path,
        "DONE" => &config.done_folder_path,
        "NOT_FOUND" => &config.not_found_folder_path,
        "ARCHIVE" => &config.archive_folder_path,
        _ => return Err(format!("Invalid status: {}", status)),
    };

    if path_str.is_empty() {
        return Err(format!("Folder path for status '{}' is not configured", status));
    }

    Ok(PathBuf::from(path_str))
}

// Helper function to generate a thumbnail from an image
fn generate_thumbnail(
    source_path: &PathBuf,
    thumbnail_path: &PathBuf,
    max_size: u32,
) -> Result<(), String> {
    // Load the image
    let img = image::open(source_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;

    // Calculate new dimensions while maintaining aspect ratio
    let (width, height) = img.dimensions();
    let (new_width, new_height) = if width > height {
        let ratio = max_size as f32 / width as f32;
        (max_size, (height as f32 * ratio) as u32)
    } else {
        let ratio = max_size as f32 / height as f32;
        ((width as f32 * ratio) as u32, max_size)
    };

    // Resize the image (Triangle is fastest for thumbnails)
    let thumbnail = img.resize(new_width, new_height, image::imageops::FilterType::Triangle);

    // Save the thumbnail as JPEG to save space
    thumbnail
        .save_with_format(thumbnail_path, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to save thumbnail: {}", e))?;

    Ok(())
}

// Helper function to construct full property path from config and property data
fn construct_property_path_from_parts(
    config: &crate::config::AppConfig,
    status: &str,
    city: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let base_path = get_base_path_for_status(config, status)?;
    Ok(base_path.join(city).join(name))
}

// Helper function to construct relative folder_path for database storage
fn get_relative_folder_path(city: &str, name: &str) -> String {
    format!("{}/{}", city, name)
}

// Helper function to convert folder_path (stored with /) to a proper PathBuf
// This is needed because on Windows, PathBuf::join doesn't convert / to \
fn folder_path_to_pathbuf(folder_path: &str) -> PathBuf {
    let parts: Vec<&str> = folder_path.split('/').collect();
    let mut path = PathBuf::new();
    for part in parts {
        path.push(part);
    }
    path
}

// Helper function to construct full property base path from config, folder_path and status
async fn get_property_base_path(
    app: &tauri::AppHandle,
    folder_path: &str,
    status: &str,
) -> Result<PathBuf, String> {
    let config = crate::config::load_config(app.clone())
        .await
        .map_err(|e| e.to_string())?;
    let config = config.ok_or("App configuration not found")?;

    let base_path = get_base_path_for_status(&config, status)?;
    Ok(base_path.join(folder_path_to_pathbuf(folder_path)))
}

// Helper to find where a property folder actually exists across all status folders
// Returns (full_path, actual_status) if found
fn find_actual_folder_location(
    config: &crate::config::AppConfig,
    folder_path: &str,
) -> Option<(PathBuf, String)> {
    let status_paths = [
        (&config.new_folder_path, "NEW"),
        (&config.done_folder_path, "DONE"),
        (&config.not_found_folder_path, "NOT_FOUND"),
        (&config.archive_folder_path, "ARCHIVE"),
    ];

    let folder_path_buf = folder_path_to_pathbuf(folder_path);
    for (base_path_str, status) in status_paths {
        if base_path_str.is_empty() {
            continue;
        }
        let full_path = PathBuf::from(base_path_str).join(&folder_path_buf);
        if full_path.exists() && full_path.is_dir() {
            return Some((full_path, status.to_string()));
        }
    }
    None
}

// Database initialization
pub async fn init_database(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // Ensure the directory exists with proper error handling
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir).map_err(|e| {
            format!(
                "Failed to create app data directory {}: {}",
                app_data_dir.display(),
                e
            )
        })?;
    }

    let database_path = app_data_dir.join("properties.db");

    println!(
        "Attempting to connect to database at: {}",
        database_path.display()
    );

    // Set connection options for SQLite
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
    )
    .await
    .map_err(|e| {
        format!(
            "Failed to connect to database at {}: {}",
            database_path.display(),
            e
        )
    })?;

    println!("Database connection established successfully");

    // Run migrations
    run_migrations(&pool).await?;

    println!("Database migrations completed successfully");

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), String> {
    // Create properties table with TIMESTAMP columns
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS properties (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            city TEXT NOT NULL,
            completed BOOLEAN NOT NULL DEFAULT 0,
            folder_path TEXT NOT NULL,
            notes TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create properties table: {}", e))?;

    // Create cities table for autocomplete
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            usage_count INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create cities table: {}", e))?;

    // Create indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_properties_completed ON properties(completed)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create completed index: {}", e))?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_properties_city ON properties(city)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create city index: {}", e))?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cities_name ON cities(name)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create cities name index: {}", e))?;

    // Migration: Add status column if it doesn't exist
    // First check if the column exists
    let column_check = sqlx::query("PRAGMA table_info(properties)")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to check table info: {}", e))?;

    let has_status_column = column_check.iter().any(|row| {
        row.try_get::<String, _>("name")
            .map(|name| name == "status")
            .unwrap_or(false)
    });

    if !has_status_column {
        // Add status column with default value 'NEW'
        sqlx::query("ALTER TABLE properties ADD COLUMN status TEXT DEFAULT 'NEW'")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to add status column: {}", e))?;

        // Migrate existing data from completed boolean to status
        sqlx::query(
            r#"
            UPDATE properties
            SET status = CASE
                WHEN completed = 1 THEN 'DONE'
                ELSE 'NEW'
            END
            WHERE status IS NULL OR status = 'NEW'
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to migrate completed to status: {}", e))?;

        // Create index for status column
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_properties_status ON properties(status)")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create status index: {}", e))?;
    }

    // Migration: Add code column if it doesn't exist
    let has_code_column = column_check.iter().any(|row| {
        row.try_get::<String, _>("name")
            .map(|name| name == "code")
            .unwrap_or(false)
    });

    if !has_code_column {
        sqlx::query("ALTER TABLE properties ADD COLUMN code TEXT")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to add code column: {}", e))?;

        // Create index for code column to enable fast searches
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_properties_code ON properties(code)")
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to create code index: {}", e))?;
    }

    // Create sets table for tracking completed property sets
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS sets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            zip_path TEXT NOT NULL,
            property_count INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )
        ",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create sets table: {}", e))?;

    // Create set_properties junction table for tracking which properties were in each set
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS set_properties (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            set_id INTEGER NOT NULL,
            property_id INTEGER,
            property_name TEXT NOT NULL,
            property_city TEXT NOT NULL,
            property_code TEXT,
            FOREIGN KEY (set_id) REFERENCES sets(id) ON DELETE CASCADE
        )
        ",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create set_properties table: {}", e))?;

    // Create indexes for sets tables
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sets_created_at ON sets(created_at)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create sets created_at index: {}", e))?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_set_properties_set_id ON set_properties(set_id)")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create set_properties set_id index: {}", e))?;

    Ok(())
}

// Property CRUD operations
#[tauri::command]
pub async fn create_property(
    app: tauri::AppHandle,
    name: String,
    city: String,
    notes: Option<String>,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let status = "NEW";
    let folder_path = get_relative_folder_path(&city, &name);
    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();

    // Start a transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // Insert or update city
    sqlx::query(
        r#"
        INSERT INTO cities (name, usage_count, created_at)
        VALUES (?, 1, ?)
        ON CONFLICT(name) DO UPDATE SET usage_count = usage_count + 1
        "#,
    )
    .bind(&city)
    .bind(now_timestamp)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update city: {}", e))?;

    // Insert property
    let result = sqlx::query(
        r#"
        INSERT INTO properties (name, city, status, folder_path, notes, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&name)
    .bind(&city)
    .bind(status)
    .bind(&folder_path)
    .bind(&notes)
    .bind(now_timestamp)
    .bind(now_timestamp)
    .execute(&mut *tx)
    .await;

    match result {
        Ok(result) => {
            let property_id = result.last_insert_rowid();

            // Commit the transaction
            tx.commit()
                .await
                .map_err(|e| format!("Failed to commit transaction: {}", e))?;

            // Create the folder structure
            let config_result = crate::config::load_config(app.clone()).await;
            if let Ok(Some(config)) = config_result {
                match construct_property_path_from_parts(&config, status, &city, &name) {
                    Ok(property_path) => {
                        if let Err(e) = create_property_folder_structure(&property_path).await {
                            return Ok(CommandResult {
                                success: false,
                                error: Some(format!(
                                    "Property created but folder creation failed: {}",
                                    e
                                )),
                                data: None,
                            });
                        }
                    }
                    Err(e) => {
                        return Ok(CommandResult {
                            success: false,
                            error: Some(format!("Failed to get property path: {}", e)),
                            data: None,
                        });
                    }
                }
            }

            Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({"id": property_id})),
            })
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Ok(CommandResult {
                success: false,
                error: Some(format!("Failed to create property: {}", e)),
                data: None,
            })
        }
    }
}

#[tauri::command]
pub async fn get_properties(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let rows = sqlx::query("SELECT * FROM properties ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch properties: {}", e))?;

    let mut properties = Vec::new();

    for row in rows {
        // Convert timestamps back to DateTime
        let created_at_timestamp: i64 = row.get("created_at");
        let updated_at_timestamp: i64 = row.get("updated_at");

        let created_at = chrono::DateTime::from_timestamp_millis(created_at_timestamp)
            .unwrap_or_else(|| chrono::Utc::now());
        let updated_at = chrono::DateTime::from_timestamp_millis(updated_at_timestamp)
            .unwrap_or_else(|| chrono::Utc::now());

        let property = Property {
            id: Some(row.get("id")),
            name: row.get("name"),
            city: row.get("city"),
            status: row.get("status"),
            folder_path: row.get("folder_path"),
            notes: row.get("notes"),
            code: row.get("code"),
            created_at,
            updated_at,
            completed: None,
        };

        properties.push(property);
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(properties).unwrap()),
    })
}

#[tauri::command]
pub async fn get_properties_by_status(
    app: tauri::AppHandle,
    status: String,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let rows = sqlx::query("SELECT * FROM properties WHERE status = ? ORDER BY created_at DESC")
        .bind(&status)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch properties: {}", e))?;

    let mut properties = Vec::new();

    for row in rows {
        let created_at_timestamp: i64 = row.get("created_at");
        let updated_at_timestamp: i64 = row.get("updated_at");

        let created_at = chrono::DateTime::from_timestamp_millis(created_at_timestamp)
            .unwrap_or_else(|| chrono::Utc::now());
        let updated_at = chrono::DateTime::from_timestamp_millis(updated_at_timestamp)
            .unwrap_or_else(|| chrono::Utc::now());

        let property = Property {
            id: Some(row.get("id")),
            name: row.get("name"),
            city: row.get("city"),
            status: row.get("status"),
            folder_path: row.get("folder_path"),
            notes: row.get("notes"),
            code: row.get("code"),
            created_at,
            updated_at,
            completed: None,
        };

        properties.push(property);
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(properties).unwrap()),
    })
}

#[tauri::command]
pub async fn update_property_status(
    app: tauri::AppHandle,
    property_id: i64,
    new_status: String,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    // Validate status
    if !["NEW", "DONE", "NOT_FOUND", "ARCHIVE"].contains(&new_status.as_str()) {
        return Ok(CommandResult {
            success: false,
            error: Some(format!("Invalid status: {}", new_status)),
            data: None,
        });
    }

    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();

    // Get current property info
    let property_row = sqlx::query("SELECT * FROM properties WHERE id = ?")
        .bind(property_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Property not found: {}", e))?;

    let current_status: String = property_row.get("status");
    let _city: String = property_row.get("city");
    let _name: String = property_row.get("name");
    // Get the actual folder_path from database - this is the real folder name on disk
    // which may include a code suffix like "PROPERTY NAME (CODE)"
    let db_folder_path: String = property_row.get("folder_path");

    // Use the database folder_path for operations, don't reconstruct it
    let folder_path = db_folder_path;

    // IMPORTANT: Move folder FIRST before updating database
    // This ensures we don't update the database if the folder move fails
    if current_status != new_status {
        let config_result = crate::config::load_config(app.clone()).await;
        if let Ok(Some(config)) = config_result {
            // Get base paths using actual folder_path from database
            let new_base = get_base_path_for_status(&config, &new_status);
            let old_base = get_base_path_for_status(&config, &current_status);

            match new_base {
                Ok(new_base_path) => {
                    let folder_path_buf = folder_path_to_pathbuf(&folder_path);
                    let new_path = new_base_path.join(&folder_path_buf);

                    // First try the expected location based on current_status
                    let expected_old_path = old_base.ok().map(|b| b.join(&folder_path_buf));

                    // Find actual folder location - check expected location first, then search all
                    let actual_old_path = match expected_old_path {
                        Some(ref path) if path.exists() => Some(path.clone()),
                        _ => {
                            // Folder not at expected location, search all status folders
                            find_actual_folder_location(&config, &folder_path)
                                .map(|(path, _)| path)
                        }
                    };

                    if let Some(old_path) = actual_old_path {
                        if old_path != new_path {
                            // Create parent directory for new path
                            if let Some(parent) = new_path.parent() {
                                if let Err(e) = fs::create_dir_all(parent) {
                                    return Ok(CommandResult {
                                        success: false,
                                        error: Some(format!(
                                            "Failed to create parent directory: {}. \
                                            Hint: Make sure no files are open in the folder and try again.",
                                            e
                                        )),
                                        data: None,
                                    });
                                }
                            }
                            // Move the folder - try with retry for Windows lock issues
                            if let Err(e) = fs::rename(&old_path, &new_path) {
                                // On Windows, "Access is denied" often means a file is locked
                                // Try a small delay and retry once
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                if let Err(e2) = fs::rename(&old_path, &new_path) {
                                    return Ok(CommandResult {
                                        success: false,
                                        error: Some(format!(
                                            "Failed to move folder: {}. \
                                            Hint: Close any open files/folders and File Explorer windows for this property, then try again.",
                                            e2
                                        )),
                                        data: None,
                                    });
                                }
                            }
                        }
                    }
                    // If folder not found anywhere, just update status without moving
                    // (folder might have been manually deleted)
                }
                Err(e) => {
                    return Ok(CommandResult {
                        success: false,
                        error: Some(format!("Failed to get property path: {}", e)),
                        data: None,
                    });
                }
            }
        }
    }

    // Only update database AFTER folder move succeeded
    let result = sqlx::query(
        "UPDATE properties SET status = ?, folder_path = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&new_status)
    .bind(&folder_path)
    .bind(now_timestamp)
    .bind(property_id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(CommandResult {
            success: true,
            error: None,
            data: None,
        }),
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to update property: {}", e)),
            data: None,
        }),
    }
}

#[tauri::command]
pub async fn set_property_code(
    app: tauri::AppHandle,
    property_id: i64,
    code: String,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    // Validate code is not empty
    let code = code.trim();
    if code.is_empty() {
        return Ok(CommandResult {
            success: false,
            error: Some("Code cannot be empty".to_string()),
            data: None,
        });
    }

    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();

    // Get current property info
    let property_row = sqlx::query("SELECT * FROM properties WHERE id = ?")
        .bind(property_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Property not found: {}", e))?;

    let name: String = property_row.get("name");
    let city: String = property_row.get("city");
    let status: String = property_row.get("status");
    let folder_path: String = property_row.get("folder_path");

    // Extract the actual folder name from the stored folder_path (format: "city/folder_name")
    // This ensures we use the real folder name on disk, not a reconstructed one
    let old_folder_name = folder_path
        .split('/')
        .last()
        .unwrap_or(&name)
        .to_string();

    // For folder names, replace "/" with "-" since "/" is not allowed in folder names
    // This allows codes like "204905/44538" to be saved as "204905-44538" in the folder name
    let folder_safe_code = code.replace('/', "-");
    let new_folder_name = format!("{} ({})", name, folder_safe_code);

    // Calculate new folder path (relative) for database storage
    let new_folder_path = format!("{}/{}", city, new_folder_name);

    // Get config for absolute paths
    let config = crate::config::load_config(app.clone())
        .await
        .map_err(|e| format!("Failed to load config: {}", e))?
        .ok_or("App configuration not found")?;

    let base_path = get_base_path_for_status(&config, &status)?;
    let old_absolute_path = base_path.join(&city).join(&old_folder_name);
    let new_absolute_path = base_path.join(&city).join(&new_folder_name);

    // Only rename if paths are different and old path exists
    if old_absolute_path != new_absolute_path && old_absolute_path.exists() {
        // Rename the folder
        std::fs::rename(&old_absolute_path, &new_absolute_path)
            .map_err(|e| format!("Failed to rename folder: {}", e))?;
    }

    // Update database with new code and folder_path
    let result = sqlx::query(
        "UPDATE properties SET code = ?, folder_path = ?, updated_at = ? WHERE id = ?",
    )
    .bind(code)
    .bind(&new_folder_path)
    .bind(now_timestamp)
    .bind(property_id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "new_folder_path": new_folder_path,
                "code": code
            })),
        }),
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to update property code: {}", e)),
            data: None,
        }),
    }
}

#[tauri::command]
pub async fn update_property(
    app: tauri::AppHandle,
    property_id: i64,
    name: String,
    city: String,
    notes: Option<String>,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    // Validate inputs
    let name = name.trim();
    let city = city.trim();
    if name.is_empty() {
        return Ok(CommandResult {
            success: false,
            error: Some("Property name cannot be empty".to_string()),
            data: None,
        });
    }
    if city.is_empty() {
        return Ok(CommandResult {
            success: false,
            error: Some("City cannot be empty".to_string()),
            data: None,
        });
    }

    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();

    // Get current property info
    let property_row = sqlx::query("SELECT * FROM properties WHERE id = ?")
        .bind(property_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Property not found: {}", e))?;

    let old_name: String = property_row.get("name");
    let old_city: String = property_row.get("city");
    let status: String = property_row.get("status");
    let folder_path: String = property_row.get("folder_path");
    let code: Option<String> = property_row.get("code");

    // Extract current folder name from folder_path (format: "city/folder_name")
    let old_folder_name = folder_path
        .split('/')
        .last()
        .unwrap_or(&old_name)
        .to_string();

    // Determine what changed
    let name_changed = name != old_name;
    let city_changed = city != old_city;

    // Calculate the new folder name
    // If there's a code, the folder name format is "{name} ({code})"
    // We need to preserve the code suffix when renaming
    let new_folder_name = if let Some(ref c) = code {
        let folder_safe_code = c.replace('/', "-");
        format!("{} ({})", name, folder_safe_code)
    } else {
        name.to_string()
    };

    // Calculate new folder path (relative) for database storage
    let new_folder_path = format!("{}/{}", city, new_folder_name);

    // Get config for absolute paths
    let config = crate::config::load_config(app.clone())
        .await
        .map_err(|e| format!("Failed to load config: {}", e))?
        .ok_or("App configuration not found")?;

    let base_path = get_base_path_for_status(&config, &status)?;

    // Handle folder operations if name or city changed
    if name_changed || city_changed {
        let old_absolute_path = base_path.join(&old_city).join(&old_folder_name);
        let new_absolute_path = base_path.join(&city).join(&new_folder_name);

        // Check if source folder exists
        if old_absolute_path.exists() {
            // Check if target folder already exists (would be a conflict)
            if old_absolute_path != new_absolute_path && new_absolute_path.exists() {
                return Ok(CommandResult {
                    success: false,
                    error: Some(format!(
                        "Cannot move/rename: folder '{}' already exists",
                        new_absolute_path.display()
                    )),
                    data: None,
                });
            }

            // If city changed, ensure the new city directory exists
            if city_changed {
                let new_city_path = base_path.join(&city);
                if !new_city_path.exists() {
                    std::fs::create_dir_all(&new_city_path)
                        .map_err(|e| format!("Failed to create city folder: {}", e))?;
                }
            }

            // Move/rename the folder
            if old_absolute_path != new_absolute_path {
                std::fs::rename(&old_absolute_path, &new_absolute_path)
                    .map_err(|e| format!("Failed to move/rename folder: {}", e))?;
            }
        }
    }

    // Update database
    let result = sqlx::query(
        "UPDATE properties SET name = ?, city = ?, notes = ?, folder_path = ?, updated_at = ? WHERE id = ?",
    )
    .bind(name)
    .bind(city)
    .bind(&notes)
    .bind(&new_folder_path)
    .bind(now_timestamp)
    .bind(property_id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => {
            // Also update city usage count
            let _ = sqlx::query(
                "INSERT INTO cities (name, usage_count, created_at) VALUES (?, 1, ?)
                 ON CONFLICT(name) DO UPDATE SET usage_count = usage_count + 1",
            )
            .bind(city)
            .bind(now_timestamp)
            .execute(pool)
            .await;

            Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::json!({
                    "name": name,
                    "city": city,
                    "notes": notes,
                    "folder_path": new_folder_path
                })),
            })
        }
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to update property: {}", e)),
            data: None,
        }),
    }
}

#[tauri::command]
pub async fn delete_property(
    app: tauri::AppHandle,
    property_id: i64,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let result = sqlx::query("DELETE FROM properties WHERE id = ?")
        .bind(property_id)
        .execute(pool)
        .await;

    match result {
        Ok(_) => Ok(CommandResult {
            success: true,
            error: None,
            data: None,
        }),
        Err(e) => Ok(CommandResult {
            success: false,
            error: Some(format!("Failed to delete property: {}", e)),
            data: None,
        }),
    }
}

// City operations for autocomplete
#[tauri::command]
pub async fn get_cities(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let rows = sqlx::query("SELECT * FROM cities ORDER BY usage_count DESC, name ASC")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch cities: {}", e))?;

    let mut cities = Vec::new();

    for row in rows {
        let created_at_timestamp: i64 = row.get("created_at");
        let created_at = chrono::DateTime::from_timestamp_millis(created_at_timestamp)
            .unwrap_or_else(|| chrono::Utc::now());

        let city = City {
            id: Some(row.get("id")),
            name: row.get("name"),
            usage_count: row.get("usage_count"),
            created_at,
        };

        cities.push(city);
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(cities).unwrap()),
    })
}

#[tauri::command]
pub async fn search_cities(app: tauri::AppHandle, query: String) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let search_pattern = format!("%{}%", query);

    let rows = sqlx::query(
        "SELECT * FROM cities WHERE name LIKE ? ORDER BY usage_count DESC, name ASC LIMIT 10",
    )
    .bind(&search_pattern)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to search cities: {}", e))?;

    let mut cities = Vec::new();

    for row in rows {
        let created_at_timestamp: i64 = row.get("created_at");
        let created_at = chrono::DateTime::from_timestamp_millis(created_at_timestamp)
            .unwrap_or_else(|| chrono::Utc::now());

        let city = City {
            id: Some(row.get("id")),
            name: row.get("name"),
            usage_count: row.get("usage_count"),
            created_at,
        };

        cities.push(city);
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(cities).unwrap()),
    })
}

#[tauri::command]
pub async fn get_property_by_id(
    app: tauri::AppHandle,
    property_id: i64,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let row_result = sqlx::query("SELECT * FROM properties WHERE id = ?")
        .bind(property_id)
        .fetch_one(pool)
        .await;

    match row_result {
        Ok(row) => {
            let created_at_timestamp: i64 = row.get("created_at");
            let updated_at_timestamp: i64 = row.get("updated_at");

            let created_at = chrono::DateTime::from_timestamp_millis(created_at_timestamp)
                .unwrap_or_else(|| chrono::Utc::now());
            let updated_at = chrono::DateTime::from_timestamp_millis(updated_at_timestamp)
                .unwrap_or_else(|| chrono::Utc::now());

            let property = Property {
                id: Some(row.get("id")),
                name: row.get("name"),
                city: row.get("city"),
                status: row.get("status"),
                folder_path: row.get("folder_path"),
                notes: row.get("notes"),
                code: row.get("code"),
                created_at,
                updated_at,
                completed: None,
            };

            Ok(CommandResult {
                success: true,
                error: None,
                data: Some(serde_json::to_value(property).unwrap()),
            })
        }
        Err(_) => Ok(CommandResult {
            success: false,
            error: Some("Property not found".to_string()),
            data: None,
        }),
    }
}

// Scan and import properties function
#[tauri::command]
pub async fn scan_and_import_properties(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let config_result = crate::config::load_config(app.clone()).await;
    let config = match config_result {
        Ok(Some(config)) => config,
        Ok(None) => {
            return Ok(CommandResult {
                success: false,
                error: Some(
                    "No configuration found. Please set up the root folder first.".to_string(),
                ),
                data: None,
            });
        }
        Err(e) => {
            return Ok(CommandResult {
                success: false,
                error: Some(format!("Failed to load configuration: {}", e)),
                data: None,
            });
        }
    };

    let mut scan_result = ScanResult {
        found_properties: 0,
        new_properties: 0,
        existing_properties: 0,
        errors: Vec::new(),
    };

    let existing_properties = match get_existing_properties_set(pool).await {
        Ok(props) => props,
        Err(e) => {
            return Ok(CommandResult {
                success: false,
                error: Some(e),
                data: None,
            });
        }
    };

    // Scan all 4 status folders
    let folders_to_scan = [
        (&config.new_folder_path, "NEW"),
        (&config.done_folder_path, "DONE"),
        (&config.not_found_folder_path, "NOT_FOUND"),
        (&config.archive_folder_path, "ARCHIVE"),
    ];

    for (folder_path_str, status) in folders_to_scan {
        if folder_path_str.is_empty() {
            continue; // Skip if folder path not configured
        }

        let folder_path = PathBuf::from(folder_path_str);

        if !folder_path.exists() {
            continue; // Skip if folder doesn't exist
        }

        match scan_folder_for_properties(&folder_path, status, &existing_properties, pool)
            .await
        {
            Ok(folder_result) => {
                scan_result.found_properties += folder_result.found_properties;
                scan_result.new_properties += folder_result.new_properties;
                scan_result.existing_properties += folder_result.existing_properties;
                scan_result.errors.extend(folder_result.errors);
            }
            Err(e) => {
                scan_result
                    .errors
                    .push(format!("Error scanning {} folder: {}", status, e));
            }
        }
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(scan_result).map_err(|e| e.to_string())?),
    })
}

/// Repair result structure
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairResult {
    pub properties_checked: usize,
    pub properties_fixed: usize,
    pub errors: Vec<String>,
}

/// Helper function to find a folder by prefix match within a city directory
/// This handles cases where folder has a code suffix like "PROPERTY NAME (12345)"
fn find_folder_by_prefix(city_path: &PathBuf, property_name: &str) -> Option<String> {
    if !city_path.exists() || !city_path.is_dir() {
        return None;
    }

    if let Ok(entries) = fs::read_dir(city_path) {
        for entry in entries.flatten() {
            if let Some(folder_name) = entry.file_name().to_str() {
                // Check if folder starts with property name
                // Match "PROPERTY NAME" or "PROPERTY NAME (code)" or "PROPERTY NAME (code-code)"
                if folder_name == property_name
                    || folder_name.starts_with(&format!("{} (", property_name))
                {
                    return Some(folder_name.to_string());
                }
            }
        }
    }
    None
}

/// Repair property statuses by checking actual folder locations
/// This fixes properties where the database status doesn't match where the folder actually exists
/// Also handles folder name mismatches (e.g., when folder has code suffix but DB doesn't)
#[tauri::command]
pub async fn repair_property_statuses(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let config = crate::config::load_config(app.clone())
        .await
        .map_err(|e| e.to_string())?
        .ok_or("App configuration not found")?;

    let mut result = RepairResult {
        properties_checked: 0,
        properties_fixed: 0,
        errors: Vec::new(),
    };

    // Get all properties from database
    let properties: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, folder_path, status, name FROM properties"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch properties: {}", e))?;

    // Get base paths for all statuses
    let status_paths: Vec<(&str, Option<PathBuf>)> = vec![
        ("NEW", get_base_path_for_status(&config, "NEW").ok()),
        ("DONE", get_base_path_for_status(&config, "DONE").ok()),
        ("NOT_FOUND", get_base_path_for_status(&config, "NOT_FOUND").ok()),
        ("ARCHIVE", get_base_path_for_status(&config, "ARCHIVE").ok()),
    ];

    for (id, folder_path, db_status, name) in properties {
        result.properties_checked += 1;

        // Parse folder_path into city and property folder name
        let parts: Vec<&str> = folder_path.split('/').collect();
        if parts.len() != 2 {
            result.errors.push(format!(
                "Property '{}' has invalid folder_path format: '{}'",
                name, folder_path
            ));
            continue;
        }
        let city = parts[0];
        let property_folder_name = parts[1];

        // Convert folder_path to proper PathBuf (handles / -> \ on Windows)
        let folder_path_buf = folder_path_to_pathbuf(&folder_path);

        // First try exact match
        let mut found_info: Option<(&str, Option<String>)> = None; // (status, new_folder_name if different)

        for (status, base_path_opt) in &status_paths {
            if let Some(base_path) = base_path_opt {
                let full_path = base_path.join(&folder_path_buf);
                if full_path.exists() {
                    found_info = Some((status, None)); // Exact match
                    break;
                }
            }
        }

        // If not found with exact match, try prefix matching (for code suffixes)
        if found_info.is_none() {
            for (status, base_path_opt) in &status_paths {
                if let Some(base_path) = base_path_opt {
                    let city_path = base_path.join(city);
                    if let Some(actual_folder_name) = find_folder_by_prefix(&city_path, property_folder_name) {
                        if actual_folder_name != property_folder_name {
                            found_info = Some((status, Some(actual_folder_name)));
                        } else {
                            found_info = Some((status, None));
                        }
                        break;
                    }
                }
            }
        }

        // If folder found, update database if needed
        if let Some((found_status, new_folder_name_opt)) = found_info {
            let status_changed = found_status != db_status;
            let folder_path_changed = new_folder_name_opt.is_some();

            if status_changed || folder_path_changed {
                let now_ts = chrono::Utc::now().timestamp_millis();
                let new_folder_path = if let Some(ref new_name) = new_folder_name_opt {
                    format!("{}/{}", city, new_name)
                } else {
                    folder_path.clone()
                };

                match sqlx::query("UPDATE properties SET status = ?, folder_path = ?, updated_at = ? WHERE id = ?")
                    .bind(found_status)
                    .bind(&new_folder_path)
                    .bind(now_ts)
                    .bind(id)
                    .execute(pool)
                    .await
                {
                    Ok(_) => {
                        result.properties_fixed += 1;
                        if folder_path_changed {
                            if let Some(new_name) = new_folder_name_opt {
                                result.errors.push(format!(
                                    "Fixed '{}': folder_path updated from '{}' to '{}/{}'",
                                    name, folder_path, city, new_name
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        result.errors.push(format!(
                            "Failed to update status for '{}': {}",
                            name, e
                        ));
                    }
                }
            }
        } else {
            // Folder not found in any location - this is a warning but not necessarily an error
            // The property might have been manually deleted from the filesystem
            // Include the folder_path and checked paths for debugging
            let checked_paths: Vec<String> = status_paths
                .iter()
                .filter_map(|(status, base_path_opt)| {
                    base_path_opt.as_ref().map(|bp| {
                        format!("{}: {}", status, bp.join(&folder_path_buf).display())
                    })
                })
                .collect();
            result.errors.push(format!(
                "Property '{}' folder not found. DB folder_path='{}'. Checked: [{}]",
                name, folder_path, checked_paths.join(", ")
            ));
        }
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(result).map_err(|e| e.to_string())?),
    })
}

async fn get_existing_properties_set(pool: &SqlitePool) -> Result<HashSet<String>, String> {
    // Use folder_path which contains the actual folder name on disk (including code if present)
    let rows = sqlx::query("SELECT folder_path FROM properties")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch existing properties: {}", e))?;

    let mut existing = HashSet::new();
    for row in rows {
        let folder_path: String = row.get("folder_path");
        existing.insert(folder_path);
    }

    Ok(existing)
}

async fn scan_folder_for_properties(
    folder_path: &PathBuf,
    status: &str,
    existing_properties: &HashSet<String>,
    pool: &SqlitePool,
) -> Result<ScanResult, String> {
    let mut result = ScanResult {
        found_properties: 0,
        new_properties: 0,
        existing_properties: 0,
        errors: Vec::new(),
    };

    let entries =
        std::fs::read_dir(folder_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                result
                    .errors
                    .push(format!("Error reading directory entry: {}", e));
                continue;
            }
        };

        let city_path = entry.path();
        if !city_path.is_dir() {
            continue;
        }

        let city_name = match city_path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => {
                result
                    .errors
                    .push(format!("Invalid city folder name: {:?}", city_path));
                continue;
            }
        };

        let city_entries = match std::fs::read_dir(&city_path) {
            Ok(entries) => entries,
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to read city folder {}: {}", city_name, e));
                continue;
            }
        };

        for property_entry in city_entries {
            let property_entry = match property_entry {
                Ok(entry) => entry,
                Err(e) => {
                    result.errors.push(format!(
                        "Error reading property entry in {}: {}",
                        city_name, e
                    ));
                    continue;
                }
            };

            let property_path = property_entry.path();
            if !property_path.is_dir() {
                continue;
            }

            let folder_name = match property_path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => {
                    result
                        .errors
                        .push(format!("Invalid property folder name: {:?}", property_path));
                    continue;
                }
            };

            // Parse folder name to extract property name and code
            // e.g., "Apartment 85sqm (45164)" -> name: "Apartment 85sqm", code: Some("45164")
            let (property_name, code) = parse_folder_name(&folder_name);

            result.found_properties += 1;

            // Use folder_name for the key since that's what's on disk
            let property_key = format!("{}/{}", city_name, folder_name);

            if existing_properties.contains(&property_key) {
                result.existing_properties += 1;
                continue;
            }

            if !is_valid_property_folder(&property_path) {
                result
                    .errors
                    .push(format!("Invalid property structure: {}", property_key));
                continue;
            }

            match add_property_to_database(
                pool,
                &property_name,
                &city_name,
                status,
                &folder_name,
                code.as_deref(),
            )
            .await
            {
                Ok(_) => {
                    result.new_properties += 1;
                }
                Err(e) => {
                    result
                        .errors
                        .push(format!("Failed to add property {}: {}", property_key, e));
                }
            }
        }
    }

    Ok(result)
}

fn is_valid_property_folder(property_path: &PathBuf) -> bool {
    // A valid property folder just needs to be a directory
    // INTERNET and WATERMARK folders will be created when user starts working on it
    property_path.is_dir()
}

/// Parse a folder name that may contain a code in the format "Property Name (12345)"
/// Returns (property_name, code) where:
/// - property_name: The name without the code suffix
/// - code: The extracted code if present
fn parse_folder_name(folder_name: &str) -> (String, Option<String>) {
    // Check if folder name ends with pattern " (code)" where code is alphanumeric
    if let Some(open_paren) = folder_name.rfind(" (") {
        if folder_name.ends_with(')') {
            let potential_code = &folder_name[open_paren + 2..folder_name.len() - 1];
            // Check if the content in parentheses looks like a code (alphanumeric, not too long)
            if !potential_code.is_empty()
                && potential_code.len() <= 20
                && potential_code.chars().all(|c| c.is_alphanumeric())
            {
                let property_name = folder_name[..open_paren].to_string();
                return (property_name, Some(potential_code.to_string()));
            }
        }
    }
    // No code found, return the folder name as-is
    (folder_name.to_string(), None)
}

async fn add_property_to_database(
    pool: &SqlitePool,
    property_name: &str,
    city_name: &str,
    status: &str,
    folder_name: &str,
    code: Option<&str>,
) -> Result<(), String> {
    // Use the folder_name for the path (keeps the code in the path if present)
    let folder_path = get_relative_folder_path(city_name, folder_name);

    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    sqlx::query(
        r#"
        INSERT INTO cities (name, usage_count, created_at)
        VALUES (?, 1, ?)
        ON CONFLICT(name) DO UPDATE SET usage_count = usage_count + 1
        "#,
    )
    .bind(city_name)
    .bind(now_timestamp)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update city: {}", e))?;

    sqlx::query(
        r#"
        INSERT INTO properties (name, city, status, folder_path, notes, code, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(property_name)
    .bind(city_name)
    .bind(status)
    .bind(&folder_path)
    .bind("Imported from existing folder")
    .bind(code)
    .bind(now_timestamp)
    .bind(now_timestamp)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to insert property: {}", e))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(())
}

// Helper functions
async fn create_property_folder_structure(property_path: &PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(property_path)
        .map_err(|e| format!("Failed to create property directory: {}", e))?;

    let internet_path = property_path.join("INTERNET");
    let watermark_path = property_path.join("WATERMARK");

    std::fs::create_dir_all(&internet_path)
        .map_err(|e| format!("Failed to create INTERNET folder: {}", e))?;

    std::fs::create_dir_all(&watermark_path)
        .map_err(|e| format!("Failed to create WATERMARK folder: {}", e))?;

    std::fs::create_dir_all(internet_path.join("AGGELIA"))
        .map_err(|e| format!("Failed to create INTERNET/AGGELIA folder: {}", e))?;

    std::fs::create_dir_all(watermark_path.join("AGGELIA"))
        .map_err(|e| format!("Failed to create WATERMARK/AGGELIA folder: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn debug_database_dates(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    // Check the actual schema
    let schema_info = sqlx::query("PRAGMA table_info(properties)")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get schema info: {}", e))?;

    println!("=== DATABASE SCHEMA ===");
    for row in &schema_info {
        let name: String = row.get("name");
        let type_name: String = row.get("type");
        println!("Column: {} - Type: {}", name, type_name);
    }

    // Check actual data
    let data_rows = sqlx::query("SELECT id, name, created_at, updated_at FROM properties LIMIT 5")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get data: {}", e))?;

    println!("=== SAMPLE DATA ===");
    for row in &data_rows {
        let id: i64 = row.get("id");
        let name: String = row.get("name");

        // Try to get the dates as different types to see what's actually stored
        println!("Property ID: {}, Name: {}", id, name);

        // Try as string first
        if let Ok(created_str) = row.try_get::<String, _>("created_at") {
            println!("  created_at (as string): {}", created_str);
        }

        // Try as i64
        if let Ok(created_i64) = row.try_get::<i64, _>("created_at") {
            println!("  created_at (as i64): {}", created_i64);
        }

        // Try as f64
        if let Ok(created_f64) = row.try_get::<f64, _>("created_at") {
            println!("  created_at (as f64): {}", created_f64);
        }
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::json!({
            "schema": "Check console for schema info",
            "data": "Check console for data info"
        })),
    })
}

#[tauri::command]
pub async fn reset_database_with_proper_dates(
    app: tauri::AppHandle,
) -> Result<CommandResult, String> {
    // Get the existing database pool from app state
    let pool = get_database_pool(&app)?;

    // Delete all data from tables (this avoids file locking issues on Windows)
    sqlx::query("DELETE FROM properties")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to clear properties table: {}", e))?;

    sqlx::query("DELETE FROM cities")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to clear cities table: {}", e))?;

    // Reset SQLite auto-increment counters
    sqlx::query("DELETE FROM sqlite_sequence WHERE name='properties' OR name='cities'")
        .execute(pool)
        .await
        .ok(); // Ignore errors if sqlite_sequence doesn't exist

    // Force WAL checkpoint to ensure all changes are written to the main database file
    // This is important on Windows where WAL might not be immediately flushed
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await
        .ok(); // Ignore errors if WAL is not enabled

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::json!({"message": "Database cleared successfully"})),
    })
}

#[tauri::command]
pub async fn list_original_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    // folder_path is relative (city/name), status determines which base folder to use

    // Load config to get base path for status
    let config = crate::config::load_config(app.clone())
        .await
        .map_err(|e| e.to_string())?;
    let config = config.ok_or("App configuration not found")?;

    // Get base path for the property's status
    let base_path = get_base_path_for_status(&config, &status)?;
    let folder_path_buf = folder_path_to_pathbuf(&folder_path);
    let full_path = base_path.join(&folder_path_buf);

    // If not found at expected location, try to find it in other status folders
    let full_path = if full_path.exists() && full_path.is_dir() {
        full_path
    } else {
        // Fallback: search all status folders for the actual folder location
        match find_actual_folder_location(&config, &folder_path) {
            Some((found_path, _actual_status)) => found_path,
            None => return Err(format!("Folder not found: {}", full_path.display())),
        }
    };

    let mut images = Vec::new();

    for entry in fs::read_dir(full_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            // Filter image file extensions (you can extend this list)
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ext_lc == "jpg"
                    || ext_lc == "jpeg"
                    || ext_lc == "png"
                    || ext_lc == "bmp"
                    || ext_lc == "gif"
                    || ext_lc == "heic"
                {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        images.push(filename.to_string());
                    }
                }
            }
        }
    }

    Ok(images)
}

#[tauri::command]
pub async fn open_images_in_folder(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    selected_image: String,
) -> Result<CommandResult, String> {
    // Get the full absolute path using the property base path
    let full_folder_path = get_property_base_path(&app, &folder_path, &status).await?;
    if !full_folder_path.exists() || !full_folder_path.is_dir() {
        return Ok(CommandResult {
            success: false,
            error: Some(format!(
                "Folder path does not exist: {}",
                full_folder_path.display()
            )),
            data: None,
        });
    }

    // List all image files in the folder
    let mut image_paths = Vec::new();
    for entry in std::fs::read_dir(&full_folder_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                let ext = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext.as_str()) {
                    image_paths.push(path);
                }
            }
        }
    }

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
pub async fn get_image_as_base64(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
) -> Result<String, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    let full_path = property_path.join(&filename);

    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    // Read file bytes
    let image_bytes =
        fs::read(&full_path).map_err(|e| format!("Failed to read image file: {}", e))?;

    // Convert to base64
    let base64_string = general_purpose::STANDARD.encode(&image_bytes);

    Ok(base64_string)
}

#[tauri::command]
pub async fn list_internet_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let internet_path = property_path.join("INTERNET");

    if !internet_path.exists() {
        return Ok(Vec::new());
    }

    let mut images = Vec::new();
    for entry in fs::read_dir(internet_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        images.push(filename.to_string());
                    }
                }
            }
        }
    }

    images.sort();
    Ok(images)
}

#[tauri::command]
pub async fn get_internet_image_as_base64(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
) -> Result<String, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let full_path = property_path.join("INTERNET").join(&filename);

    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    let image_bytes =
        fs::read(&full_path).map_err(|e| format!("Failed to read image file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

#[tauri::command]
pub async fn copy_images_to_internet(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let internet_path = property_path.join("INTERNET");

    // Ensure INTERNET folder exists
    fs::create_dir_all(&internet_path)
        .map_err(|e| format!("Failed to create INTERNET folder: {}", e))?;

    // Get list of original images
    let mut copied_count = 0;
    let mut errors = Vec::new();

    for entry in fs::read_dir(&property_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        let dest_path = internet_path.join(filename);

                        // Only copy if the file doesn't already exist
                        if !dest_path.exists() {
                            match fs::copy(&path, &dest_path) {
                                Ok(_) => {
                                    copied_count += 1;
                                }
                                Err(e) => {
                                    errors.push(format!("Failed to copy {}: {}", filename, e))
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "copied_count": copied_count,
                "message": format!("Successfully copied {} images to INTERNET folder", copied_count)
            })),
        })
    } else {
        Ok(CommandResult {
            success: false,
            error: Some(format!(
                "Copied {} images but encountered errors: {}",
                copied_count,
                errors.join(", ")
            )),
            data: None,
        })
    }
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

    let mut thumbnails = Vec::new();
    for entry in fs::read_dir(&property_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        // Return as .jpg since thumbnails are always JPEG
                        thumbnails.push(format!("{}.jpg", stem));
                    }
                }
            }
        }
    }

    thumbnails.sort();
    Ok(thumbnails)
}

#[tauri::command]
pub async fn get_thumbnail_as_base64(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
) -> Result<String, String> {
    // Get app data directory for thumbnails
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    let thumbnails_base = app_data_dir.join("thumbnails");
    let safe_folder_name = folder_path.replace('/', "_").replace('\\', "_");
    let thumbnails_dir = thumbnails_base.join(&safe_folder_name);
    let thumbnail_path = thumbnails_dir.join(&filename);

    // If thumbnail doesn't exist, generate it on-demand
    if !thumbnail_path.exists() {
        // Create thumbnails directory if it doesn't exist
        fs::create_dir_all(&thumbnails_dir)
            .map_err(|e| format!("Failed to create thumbnails directory: {}", e))?;

        // Get the original image path
        let property_path = get_property_base_path(&app, &folder_path, &status).await?;

        // Remove .jpg extension from filename to get original stem
        let original_stem = filename.trim_end_matches(".jpg");

        // Try to find the original image file with any supported extension
        let supported_exts = ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"];
        let mut original_path = None;

        for ext in &supported_exts {
            let potential_path = property_path.join(format!("{}.{}", original_stem, ext));
            if potential_path.exists() {
                original_path = Some(potential_path);
                break;
            }
        }

        if let Some(source_path) = original_path {
            // Generate thumbnail (100x100 for fast generation)
            generate_thumbnail(&source_path, &thumbnail_path, 100)
                .map_err(|e| format!("Failed to generate thumbnail: {}", e))?;
        } else {
            return Err(format!("Original image not found for thumbnail: {}", original_stem));
        }
    }

    let image_bytes =
        fs::read(&thumbnail_path).map_err(|e| format!("Failed to read thumbnail file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

/// Get a gallery-sized thumbnail for workflow step displays.
/// This is larger than the property list thumbnails (400px vs 100px)
/// and supports different subfolders (INTERNET, AGGELIA, WATERMARK, etc.)
#[tauri::command]
pub async fn get_gallery_thumbnail_as_base64(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    subfolder: String,
    filename: String,
    max_dimension: Option<u32>,
) -> Result<String, String> {
    let max_size = max_dimension.unwrap_or(400);

    // Get app data directory for gallery thumbnails
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    // Use separate directory for gallery thumbnails with size in path
    let thumbnails_base = app_data_dir.join("thumbnails").join(format!("gallery_{}", max_size));
    let safe_folder_name = folder_path.replace('/', "_").replace('\\', "_");
    let safe_subfolder = if subfolder.is_empty() {
        "root".to_string()
    } else {
        subfolder.replace('/', "_").replace('\\', "_")
    };
    let thumbnails_dir = thumbnails_base.join(&safe_folder_name).join(&safe_subfolder);
    let thumbnail_path = thumbnails_dir.join(&filename).with_extension("jpg");

    // Get the original image path
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let source_dir = if subfolder.is_empty() {
        property_path
    } else {
        property_path.join(&subfolder)
    };
    let source_path = source_dir.join(&filename);

    if !source_path.exists() {
        return Err(format!("Source image not found: {}", source_path.display()));
    }

    // Check if we need to regenerate the thumbnail:
    // 1. Thumbnail doesn't exist, OR
    // 2. Source image is newer than thumbnail (was modified)
    let needs_regeneration = if !thumbnail_path.exists() {
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
            _ => true, // If we can't get times, regenerate to be safe
        }
    };

    if needs_regeneration {
        // Create thumbnails directory if it doesn't exist
        fs::create_dir_all(&thumbnails_dir)
            .map_err(|e| format!("Failed to create thumbnails directory: {}", e))?;

        // Generate thumbnail
        generate_thumbnail(&source_path, &thumbnail_path, max_size)
            .map_err(|e| format!("Failed to generate gallery thumbnail: {}", e))?;
    }

    let image_bytes = fs::read(&thumbnail_path)
        .map_err(|e| format!("Failed to read gallery thumbnail file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
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
    use std::sync::Arc;
    use std::thread;

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

    // Generate thumbnails in parallel using threads
    let to_generate = Arc::new(to_generate);
    let num_threads = std::cmp::min(8, to_generate.len()); // Max 8 threads
    let chunk_size = (to_generate.len() + num_threads - 1) / num_threads;

    let mut handles = Vec::new();

    for i in 0..num_threads {
        let to_generate = Arc::clone(&to_generate);
        let start = i * chunk_size;
        let end = std::cmp::min(start + chunk_size, to_generate.len());

        if start >= end {
            break;
        }

        let handle = thread::spawn(move || {
            let mut generated = 0;
            for j in start..end {
                let (source_path, thumbnail_path) = &to_generate[j];
                if generate_thumbnail(source_path, thumbnail_path, max_size).is_ok() {
                    generated += 1;
                }
            }
            generated
        });
        handles.push(handle);
    }

    // Wait for all threads and sum results
    let generated_count: usize = handles
        .into_iter()
        .filter_map(|h| h.join().ok())
        .sum();

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::json!({
            "generated": generated_count,
            "cached": cached_count,
            "total": filenames.len()
        })),
    })
}

#[tauri::command]
pub async fn clear_internet_folder(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    let internet_path = property_path.join("INTERNET");

    if !internet_path.exists() {
        return Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({"message": "INTERNET folder doesn't exist"})),
        });
    }

    let mut deleted_count = 0;
    let mut errors = Vec::new();

    for entry in fs::read_dir(&internet_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            match fs::remove_file(&path) {
                Ok(_) => deleted_count += 1,
                Err(e) => {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        errors.push(format!("Failed to delete {}: {}", filename, e));
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "deleted_count": deleted_count,
                "message": format!("Successfully deleted {} images from INTERNET folder", deleted_count)
            })),
        })
    } else {
        Ok(CommandResult {
            success: false,
            error: Some(format!(
                "Deleted {} images but encountered errors: {}",
                deleted_count,
                errors.join(", ")
            )),
            data: None,
        })
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
    let config = crate::config::load_config(app.clone())
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

    if !image_path.exists() {
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

    // Use configured fast editor or system default
    let result = if let Some(editor_path) = &config.fast_editor_path {
        // Use custom fast editor
        Command::new(editor_path)
            .arg(image_path.to_str().unwrap())
            .spawn()
    } else {
        // Use system default
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "", image_path.to_str().unwrap()])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
        } else if cfg!(target_os = "macos") {
            Command::new("open")
                .arg(image_path.to_str().unwrap())
                .spawn()
        } else {
            Command::new("xdg-open")
                .arg(image_path.to_str().unwrap())
                .spawn()
        }
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

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameMapping {
    pub old_name: String,
    pub new_name: String,
}

#[tauri::command]
pub async fn rename_internet_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    rename_map: Vec<RenameMapping>,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let internet_path = property_path.join("INTERNET");

    if !internet_path.exists() {
        return Ok(CommandResult {
            success: false,
            error: Some("INTERNET folder does not exist".to_string()),
            data: None,
        });
    }

    let mut renamed_count = 0;
    let mut errors = Vec::new();
    let mut temp_renames = Vec::new();

    // First pass: Rename all files to temporary names to avoid conflicts
    for (i, mapping) in rename_map.iter().enumerate() {
        let old_path = internet_path.join(&mapping.old_name);
        let temp_name = format!("temp_rename_{}.tmp", i);
        let temp_path = internet_path.join(&temp_name);

        if old_path.exists() {
            match fs::rename(&old_path, &temp_path) {
                Ok(_) => {
                    temp_renames.push((temp_name, mapping.new_name.clone()));
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to rename {} to temp: {}",
                        mapping.old_name, e
                    ));
                }
            }
        } else {
            errors.push(format!("File not found: {}", mapping.old_name));
        }
    }

    // Second pass: Rename temporary files to final names
    for (temp_name, final_name) in temp_renames {
        let temp_path = internet_path.join(&temp_name);
        let final_path = internet_path.join(&final_name);

        match fs::rename(&temp_path, &final_path) {
            Ok(_) => renamed_count += 1,
            Err(e) => {
                errors.push(format!(
                    "Failed to rename {} to {}: {}",
                    temp_name, final_name, e
                ));
                // Try to restore original name if possible
                // This is a best-effort cleanup
            }
        }
    }

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "renamed_count": renamed_count,
                "message": format!("Successfully renamed {} images", renamed_count)
            })),
        })
    } else {
        Ok(CommandResult {
            success: renamed_count > 0,
            error: Some(format!(
                "Renamed {} images but encountered errors: {}",
                renamed_count,
                errors.join(", ")
            )),
            data: Some(serde_json::json!({
                "renamed_count": renamed_count,
                "errors": errors
            })),
        })
    }
}

#[tauri::command]
pub async fn list_aggelia_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let aggelia_path = property_path.join("INTERNET").join("AGGELIA");

    if !aggelia_path.exists() {
        return Ok(Vec::new());
    }

    let mut images = Vec::new();
    for entry in fs::read_dir(aggelia_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        images.push(filename.to_string());
                    }
                }
            }
        }
    }

    images.sort();
    Ok(images)
}

#[tauri::command]
pub async fn get_aggelia_image_as_base64(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
) -> Result<String, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let full_path = property_path
        .join("INTERNET")
        .join("AGGELIA")
        .join(&filename);

    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    let image_bytes =
        fs::read(&full_path).map_err(|e| format!("Failed to read image file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

#[tauri::command]
pub async fn copy_images_to_aggelia(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filenames: Vec<String>,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let internet_path = property_path.join("INTERNET");
    let aggelia_path = internet_path.join("AGGELIA");

    // Ensure AGGELIA folder exists
    fs::create_dir_all(&aggelia_path)
        .map_err(|e| format!("Failed to create AGGELIA folder: {}", e))?;

    let mut copied_count = 0;
    let mut errors = Vec::new();

    for filename in filenames {
        let source_path = internet_path.join(&filename);
        let dest_path = aggelia_path.join(&filename);

        if source_path.exists() {
            // Only copy if the file doesn't already exist in AGGELIA
            if !dest_path.exists() {
                match fs::copy(&source_path, &dest_path) {
                    Ok(_) => copied_count += 1,
                    Err(e) => errors.push(format!("Failed to copy {}: {}", filename, e)),
                }
            } else {
                // File already exists, count as "copied"
                copied_count += 1;
            }
        } else {
            errors.push(format!("Source file not found: {}", filename));
        }
    }

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "copied_count": copied_count,
                "message": format!("Successfully copied {} images to AGGELIA folder", copied_count)
            })),
        })
    } else {
        Ok(CommandResult {
            success: copied_count > 0,
            error: Some(format!(
                "Copied {} images but encountered errors: {}",
                copied_count,
                errors.join(", ")
            )),
            data: Some(serde_json::json!({
                "copied_count": copied_count,
                "errors": errors
            })),
        })
    }
}

#[tauri::command]
pub async fn clear_aggelia_folder(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let aggelia_path = property_path.join("INTERNET").join("AGGELIA");

    if !aggelia_path.exists() {
        return Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({"message": "AGGELIA folder doesn't exist"})),
        });
    }

    let mut deleted_count = 0;
    let mut errors = Vec::new();

    for entry in fs::read_dir(&aggelia_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            match fs::remove_file(&path) {
                Ok(_) => deleted_count += 1,
                Err(e) => {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        errors.push(format!("Failed to delete {}: {}", filename, e));
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "deleted_count": deleted_count,
                "message": format!("Successfully deleted {} images from AGGELIA folder", deleted_count)
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
            data: Some(serde_json::json!({
                "deleted_count": deleted_count,
                "errors": errors
            })),
        })
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
    let config = crate::config::load_config(app.clone())
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

    if !image_path.exists() {
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

    // Use configured complex editor or system default
    let result = if let Some(editor_path) = &config.complex_editor_path {
        // Use custom complex editor
        Command::new(editor_path)
            .arg(image_path.to_str().unwrap())
            .spawn()
    } else {
        // Use system default
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "", image_path.to_str().unwrap()])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
        } else if cfg!(target_os = "macos") {
            Command::new("open")
                .arg(image_path.to_str().unwrap())
                .spawn()
        } else {
            Command::new("xdg-open")
                .arg(image_path.to_str().unwrap())
                .spawn()
        }
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
pub async fn copy_and_watermark_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone())
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
    let internet_path = property_path.join("INTERNET");
    let aggelia_path = internet_path.join("AGGELIA");
    let watermark_path = property_path.join("WATERMARK");
    let watermark_aggelia_path = watermark_path.join("AGGELIA");

    // Ensure WATERMARK folders exist
    fs::create_dir_all(&watermark_path)
        .map_err(|e| format!("Failed to create WATERMARK folder: {}", e))?;
    fs::create_dir_all(&watermark_aggelia_path)
        .map_err(|e| format!("Failed to create WATERMARK/AGGELIA folder: {}", e))?;

    // Load watermark image once
    let watermark_img = image::open(&watermark_img_path)
        .map_err(|e| format!("Failed to load watermark image: {}", e))?;

    let mut processed_count = 0;
    let mut errors = Vec::new();

    // Process INTERNET folder -> WATERMARK folder
    if internet_path.exists() {
        match copy_and_process_folder_with_config(
            &internet_path,
            &watermark_path,
            &watermark_img,
            &config.watermark_config,
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
            &config.watermark_config,
        ) {
            Ok(count) => processed_count += count,
            Err(e) => errors.push(format!("AGGELIA folder: {}", e)),
        }
    }

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
) -> Result<usize, String> {
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

    // Process images in parallel using rayon
    image_files.par_iter().for_each(|(source, dest)| {
        match apply_watermark_to_image_with_config(source, dest, watermark_img, config) {
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
        }
    });

    // Check for errors
    if let Ok(errs) = errors.lock() {
        if !errs.is_empty() {
            return Err(errs.join("; "));
        }
    }

    Ok(processed_count.load(Ordering::Relaxed))
}

fn apply_watermark_to_image_with_config(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    watermark_img: &DynamicImage,
    config: &crate::config::WatermarkConfig,
) -> Result<(), String> {
    // Load source image
    let mut base_img = image::open(source_path)
        .map_err(|e| format!("Failed to open source image: {}", e))?
        .to_rgba8();

    // Apply watermark using config
    apply_watermark_with_config(&mut base_img, watermark_img, config)?;

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
pub async fn generate_watermark_preview(
    app: tauri::AppHandle,
    sample_image_base64: Option<String>,
) -> Result<String, String> {
    // Load watermark config
    let config = crate::config::load_config(app.clone())
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

    let watermark_img =
        image::open(&watermark_path).map_err(|e| format!("Failed to load watermark: {}", e))?;

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

    // Apply watermark using current config
    apply_watermark_with_config(&mut base_img, &watermark_img, &config.watermark_config)?;

    // Encode result as base64
    let mut buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buffer);
    base_img
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image: {}", e))?;

    let base64_result = general_purpose::STANDARD.encode(&buffer);
    Ok(base64_result)
}

fn apply_watermark_with_config(
    base_img: &mut RgbaImage,
    watermark_img: &DynamicImage,
    config: &crate::config::WatermarkConfig,
) -> Result<(), String> {
    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark_img.dimensions();

    // Calculate watermark size based on size_mode
    let (new_wm_width, new_wm_height) = match config.size_mode.as_str() {
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
            // Fit within image bounds while maintaining aspect ratio
            let scale_x = base_width as f32 / wm_width as f32;
            let scale_y = base_height as f32 / wm_height as f32;
            let scale = scale_x.min(scale_y).min(1.0);

            (
                (wm_width as f32 * scale) as u32,
                (wm_height as f32 * scale) as u32,
            )
        }
        "stretch" => (base_width, base_height),
        "tile" => (wm_width, wm_height), // Keep original size for tiling
        _ => {
            let max_size = (base_width.max(base_height) as f32 * config.size_percentage) as u32;
            let scale = max_size as f32 / wm_width.max(wm_height) as f32;
            (
                (wm_width as f32 * scale) as u32,
                (wm_height as f32 * scale) as u32,
            )
        }
    };

    // Resize watermark (CatmullRom is faster than Lanczos3 with good quality)
    let resized_watermark = watermark_img
        .resize_exact(
            new_wm_width,
            new_wm_height,
            image::imageops::FilterType::CatmullRom,
        )
        .to_rgba8();

    // Apply watermark based on mode
    if config.size_mode == "tile" {
        apply_tiled_watermark(base_img, &resized_watermark, config)?;
    } else {
        apply_single_watermark(base_img, &resized_watermark, config)?;
    }

    Ok(())
}

fn apply_single_watermark(
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    config: &crate::config::WatermarkConfig,
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
    let pos_x = (base_x as i32 + config.offset_x).max(0).min(base_width as i32 - wm_width as i32) as u32;
    let pos_y = (base_y as i32 + config.offset_y).max(0).min(base_height as i32 - wm_height as i32) as u32;

    // Apply watermark with opacity
    blend_watermark(base_img, watermark, pos_x, pos_y, config.opacity, config.use_alpha_channel);

    Ok(())
}

fn apply_tiled_watermark(
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    config: &crate::config::WatermarkConfig,
) -> Result<(), String> {
    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark.dimensions();

    let mut y = 0;
    while y < base_height {
        let mut x = 0;
        while x < base_width {
            blend_watermark(base_img, watermark, x, y, config.opacity, config.use_alpha_channel);
            x += wm_width + config.offset_x.unsigned_abs();
        }
        y += wm_height + config.offset_y.unsigned_abs();
    }

    Ok(())
}

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

fn apply_watermark_to_image(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    watermark_img: &DynamicImage,
    opacity: f32,
) -> Result<(), String> {
    // Load source image
    let mut base_img = image::open(source_path)
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

    // Resize watermark (CatmullRom is faster than Lanczos3 with good quality)
    let resized_watermark = watermark_img
        .resize_exact(
            new_wm_width,
            new_wm_height,
            image::imageops::FilterType::CatmullRom,
        )
        .to_rgba8();

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
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        images.push(filename.to_string());
                    }
                }
            }
        }
    }

    images.sort();
    Ok(images)
}

#[tauri::command]
pub async fn list_watermark_aggelia_images(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<String>, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    let watermark_aggelia_path = property_path
        .join("WATERMARK")
        .join("AGGELIA");

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
                if ["jpg", "jpeg", "png", "bmp", "gif", "heic", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        images.push(filename.to_string());
                    }
                }
            }
        }
    }

    images.sort();
    Ok(images)
}

#[tauri::command]
pub async fn get_watermark_image_as_base64(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
    filename: String,
    from_aggelia: bool,
) -> Result<String, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    let full_path = if from_aggelia {
        property_path
            .join("WATERMARK")
            .join("AGGELIA")
            .join(&filename)
    } else {
        property_path
            .join("WATERMARK")
            .join(&filename)
    };

    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    let image_bytes =
        fs::read(&full_path).map_err(|e| format!("Failed to read image file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

#[tauri::command]
pub async fn clear_watermark_folders(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;

    let watermark_path = property_path.join("WATERMARK");
    let mut deleted_count = 0;
    let mut errors = Vec::new();

    if watermark_path.exists() {
        // Clear main WATERMARK folder
        match clear_folder_images(&watermark_path) {
            Ok(count) => deleted_count += count,
            Err(e) => errors.push(format!("WATERMARK folder: {}", e)),
        }

        // Clear WATERMARK/AGGELIA folder
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
}

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
pub async fn open_property_folder(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let full_path = get_property_base_path(&app, &folder_path, &status).await?;

    println!("Attempting to open: {}", full_path.display());

    // Use opener crate which handles cross-platform file opening
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

/// Fill AGGELIA folders to 25 images by duplicating existing images with a 1% crop
#[tauri::command]
pub async fn fill_aggelia_to_25(
    app: tauri::AppHandle,
    folder_path: String,
    status: String,
) -> Result<CommandResult, String> {
    let property_path = get_property_base_path(&app, &folder_path, &status).await?;
    let internet_aggelia_path = property_path.join("INTERNET").join("AGGELIA");
    let watermark_aggelia_path = property_path.join("WATERMARK").join("AGGELIA");

    // Ensure folders exist
    if !internet_aggelia_path.exists() {
        return Ok(CommandResult {
            success: false,
            error: Some("INTERNET/AGGELIA folder does not exist".to_string()),
            data: None,
        });
    }

    // Get existing images from INTERNET/AGGELIA
    let mut existing_images: Vec<String> = Vec::new();
    for entry in fs::read_dir(&internet_aggelia_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        existing_images.push(filename.to_string());
                    }
                }
            }
        }
    }

    existing_images.sort();

    let current_count = existing_images.len();

    // Check if already at 25+
    if current_count >= 25 {
        return Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "message": "Property already has 25 or more images",
                "current_count": current_count,
                "added_count": 0
            })),
        });
    }

    if current_count == 0 {
        return Ok(CommandResult {
            success: false,
            error: Some("No images in AGGELIA folder to duplicate".to_string()),
            data: None,
        });
    }

    // Ensure WATERMARK/AGGELIA exists
    fs::create_dir_all(&watermark_aggelia_path)
        .map_err(|e| format!("Failed to create WATERMARK/AGGELIA folder: {}", e))?;

    // Find the highest existing number for naming
    let mut max_number: u32 = 0;
    for img in &existing_images {
        if let Some(num_str) = img.split('.').next() {
            if let Ok(num) = num_str.parse::<u32>() {
                if num > max_number {
                    max_number = num;
                }
            }
        }
    }

    let images_to_add = 25 - current_count;

    // Process images in parallel using rayon
    use rayon::prelude::*;

    let results: Vec<Result<String, String>> = (0..images_to_add)
        .into_par_iter()
        .map(|i| {
            // Cycle through existing images
            let source_filename = &existing_images[i % current_count];
            let source_path = internet_aggelia_path.join(source_filename);

            // Get the extension from source
            let ext = source_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg")
                .to_lowercase();

            // New filename with next sequential number
            let new_number = max_number + (i as u32) + 1;
            let new_filename = format!("{:02}.{}", new_number, ext);

            let dest_internet_path = internet_aggelia_path.join(&new_filename);
            let dest_watermark_path = watermark_aggelia_path.join(&new_filename);

            // Process for INTERNET/AGGELIA
            match crop_and_save_image(&source_path, &dest_internet_path) {
                Ok(()) => {
                    // Also create cropped version for WATERMARK/AGGELIA if source exists there
                    let watermark_source = watermark_aggelia_path.join(source_filename);
                    if watermark_source.exists() {
                        if let Err(e) = crop_and_save_image(&watermark_source, &dest_watermark_path) {
                            return Err(format!("Failed to create watermark copy for {}: {}", new_filename, e));
                        }
                    }
                    Ok(new_filename)
                }
                Err(e) => Err(format!("Failed to create {}: {}", new_filename, e)),
            }
        })
        .collect();

    // Aggregate results
    let mut added_count = 0;
    let mut errors: Vec<String> = Vec::new();
    for result in results {
        match result {
            Ok(_) => added_count += 1,
            Err(e) => errors.push(e),
        }
    }

    if errors.is_empty() {
        Ok(CommandResult {
            success: true,
            error: None,
            data: Some(serde_json::json!({
                "message": format!("Added {} images to reach 25", added_count),
                "added_count": added_count,
                "total_count": current_count + added_count
            })),
        })
    } else {
        Ok(CommandResult {
            success: added_count > 0,
            error: Some(format!("Added {} images with some errors: {}", added_count, errors.join(", "))),
            data: Some(serde_json::json!({
                "added_count": added_count,
                "errors": errors
            })),
        })
    }
}

/// Crop an image by 1% on each edge and save to destination
fn crop_and_save_image(source_path: &PathBuf, dest_path: &PathBuf) -> Result<(), String> {
    use image::GenericImageView;

    let img = image::open(source_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;

    let (width, height) = img.dimensions();

    // Calculate 1% crop from each edge
    let crop_percent = 0.01_f32;
    let crop_x = (width as f32 * crop_percent) as u32;
    let crop_y = (height as f32 * crop_percent) as u32;

    // Ensure we don't crop too much for very small images
    let crop_x = crop_x.max(1);
    let crop_y = crop_y.max(1);

    let new_width = width.saturating_sub(crop_x * 2);
    let new_height = height.saturating_sub(crop_y * 2);

    // Ensure minimum dimensions
    if new_width < 10 || new_height < 10 {
        return Err("Image too small to crop".to_string());
    }

    let cropped = img.crop_imm(crop_x, crop_y, new_width, new_height);

    // Determine output format based on extension
    let ext = dest_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "jpg" || ext == "jpeg" {
        // Convert to RGB for JPEG (no alpha channel)
        let rgb_img = cropped.to_rgb8();
        rgb_img
            .save(dest_path)
            .map_err(|e| format!("Failed to save cropped image: {}", e))?;
    } else {
        cropped
            .save(dest_path)
            .map_err(|e| format!("Failed to save cropped image: {}", e))?;
    }

    Ok(())
}

// ============================================================================
// Sets Commands
// ============================================================================

/// Helper function to recursively add a directory to a ZIP file
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

        // Calculate relative path from the base
        let relative_path = path
            .strip_prefix(base_path)
            .map_err(|e| format!("Failed to strip prefix: {}", e))?;

        let relative_path_str = relative_path
            .to_str()
            .ok_or("Invalid path encoding")?
            .replace('\\', "/"); // Ensure forward slashes in ZIP

        if path.is_dir() {
            // Add directory entry (with trailing slash)
            if !relative_path_str.is_empty() {
                let dir_name = format!("{}/", relative_path_str);
                zip.add_directory(&dir_name, options)
                    .map_err(|e| format!("Failed to add directory to ZIP: {}", e))?;
            }
        } else {
            // Add file
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
    let config = crate::config::load_config(app.clone())
        .await
        .map_err(|e| e.to_string())?
        .ok_or("App configuration not found")?;

    // Validate sets folder path is configured
    if config.sets_folder_path.is_empty() {
        return Err("Sets folder path is not configured. Please configure it in Settings.".to_string());
    }

    let sets_folder = PathBuf::from(&config.sets_folder_path);
    if !sets_folder.exists() {
        std::fs::create_dir_all(&sets_folder)
            .map_err(|e| format!("Failed to create sets folder: {}", e))?;
    }

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

    // Create ZIP file
    let now = chrono::Local::now();
    let set_name = format!("Done - {}", now.format("%Y-%m-%d %H-%M-%S"));
    let zip_filename = format!("{}.zip", set_name);
    let zip_path = sets_folder.join(&zip_filename);

    let file = std::fs::File::create(&zip_path)
        .map_err(|e| format!("Failed to create ZIP file: {}", e))?;

    let mut zip = zip::ZipWriter::new(file);
    // Use Stored (no compression) instead of Deflated for speed
    // Photos are already compressed (JPEG/PNG), so deflate provides minimal benefit
    // but takes much longer. Stored mode is ~10x faster with minimal size increase.
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // Add each property to the ZIP
    let done_base_path = get_base_path_for_status(&config, "DONE")?;
    for property in &with_code {
        // Use folder_path which contains the actual folder name (including code suffix)
        let property_path = done_base_path.join(folder_path_to_pathbuf(&property.folder_path));

        if property_path.exists() {
            // The ZIP will have structure: City/PropertyName/...
            // We need to create the City folder in the ZIP
            let city_folder = format!("{}/", property.city);
            let _ = zip.add_directory(&city_folder, options); // Ignore if already exists

            // Add the property folder with its contents
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

    // Move properties with code to ARCHIVE
    let archive_base_path = get_base_path_for_status(&config, "ARCHIVE")?;
    let properties_archived = with_code.len();
    for property in &with_code {
        if let Some(property_id) = property.id {
            // Update status in database
            let now_ts = chrono::Utc::now().timestamp_millis();
            sqlx::query("UPDATE properties SET status = 'ARCHIVE', updated_at = ? WHERE id = ?")
                .bind(now_ts)
                .bind(property_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update property status: {}", e))?;

            // Move folder - use folder_path which has the actual folder name
            let folder_path_buf = folder_path_to_pathbuf(&property.folder_path);
            let old_path = done_base_path.join(&folder_path_buf);
            let new_path = archive_base_path.join(&folder_path_buf);

            if old_path.exists() && old_path != new_path {
                if let Some(parent) = new_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
                std::fs::rename(&old_path, &new_path)
                    .map_err(|e| format!("Failed to move folder to archive: {}", e))?;
            }
        }
    }

    // Move properties without code to NOT_FOUND
    let not_found_base_path = get_base_path_for_status(&config, "NOT_FOUND")?;
    let properties_moved_to_not_found = without_code.len();
    for property in &without_code {
        if let Some(property_id) = property.id {
            // Update status in database
            let now_ts = chrono::Utc::now().timestamp_millis();
            sqlx::query("UPDATE properties SET status = 'NOT_FOUND', updated_at = ? WHERE id = ?")
                .bind(now_ts)
                .bind(property_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Failed to update property status: {}", e))?;

            // Move folder - use folder_path which has the actual folder name
            let folder_path_buf = folder_path_to_pathbuf(&property.folder_path);
            let old_path = done_base_path.join(&folder_path_buf);
            let new_path = not_found_base_path.join(&folder_path_buf);

            if old_path.exists() && old_path != new_path {
                if let Some(parent) = new_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
                std::fs::rename(&old_path, &new_path)
                    .map_err(|e| format!("Failed to move folder to not found: {}", e))?;
            }
        }
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
    let config = crate::config::load_config(app.clone())
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
        let zip_file = PathBuf::from(&zip_path);
        if zip_file.exists() {
            std::fs::remove_file(&zip_file)
                .map_err(|e| format!("Failed to delete ZIP file: {}", e))?;
        }
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
