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
    Ok(base_path.join(folder_path))
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
    let city: String = property_row.get("city");
    let name: String = property_row.get("name");

    // folder_path remains relative (city/name), no change needed
    let folder_path = get_relative_folder_path(&city, &name);

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
        Ok(_) => {
            // Move folder if status changed
            if current_status != new_status {
                let config_result = crate::config::load_config(app.clone()).await;
                if let Ok(Some(config)) = config_result {
                    match (
                        construct_property_path_from_parts(&config, &current_status, &city, &name),
                        construct_property_path_from_parts(&config, &new_status, &city, &name),
                    ) {
                        (Ok(old_path), Ok(new_path)) => {
                            if old_path.exists() && old_path != new_path {
                                // Create parent directory for new path
                                if let Some(parent) = new_path.parent() {
                                    std::fs::create_dir_all(parent).map_err(|e| {
                                        format!("Failed to create parent directory: {}", e)
                                    })?;
                                }
                                // Move the folder
                                std::fs::rename(&old_path, &new_path)
                                    .map_err(|e| format!("Failed to move folder: {}", e))?;
                            }
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            return Ok(CommandResult {
                                success: false,
                                error: Some(format!("Failed to get property path: {}", e)),
                                data: None,
                            });
                        }
                    }
                }
            }

            Ok(CommandResult {
                success: true,
                error: None,
                data: None,
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
    let old_code: Option<String> = property_row.get("code");

    // Calculate old and new folder names
    let old_folder_name = match &old_code {
        Some(c) if !c.is_empty() => format!("{} ({})", name, c),
        _ => name.clone(),
    };
    let new_folder_name = format!("{} ({})", name, code);

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
    let full_path = base_path.join(&folder_path);
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
    let mut added_count = 0;
    let mut errors: Vec<String> = Vec::new();

    for i in 0..images_to_add {
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
            Ok(_) => {
                added_count += 1;

                // Also create cropped version for WATERMARK/AGGELIA if source exists there
                let watermark_source = watermark_aggelia_path.join(source_filename);
                if watermark_source.exists() {
                    if let Err(e) = crop_and_save_image(&watermark_source, &dest_watermark_path) {
                        errors.push(format!("Failed to create watermark copy for {}: {}", new_filename, e));
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Failed to create {}: {}", new_filename, e));
            }
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
