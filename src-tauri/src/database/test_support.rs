//! Shared test helpers for the database submodule tests.
//!
//! This module is gated on `#[cfg(test)]` so it never ships in release
//! builds. Both `database.rs::tests` and `database/scan.rs::tests`
//! import from it.

use sqlx::SqlitePool;

use crate::database::{get_relative_folder_path, migrations::run_migrations};

pub(crate) async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");
    run_migrations(&pool).await.expect("Migrations failed");
    pool
}

pub(crate) async fn add_property_to_database(
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
