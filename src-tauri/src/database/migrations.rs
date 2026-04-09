//! Database initialisation and schema migrations.
//!
//! `init_database` is called once on app startup from main.rs and returns
//! the SQLx pool that the rest of the app stores in Tauri state.
//!
//! Extracted from database.rs in the database-module split.

use sqlx::{Row, SqlitePool};
use tauri::Manager;

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

pub(super) async fn run_migrations(pool: &SqlitePool) -> Result<(), String> {
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

    // Used by `SELECT * FROM properties ORDER BY created_at DESC` (property list)
    // and `... WHERE status = ? ORDER BY created_at DESC`. The status index alone
    // does not let SQLite avoid a sort.
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_properties_created_at ON properties(created_at DESC)",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create properties created_at index: {}", e))?;

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
