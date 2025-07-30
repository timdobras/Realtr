use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tokio::process::Command;
use std::path::PathBuf;
use std::collections::HashSet;
use tauri::Manager;
use std::fs;
use base64::{Engine as _, engine::general_purpose};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Property {
    pub id: Option<i64>,
    pub name: String,
    pub city: String,
    pub completed: bool,
    pub folder_path: String,
    pub notes: Option<String>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
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

// Database initialization
pub async fn init_database(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    
    // Ensure the directory exists with proper error handling
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("Failed to create app data directory {}: {}", app_data_dir.display(), e))?;
    }
    
    let database_path = app_data_dir.join("properties.db");
    
    println!("Attempting to connect to database at: {}", database_path.display());
    
    // Set connection options for SQLite
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    )
    .await
    .map_err(|e| format!("Failed to connect to database at {}: {}", database_path.display(), e))?;
    
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
    
    let folder_path = format!("FOTOGRAFIES - NEW/{}/{}", city, name);
    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();
    
    // Start a transaction
    let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
    
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
        INSERT INTO properties (name, city, completed, folder_path, notes, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&name)
    .bind(&city)
    .bind(false)
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
            tx.commit().await.map_err(|e| format!("Failed to commit transaction: {}", e))?;
            
            // Create the folder structure
            let config_result = crate::config::load_config(app.clone()).await;
            if let Ok(Some(config)) = config_result {
                let root_path = PathBuf::from(&config.root_path);
                let property_path = root_path.join("FOTOGRAFIES - NEW").join(&city).join(&name);
                
                if let Err(e) = create_property_folder_structure(&property_path).await {
                    return Ok(CommandResult {
                        success: false,
                        error: Some(format!("Property created but folder creation failed: {}", e)),
                        data: None,
                    });
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
            completed: row.get("completed"),
            folder_path: row.get("folder_path"),
            notes: row.get("notes"),
            created_at,
            updated_at,
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
    completed: bool
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;
    
    let rows = sqlx::query("SELECT * FROM properties WHERE completed = ? ORDER BY created_at DESC")
        .bind(completed)
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
            completed: row.get("completed"),
            folder_path: row.get("folder_path"),
            notes: row.get("notes"),
            created_at,
            updated_at,
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
    completed: bool,
) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;
    
    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();
    
    // Get current property info
    let property_row = sqlx::query("SELECT * FROM properties WHERE id = ?")
        .bind(property_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Property not found: {}", e))?;
    
    let current_completed: bool = property_row.get("completed");
    let city: String = property_row.get("city");
    let name: String = property_row.get("name");
    
    // Update folder path based on completion status
    let new_folder_path = if completed {
        format!("FOTOGRAFIES - DONE/{}/{}", city, name)
    } else {
        format!("FOTOGRAFIES - NEW/{}/{}", city, name)
    };
    
    let result = sqlx::query(
        "UPDATE properties SET completed = ?, folder_path = ?, updated_at = ? WHERE id = ?"
    )
    .bind(completed)
    .bind(&new_folder_path)
    .bind(now_timestamp)
    .bind(property_id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => {
            // Move folder if completion status changed
            if current_completed != completed {
                let config_result = crate::config::load_config(app.clone()).await;
                if let Ok(Some(config)) = config_result {
                    let root_path = PathBuf::from(&config.root_path);
                    let old_path = root_path.join(
                        if current_completed { "FOTOGRAFIES - DONE" } else { "FOTOGRAFIES - NEW" }
                    ).join(&city).join(&name);
                    let new_path = root_path.join(
                        if completed { "FOTOGRAFIES - DONE" } else { "FOTOGRAFIES - NEW" }
                    ).join(&city).join(&name);
                    
                    if old_path.exists() && old_path != new_path {
                        if let Some(parent) = new_path.parent() {
                            std::fs::create_dir_all(parent)
                                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                        }
                        std::fs::rename(&old_path, &new_path)
                            .map_err(|e| format!("Failed to move folder: {}", e))?;
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
pub async fn delete_property(app: tauri::AppHandle, property_id: i64) -> Result<CommandResult, String> {
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
    
    let rows = sqlx::query("SELECT * FROM cities WHERE name LIKE ? ORDER BY usage_count DESC, name ASC LIMIT 10")
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
pub async fn get_property_by_id(app: tauri::AppHandle, property_id: i64) -> Result<CommandResult, String> {
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
                completed: row.get("completed"),
                folder_path: row.get("folder_path"),
                notes: row.get("notes"),
                created_at,
                updated_at,
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
                error: Some("No configuration found. Please set up the root folder first.".to_string()),
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
    
    let root_path = PathBuf::from(&config.root_path);
    
    if !root_path.exists() {
        return Ok(CommandResult {
            success: false,
            error: Some("Root folder does not exist.".to_string()),
            data: None,
        });
    }
    
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
    
    let folders_to_scan = [
        ("FOTOGRAFIES - DONE", true),
        ("FOTOGRAFIES - NEW", false),
    ];
    
    for (folder_name, is_completed) in folders_to_scan {
        let folder_path = root_path.join(folder_name);
        
        if !folder_path.exists() {
            continue;
        }
        
        match scan_folder_for_properties(&folder_path, is_completed, &existing_properties, pool).await {
            Ok(folder_result) => {
                scan_result.found_properties += folder_result.found_properties;
                scan_result.new_properties += folder_result.new_properties;
                scan_result.existing_properties += folder_result.existing_properties;
                scan_result.errors.extend(folder_result.errors);
            }
            Err(e) => {
                scan_result.errors.push(format!("Error scanning {}: {}", folder_name, e));
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
    let rows = sqlx::query("SELECT city, name FROM properties")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch existing properties: {}", e))?;
    
    let mut existing = HashSet::new();
    for row in rows {
        let city: String = row.get("city");
        let name: String = row.get("name");
        existing.insert(format!("{}/{}", city, name));
    }
    
    Ok(existing)
}

async fn scan_folder_for_properties(
    folder_path: &PathBuf,
    is_completed: bool,
    existing_properties: &HashSet<String>,
    pool: &SqlitePool,
) -> Result<ScanResult, String> {
    let mut result = ScanResult {
        found_properties: 0,
        new_properties: 0,
        existing_properties: 0,
        errors: Vec::new(),
    };
    
    let entries = std::fs::read_dir(folder_path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;
    
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                result.errors.push(format!("Error reading directory entry: {}", e));
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
                result.errors.push(format!("Invalid city folder name: {:?}", city_path));
                continue;
            }
        };
        
        let city_entries = match std::fs::read_dir(&city_path) {
            Ok(entries) => entries,
            Err(e) => {
                result.errors.push(format!("Failed to read city folder {}: {}", city_name, e));
                continue;
            }
        };
        
        for property_entry in city_entries {
            let property_entry = match property_entry {
                Ok(entry) => entry,
                Err(e) => {
                    result.errors.push(format!("Error reading property entry in {}: {}", city_name, e));
                    continue;
                }
            };
            
            let property_path = property_entry.path();
            if !property_path.is_dir() {
                continue;
            }
            
            let property_name = match property_path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => {
                    result.errors.push(format!("Invalid property folder name: {:?}", property_path));
                    continue;
                }
            };
            
            result.found_properties += 1;
            
            let property_key = format!("{}/{}", city_name, property_name);
            
            if existing_properties.contains(&property_key) {
                result.existing_properties += 1;
                continue;
            }
            
            if !is_valid_property_folder(&property_path) {
                result.errors.push(format!("Invalid property structure: {}", property_key));
                continue;
            }
            
            match add_property_to_database(pool, &property_name, &city_name, is_completed).await {
                Ok(_) => {
                    result.new_properties += 1;
                }
                Err(e) => {
                    result.errors.push(format!("Failed to add property {}: {}", property_key, e));
                }
            }
        }
    }
    
    Ok(result)
}

fn is_valid_property_folder(property_path: &PathBuf) -> bool {
    property_path.join("INTERNET").is_dir() && property_path.join("WATERMARK").is_dir()
}

async fn add_property_to_database(
    pool: &SqlitePool,
    property_name: &str,
    city_name: &str,
    is_completed: bool,
) -> Result<(), String> {
    let folder_path = if is_completed {
        format!("FOTOGRAFIES - DONE/{}/{}", city_name, property_name)
    } else {
        format!("FOTOGRAFIES - NEW/{}/{}", city_name, property_name)
    };
    
    let now = chrono::Utc::now();
    let now_timestamp = now.timestamp_millis();
    
    let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction: {}", e))?;
    
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
        INSERT INTO properties (name, city, completed, folder_path, notes, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(property_name)
    .bind(city_name)
    .bind(is_completed)
    .bind(&folder_path)
    .bind("Imported from existing folder")
    .bind(now_timestamp)
    .bind(now_timestamp)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to insert property: {}", e))?;
    
    tx.commit().await.map_err(|e| format!("Failed to commit transaction: {}", e))?;
    
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
pub async fn reset_database_with_proper_dates(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let database_path = app_data_dir.join("properties.db");
    
    // Close any existing connections and remove the file
    if database_path.exists() {
        std::fs::remove_file(&database_path)
            .map_err(|e| format!("Failed to remove old database: {}", e))?;
    }
    
    // Reinitialize the database
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
    )
    .await
    .map_err(|e| format!("Failed to connect to new database: {}", e))?;
    
    // Create tables with proper INTEGER timestamps
    sqlx::query(
        r#"
        CREATE TABLE properties (
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
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to create properties table: {}", e))?;

    sqlx::query(
        r#"
        CREATE TABLE cities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            usage_count INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to create cities table: {}", e))?;

    // Create indexes
    sqlx::query("CREATE INDEX idx_properties_completed ON properties(completed)")
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to create completed index: {}", e))?;

    sqlx::query("CREATE INDEX idx_properties_city ON properties(city)")
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to create city index: {}", e))?;

    sqlx::query("CREATE INDEX idx_cities_name ON cities(name)")
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to create cities name index: {}", e))?;

    // Update the app's managed state with the new pool
    app.manage(pool);
    
    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::json!({"message": "Database reset successfully"})),
    })
}

#[tauri::command]
pub async fn list_original_images(app: tauri::AppHandle, folder_path: String) -> Result<Vec<String>, String> {
    // Here, folder_path is relative to root folder selected by user
    // We'll get the root folder from config, then join with folder_path

    // Load config to get root path
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


#[tauri::command]
pub async fn open_images_in_folder(
    app: tauri::AppHandle,
    folder_path: String,
    selected_image: String,
) -> Result<CommandResult, String> {
    
    // Get the root path from config
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let full_folder_path = root_path.join(&folder_path);
    if !full_folder_path.exists() || !full_folder_path.is_dir() {
        return Ok(CommandResult {
            success: false,
            error: Some(format!("Folder path does not exist: {}", full_folder_path.display())),
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
    let paths_strs: Vec<String> = ordered_paths.iter()
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
        // For Windows, try opening the selected image first
        // This gives the best chance for Photos app to get folder context
        Command::new("cmd")
            .args(["/C", "start", "", &paths_strs[0]])
            .spawn()
    } else if cfg!(target_os = "macos") {
        // macOS can handle multiple files
        Command::new("open")
            .args(&paths_strs)
            .spawn()
    } else {
        // Linux - open just the selected image
        Command::new("xdg-open")
            .arg(&paths_strs[0])
            .spawn()
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
    filename: String,
) -> Result<String, String> {
    // Get root path from config
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let full_path = root_path.join(&folder_path).join(&filename);
    
    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    // Read file bytes
    let image_bytes = fs::read(&full_path)
        .map_err(|e| format!("Failed to read image file: {}", e))?;

    // Convert to base64
    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    
    Ok(base64_string)
}



#[tauri::command]
pub async fn list_internet_images(
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<Vec<String>, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(c) => PathBuf::from(c.root_path),
        None => return Err("App root folder not configured".into()),
    };

    let internet_path = root_path.join(&folder_path).join("INTERNET");
    
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
    filename: String,
) -> Result<String, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let full_path = root_path.join(&folder_path).join("INTERNET").join(&filename);
    
    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    let image_bytes = fs::read(&full_path)
        .map_err(|e| format!("Failed to read image file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

#[tauri::command]
pub async fn copy_images_to_internet(
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let property_path = root_path.join(&folder_path);
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
                                Ok(_) => copied_count += 1,
                                Err(e) => errors.push(format!("Failed to copy {}: {}", filename, e)),
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
            error: Some(format!("Copied {} images but encountered errors: {}", copied_count, errors.join(", "))),
            data: None,
        })
    }
}

#[tauri::command]
pub async fn clear_internet_folder(
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let internet_path = root_path.join(&folder_path).join("INTERNET");

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
            error: Some(format!("Deleted {} images but encountered errors: {}", deleted_count, errors.join(", "))),
            data: None,
        })
    }
}

#[tauri::command]
pub async fn open_image_in_editor(
    app: tauri::AppHandle,
    folder_path: String,
    filename: String,
    is_from_internet: bool,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    let root_path = PathBuf::from(&config.root_path);
    let image_path = if is_from_internet {
        root_path.join(&folder_path).join("INTERNET").join(&filename)
    } else {
        root_path.join(&folder_path).join(&filename)
    };

    if !image_path.exists() {
        return Ok(CommandResult {
            success: false,
            error: Some(format!("Image file not found: {}", image_path.display())),
            data: None,
        });
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
    rename_map: Vec<RenameMapping>,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let internet_path = root_path.join(&folder_path).join("INTERNET");

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
                    errors.push(format!("Failed to rename {} to temp: {}", mapping.old_name, e));
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
                errors.push(format!("Failed to rename {} to {}: {}", temp_name, final_name, e));
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
            error: Some(format!("Renamed {} images but encountered errors: {}", renamed_count, errors.join(", "))),
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
) -> Result<Vec<String>, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(c) => PathBuf::from(c.root_path),
        None => return Err("App root folder not configured".into()),
    };

    let aggelia_path = root_path.join(&folder_path).join("INTERNET").join("AGGELIA");
    
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
    filename: String,
) -> Result<String, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let full_path = root_path.join(&folder_path).join("INTERNET").join("AGGELIA").join(&filename);
    
    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    let image_bytes = fs::read(&full_path)
        .map_err(|e| format!("Failed to read image file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

#[tauri::command]
pub async fn copy_images_to_aggelia(
    app: tauri::AppHandle,
    folder_path: String,
    filenames: Vec<String>,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let property_path = root_path.join(&folder_path);
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
            error: Some(format!("Copied {} images but encountered errors: {}", copied_count, errors.join(", "))),
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
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let aggelia_path = root_path.join(&folder_path).join("INTERNET").join("AGGELIA");

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
            error: Some(format!("Deleted {} images but encountered errors: {}", deleted_count, errors.join(", "))),
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
    filename: String,
    from_aggelia: bool,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    let root_path = PathBuf::from(&config.root_path);
    let image_path = if from_aggelia {
        root_path.join(&folder_path).join("INTERNET").join("AGGELIA").join(&filename)
    } else {
        root_path.join(&folder_path).join("INTERNET").join(&filename)
    };

    if !image_path.exists() {
        return Ok(CommandResult {
            success: false,
            error: Some(format!("Image file not found: {}", image_path.display())),
            data: None,
        });
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
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    if config.watermark_image_path.is_none() {
        return Ok(CommandResult {
            success: false,
            error: Some("Watermark image not configured. Please set it in settings first.".to_string()),
            data: None,
        });
    }

    let root_path = PathBuf::from(&config.root_path);
    let property_path = root_path.join(&folder_path);
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
    let watermark_img_path = config.watermark_image_path.unwrap();
    let watermark_img = image::open(&watermark_img_path)
        .map_err(|e| format!("Failed to load watermark image: {}", e))?;

    let mut processed_count = 0;
    let mut errors = Vec::new();

    // Process INTERNET folder -> WATERMARK folder
    if internet_path.exists() {
        match copy_and_process_folder(&internet_path, &watermark_path, &watermark_img, config.watermark_opacity) {
            Ok(count) => processed_count += count,
            Err(e) => errors.push(format!("INTERNET folder: {}", e)),
        }
    }

    // Process INTERNET/AGGELIA folder -> WATERMARK/AGGELIA folder
    if aggelia_path.exists() {
        match copy_and_process_folder(&aggelia_path, &watermark_aggelia_path, &watermark_img, config.watermark_opacity) {
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
            error: Some(format!("Processed {} images but encountered errors: {}", processed_count, errors.join(", "))),
            data: Some(serde_json::json!({
                "processed_count": processed_count,
                "errors": errors
            })),
        })
    }
}

fn copy_and_process_folder(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    watermark_img: &DynamicImage,
    opacity: f32,
) -> Result<usize, String> {
    let mut processed_count = 0;

    for entry in fs::read_dir(source_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lc = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "gif", "webp"].contains(&ext_lc.as_str()) {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        let dest_file = dest_path.join(filename);
                        
                        // Copy and watermark
                        match apply_watermark_to_image(&path, &dest_file, watermark_img, opacity) {
                            Ok(_) => processed_count += 1,
                            Err(e) => return Err(format!("Failed to process {}: {}", filename, e)),
                        }
                    }
                }
            }
        }
    }

    Ok(processed_count)
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

    // Resize watermark
    let resized_watermark = watermark_img
        .resize_exact(new_wm_width, new_wm_height, image::imageops::FilterType::Lanczos3)
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

    // Save watermarked image
    base_img.save(dest_path)
        .map_err(|e| format!("Failed to save watermarked image: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn list_watermark_images(
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<Vec<String>, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(c) => PathBuf::from(c.root_path),
        None => return Err("App root folder not configured".into()),
    };

    let watermark_path = root_path.join(&folder_path).join("WATERMARK");
    
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
) -> Result<Vec<String>, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(c) => PathBuf::from(c.root_path),
        None => return Err("App root folder not configured".into()),
    };

    let watermark_aggelia_path = root_path.join(&folder_path).join("WATERMARK").join("AGGELIA");
    
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
    filename: String,
    from_aggelia: bool,
) -> Result<String, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let full_path = if from_aggelia {
        root_path.join(&folder_path).join("WATERMARK").join("AGGELIA").join(&filename)
    } else {
        root_path.join(&folder_path).join("WATERMARK").join(&filename)
    };
    
    if !full_path.exists() {
        return Err(format!("Image file not found: {}", full_path.display()));
    }

    let image_bytes = fs::read(&full_path)
        .map_err(|e| format!("Failed to read image file: {}", e))?;

    let base64_string = general_purpose::STANDARD.encode(&image_bytes);
    Ok(base64_string)
}

#[tauri::command]
pub async fn clear_watermark_folders(
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let root_path = match config {
        Some(cfg) => PathBuf::from(cfg.root_path),
        None => return Err("App root path is not configured".into()),
    };

    let watermark_path = root_path.join(&folder_path).join("WATERMARK");
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
            error: Some(format!("Deleted {} images but encountered errors: {}", deleted_count, errors.join(", "))),
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
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    let root_path = PathBuf::from(&config.root_path);
    let full_path = root_path.join(&folder_path);
    
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
) -> Result<CommandResult, String> {
    let config = crate::config::load_config(app.clone()).await.map_err(|e| e.to_string())?;
    let config = match config {
        Some(cfg) => cfg,
        None => return Err("App configuration not found".into()),
    };

    let full_path = PathBuf::from(&config.root_path).join(&folder_path);
    
    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::json!({
            "full_path": full_path.to_string_lossy(),
            "relative_path": folder_path
        })),
    })
}