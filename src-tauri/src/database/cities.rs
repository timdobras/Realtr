//! City autocomplete commands. Used by the property add/edit modals to
//! show recently-used city names ordered by usage count.
//!
//! Extracted from database.rs in the database-module split.

use sqlx::Row;

use crate::database::get_database_pool;
use crate::database::types::{City, CommandResult};

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
            .unwrap_or_else(chrono::Utc::now);

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
        data: Some(serde_json::to_value(cities).map_err(|e| e.to_string())?),
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
            .unwrap_or_else(chrono::Utc::now);

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
        data: Some(serde_json::to_value(cities).map_err(|e| e.to_string())?),
    })
}
