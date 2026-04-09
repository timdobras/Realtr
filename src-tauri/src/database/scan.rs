//! Filesystem scan + import: walks the configured NEW/DONE/NOT_FOUND/
//! ARCHIVE folders, finds property folders, and imports any not yet
//! tracked in the database. Owns the find_folder_by_prefix helper that
//! repair_property_statuses also relies on.
//!
//! Extracted from database.rs in the database-module split.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};

use crate::database::types::{CommandResult, ScanResult};
use crate::database::{
    get_database_pool, get_relative_folder_path, is_valid_property_folder, parse_folder_name,
};

// Scan and import properties function
#[tauri::command]
pub async fn scan_and_import_properties(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let config_result = crate::config::get_cached_config(&app).await;
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

        match scan_folder_for_properties(&folder_path, status, &existing_properties, pool).await {
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

/// Helper function to find a folder by prefix match within a city directory
/// This handles cases where folder has a code suffix like "PROPERTY NAME (12345)"
pub(super) fn find_folder_by_prefix(city_path: &PathBuf, property_name: &str) -> Option<String> {
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

pub(super) async fn get_existing_properties_set(
    pool: &SqlitePool,
) -> Result<HashSet<String>, String> {
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

pub(super) async fn scan_folder_for_properties(
    folder_path: &Path,
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

    // Filesystem scan runs on a blocking thread to avoid stalling the Tokio runtime
    let folder_path_clone = folder_path.to_path_buf();
    let status_clone = status.to_string();
    let existing_clone = existing_properties.clone();

    let (new_properties_to_insert, scan_found, scan_existing, scan_errors) =
        tokio::task::spawn_blocking(move || {
            let mut new_props: Vec<(String, String, String, String, Option<String>, String)> =
                Vec::new();
            let mut found = 0usize;
            let mut existing = 0usize;
            let mut errors = Vec::new();

            let entries = match fs::read_dir(&folder_path_clone) {
                Ok(e) => e,
                Err(e) => return Err(format!("Failed to read directory: {}", e)),
            };

            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        errors.push(format!("Error reading directory entry: {}", e));
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
                        errors.push(format!("Invalid city folder name: {:?}", city_path));
                        continue;
                    }
                };

                let city_entries = match fs::read_dir(&city_path) {
                    Ok(entries) => entries,
                    Err(e) => {
                        errors.push(format!("Failed to read city folder {}: {}", city_name, e));
                        continue;
                    }
                };

                for property_entry in city_entries {
                    let property_entry = match property_entry {
                        Ok(entry) => entry,
                        Err(e) => {
                            errors.push(format!(
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
                            errors
                                .push(format!("Invalid property folder name: {:?}", property_path));
                            continue;
                        }
                    };

                    let (property_name, code) = parse_folder_name(&folder_name);

                    found += 1;

                    let property_key = format!("{}/{}", city_name, folder_name);

                    if existing_clone.contains(&property_key) {
                        existing += 1;
                        continue;
                    }

                    if !is_valid_property_folder(&property_path) {
                        errors.push(format!("Invalid property structure: {}", property_key));
                        continue;
                    }

                    new_props.push((
                        property_name,
                        city_name.clone(),
                        status_clone.clone(),
                        folder_name,
                        code,
                        property_key,
                    ));
                }
            }

            Ok((new_props, found, existing, errors))
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))??;

    result.found_properties = scan_found;
    result.existing_properties = scan_existing;
    result.errors = scan_errors;

    // Batch-insert all new properties in a single transaction.
    // SQLite is ~100x faster with batched transactions vs one-per-insert.
    if !new_properties_to_insert.is_empty() {
        let now = chrono::Utc::now().timestamp_millis();

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("Failed to start batch transaction: {}", e))?;

        for (property_name, city_name, status_str, folder_name, code, property_key) in
            &new_properties_to_insert
        {
            let folder_path = get_relative_folder_path(city_name, folder_name);

            // Upsert city
            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO cities (name, usage_count, created_at)
                VALUES (?, 1, ?)
                ON CONFLICT(name) DO UPDATE SET usage_count = usage_count + 1
                "#,
            )
            .bind(city_name)
            .bind(now)
            .execute(&mut *tx)
            .await
            {
                result
                    .errors
                    .push(format!("Failed to update city for {}: {}", property_key, e));
                continue;
            }

            // Insert property
            match sqlx::query(
                r#"
                INSERT INTO properties (name, city, status, folder_path, notes, code, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(property_name)
            .bind(city_name)
            .bind(status_str)
            .bind(&folder_path)
            .bind("Imported from existing folder")
            .bind(code.as_deref())
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
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

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit batch transaction: {}", e))?;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_support::{add_property_to_database, setup_test_db};
    use std::collections::HashSet;
    use std::fs;

    #[tokio::test]
    async fn get_existing_properties_set_empty_db() {
        let pool = setup_test_db().await;
        let set = get_existing_properties_set(&pool).await.unwrap();
        assert!(set.is_empty());
    }

    #[tokio::test]
    async fn get_existing_properties_set_returns_folder_paths() {
        let pool = setup_test_db().await;

        add_property_to_database(&pool, "Villa A", "Athens", "NEW", "Villa A", None)
            .await
            .unwrap();
        add_property_to_database(&pool, "Villa B", "Rome", "DONE", "Villa B", None)
            .await
            .unwrap();

        let set = get_existing_properties_set(&pool).await.unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains("Athens/Villa A"));
        assert!(set.contains("Rome/Villa B"));
    }

    #[tokio::test]
    async fn scan_finds_new_properties() {
        let pool = setup_test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        // Create folder structure: base/CityA/PropertyX, base/CityA/PropertyY
        let city_dir = tmp.path().join("CityA");
        fs::create_dir_all(city_dir.join("PropertyX")).unwrap();
        fs::create_dir_all(city_dir.join("PropertyY")).unwrap();

        let existing = HashSet::new();
        let result = scan_folder_for_properties(&tmp.path().to_path_buf(), "NEW", &existing, &pool)
            .await
            .unwrap();

        assert_eq!(result.found_properties, 2);
        assert_eq!(result.new_properties, 2);
        assert_eq!(result.existing_properties, 0);
        assert!(result.errors.is_empty());

        // Verify DB has the properties
        let set = get_existing_properties_set(&pool).await.unwrap();
        assert!(set.contains("CityA/PropertyX"));
        assert!(set.contains("CityA/PropertyY"));
    }

    #[tokio::test]
    async fn scan_skips_existing_properties() {
        let pool = setup_test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        let city_dir = tmp.path().join("CityA");
        fs::create_dir_all(city_dir.join("PropertyX")).unwrap();
        fs::create_dir_all(city_dir.join("PropertyY")).unwrap();

        // Mark PropertyX as already existing
        let mut existing = HashSet::new();
        existing.insert("CityA/PropertyX".to_string());

        let result = scan_folder_for_properties(&tmp.path().to_path_buf(), "NEW", &existing, &pool)
            .await
            .unwrap();

        assert_eq!(result.found_properties, 2);
        assert_eq!(result.new_properties, 1); // Only PropertyY is new
        assert_eq!(result.existing_properties, 1);
    }

    #[tokio::test]
    async fn scan_parses_code_from_folder_name() {
        let pool = setup_test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        let city_dir = tmp.path().join("CityA");
        fs::create_dir_all(city_dir.join("Villa Alpha (45164)")).unwrap();

        let result =
            scan_folder_for_properties(&tmp.path().to_path_buf(), "DONE", &HashSet::new(), &pool)
                .await
                .unwrap();

        assert_eq!(result.new_properties, 1);

        // Verify code was parsed and stored
        let row = sqlx::query("SELECT name, code, folder_path FROM properties")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("name"), "Villa Alpha");
        assert_eq!(
            row.get::<Option<String>, _>("code"),
            Some("45164".to_string())
        );
        assert_eq!(
            row.get::<String, _>("folder_path"),
            "CityA/Villa Alpha (45164)"
        );
    }

    #[tokio::test]
    async fn scan_skips_non_directory_entries() {
        let pool = setup_test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        let city_dir = tmp.path().join("CityA");
        fs::create_dir_all(&city_dir).unwrap();
        // Create a file at city level (not a property folder)
        fs::write(city_dir.join("notes.txt"), b"not a property").unwrap();
        // Also create a file at root level (not a city)
        fs::write(tmp.path().join("readme.txt"), b"not a city").unwrap();
        // Create one real property
        fs::create_dir_all(city_dir.join("RealProperty")).unwrap();

        let result =
            scan_folder_for_properties(&tmp.path().to_path_buf(), "NEW", &HashSet::new(), &pool)
                .await
                .unwrap();

        assert_eq!(result.found_properties, 1);
        assert_eq!(result.new_properties, 1);
    }

    #[tokio::test]
    async fn scan_empty_folder() {
        let pool = setup_test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        let result =
            scan_folder_for_properties(&tmp.path().to_path_buf(), "NEW", &HashSet::new(), &pool)
                .await
                .unwrap();

        assert_eq!(result.found_properties, 0);
        assert_eq!(result.new_properties, 0);
    }

    #[tokio::test]
    async fn scan_multiple_cities() {
        let pool = setup_test_db().await;
        let tmp = tempfile::tempdir().unwrap();

        fs::create_dir_all(tmp.path().join("Athens").join("Villa A")).unwrap();
        fs::create_dir_all(tmp.path().join("Rome").join("Villa B")).unwrap();
        fs::create_dir_all(tmp.path().join("Rome").join("Villa C")).unwrap();

        let result =
            scan_folder_for_properties(&tmp.path().to_path_buf(), "NEW", &HashSet::new(), &pool)
                .await
                .unwrap();

        assert_eq!(result.found_properties, 3);
        assert_eq!(result.new_properties, 3);

        // Verify cities were created with correct usage counts
        let row = sqlx::query("SELECT usage_count FROM cities WHERE name = 'Rome'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<i32, _>("usage_count"), 2);
    }
}
