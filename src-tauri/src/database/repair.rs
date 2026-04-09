//! Repair command: walks every property in the database and verifies
//! its `folder_path` + `status` row matches what is actually on disk,
//! correcting any drift. Used after manual filesystem changes.
//!
//! Extracted from database.rs in the database-module split.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::database::scan::find_folder_by_prefix;
use crate::database::types::CommandResult;
use crate::database::{folder_path_to_pathbuf, get_base_path_for_status, get_database_pool};

/// Repair result structure
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct RepairResult {
    #[ts(type = "number")]
    pub properties_checked: usize,
    #[ts(type = "number")]
    pub properties_fixed: usize,
    pub errors: Vec<String>,
}

/// Repair property statuses by checking actual folder locations
/// This fixes properties where the database status doesn't match where the folder actually exists
/// Also handles folder name mismatches (e.g., when folder has code suffix but DB doesn't)
#[tauri::command]
pub async fn repair_property_statuses(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let pool = get_database_pool(&app)?;

    let config = crate::config::get_cached_config(&app)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("App configuration not found")?;

    let mut result = RepairResult {
        properties_checked: 0,
        properties_fixed: 0,
        errors: Vec::new(),
    };

    // Get all properties from database
    let properties: Vec<(i64, String, String, String)> =
        sqlx::query_as("SELECT id, folder_path, status, name FROM properties")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to fetch properties: {}", e))?;

    // Get base paths for all statuses
    let status_paths: Vec<(String, Option<PathBuf>)> = vec![
        (
            "NEW".to_string(),
            get_base_path_for_status(&config, "NEW").ok(),
        ),
        (
            "DONE".to_string(),
            get_base_path_for_status(&config, "DONE").ok(),
        ),
        (
            "NOT_FOUND".to_string(),
            get_base_path_for_status(&config, "NOT_FOUND").ok(),
        ),
        (
            "ARCHIVE".to_string(),
            get_base_path_for_status(&config, "ARCHIVE").ok(),
        ),
    ];

    // Phase 1: Check filesystem locations on a blocking thread.
    // Each row: (id, folder_path, db_status, name, found_full_path,
    // found_status, errors). Hoisted to a type alias to satisfy the
    // clippy::type_complexity lint.
    type ScanRow = (
        i64,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Vec<String>,
    );
    let properties_clone = properties.clone();
    let status_paths_clone = status_paths.clone();
    let scan_results: Vec<ScanRow> = tokio::task::spawn_blocking(move || {
        let mut results = Vec::new();

        for (id, folder_path, db_status, name) in properties_clone {
            let parts: Vec<&str> = folder_path.split('/').collect();
            if parts.len() != 2 {
                results.push((
                    id,
                    folder_path,
                    db_status,
                    name,
                    None,
                    None,
                    vec![format!("Property has invalid folder_path format")],
                ));
                continue;
            }
            let city = parts[0];
            let property_folder_name = parts[1];
            let folder_path_buf = folder_path_to_pathbuf(&folder_path);

            let mut found_info: Option<(String, Option<String>)> = None;

            // Try exact match
            for (status, base_path_opt) in &status_paths_clone {
                if let Some(base_path) = base_path_opt {
                    let full_path = base_path.join(&folder_path_buf);
                    if full_path.exists() {
                        found_info = Some((status.clone(), None));
                        break;
                    }
                }
            }

            // Try prefix matching
            if found_info.is_none() {
                for (status, base_path_opt) in &status_paths_clone {
                    if let Some(base_path) = base_path_opt {
                        let city_path = base_path.join(city);
                        if let Some(actual_folder_name) =
                            find_folder_by_prefix(&city_path, property_folder_name)
                        {
                            if actual_folder_name != property_folder_name {
                                found_info = Some((status.clone(), Some(actual_folder_name)));
                            } else {
                                found_info = Some((status.clone(), None));
                            }
                            break;
                        }
                    }
                }
            }

            let (found_status, new_folder_name) = match found_info {
                Some((s, n)) => (Some(s), n),
                None => (None, None),
            };

            let errors = if found_status.is_none() {
                let checked_paths: Vec<String> = status_paths_clone
                    .iter()
                    .filter_map(|(status, base_path_opt)| {
                        base_path_opt.as_ref().map(|bp| {
                            format!("{}: {}", status, bp.join(&folder_path_buf).display())
                        })
                    })
                    .collect();
                vec![format!(
                    "Property '{}' folder not found. DB folder_path='{}'. Checked: [{}]",
                    name,
                    folder_path,
                    checked_paths.join(", ")
                )]
            } else {
                vec![]
            };

            results.push((
                id,
                folder_path,
                db_status,
                name,
                found_status,
                new_folder_name,
                errors,
            ));
        }

        results
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    // Phase 2: Update database based on scan results
    for (id, folder_path, db_status, name, found_status, new_folder_name_opt, errors) in
        scan_results
    {
        result.properties_checked += 1;
        result.errors.extend(errors);

        if let Some(found_status) = found_status {
            let status_changed = found_status != db_status;
            let folder_path_changed = new_folder_name_opt.is_some();

            if status_changed || folder_path_changed {
                let parts: Vec<&str> = folder_path.split('/').collect();
                let city = parts[0];
                let now_ts = chrono::Utc::now().timestamp_millis();
                let new_folder_path = if let Some(ref new_name) = new_folder_name_opt {
                    format!("{}/{}", city, new_name)
                } else {
                    folder_path.clone()
                };

                match sqlx::query("UPDATE properties SET status = ?, folder_path = ?, updated_at = ? WHERE id = ?")
                    .bind(&found_status)
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
        }
    }

    Ok(CommandResult {
        success: true,
        error: None,
        data: Some(serde_json::to_value(result).map_err(|e| e.to_string())?),
    })
}
