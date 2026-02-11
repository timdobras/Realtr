use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

/// Cached config state managed by Tauri. Avoids re-reading config.json from disk
/// on every single command invocation (dozens of times per user interaction).
pub type ConfigCache = Arc<RwLock<Option<AppConfig>>>;

/// Helper function for serde default value
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkConfig {
    pub size_mode: String, // "proportional", "fit", "stretch", "tile"
    pub size_percentage: f32, // 0.0 to 1.0 (for proportional mode)
    pub relative_to: String, // "longest-side", "shortest-side", "width", "height"
    pub position_anchor: String, // "center", "top-left", "top-center", etc.
    pub offset_x: i32,
    pub offset_y: i32,
    pub opacity: f32,
    pub use_alpha_channel: bool,
}

impl Default for WatermarkConfig {
    fn default() -> Self {
        Self {
            size_mode: "proportional".to_string(),
            size_percentage: 0.35, // 35%
            relative_to: "longest-side".to_string(),
            position_anchor: "center".to_string(),
            offset_x: 0,
            offset_y: 0,
            opacity: 0.15, // 15%
            use_alpha_channel: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    // Legacy field for backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "rootPath")]
    pub root_path: Option<String>,
    // New modular folder paths
    #[serde(rename = "newFolderPath")]
    #[serde(default)]
    pub new_folder_path: String,
    #[serde(rename = "doneFolderPath")]
    #[serde(default)]
    pub done_folder_path: String,
    #[serde(rename = "notFoundFolderPath")]
    #[serde(default)]
    pub not_found_folder_path: String,
    #[serde(rename = "archiveFolderPath")]
    #[serde(default)]
    pub archive_folder_path: String,
    #[serde(rename = "setsFolderPath")]
    #[serde(default)]
    pub sets_folder_path: String,
    #[serde(rename = "isValidPath")]
    pub is_valid_path: bool,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    #[serde(default = "default_true")]
    pub use_builtin_editor: bool,
    pub fast_editor_path: Option<String>,
    pub fast_editor_name: Option<String>,
    pub complex_editor_path: Option<String>,
    pub complex_editor_name: Option<String>,
    pub watermark_image_path: Option<String>,
    #[serde(default)]
    pub watermark_config: WatermarkConfig,
    // Legacy field for backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark_opacity: Option<f32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            root_path: None,
            new_folder_path: String::new(),
            done_folder_path: String::new(),
            not_found_folder_path: String::new(),
            archive_folder_path: String::new(),
            sets_folder_path: String::new(),
            is_valid_path: false,
            use_builtin_editor: true,
            fast_editor_path: None,
            fast_editor_name: None,
            complex_editor_path: None,
            complex_editor_name: None,
            watermark_image_path: None,
            watermark_config: WatermarkConfig::default(),
            watermark_opacity: None,
            last_updated: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Load config from disk (internal, no caching).
/// This performs the actual file read and migration logic.
fn load_config_from_disk(app: &tauri::AppHandle) -> Result<Option<AppConfig>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_path = app_data_dir.join("config.json");

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let mut config: AppConfig =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    // Migrate old watermark_opacity to new config if present
    if let Some(old_opacity) = config.watermark_opacity {
        config.watermark_config.opacity = old_opacity;
        config.watermark_opacity = None; // Clear legacy field
    }

    // Migrate old root_path to new 4-folder structure if present
    if let Some(old_root) = config.root_path.clone() {
        if config.new_folder_path.is_empty() {
            config.new_folder_path = PathBuf::from(&old_root)
                .join("FOTOGRAFIES - NEW")
                .to_string_lossy()
                .to_string();
        }
        if config.done_folder_path.is_empty() {
            config.done_folder_path = PathBuf::from(&old_root)
                .join("FOTOGRAFIES - DONE")
                .to_string_lossy()
                .to_string();
        }
        if config.not_found_folder_path.is_empty() {
            config.not_found_folder_path = PathBuf::from(&old_root)
                .join("NOT FOUND")
                .to_string_lossy()
                .to_string();
        }
        if config.archive_folder_path.is_empty() {
            config.archive_folder_path = PathBuf::from(&old_root)
                .join("ARCHIVE")
                .to_string_lossy()
                .to_string();
        }
        config.root_path = None; // Clear legacy field after migration
    }

    Ok(Some(config))
}

/// Get the config from the in-memory cache, loading from disk on first access.
/// This is the primary entry point for all internal Rust code that needs the config.
/// Eliminates dozens of redundant file reads per user interaction.
pub async fn get_cached_config(app: &tauri::AppHandle) -> Result<Option<AppConfig>, String> {
    let cache = app.state::<ConfigCache>();

    // Fast path: read lock, check if cached
    {
        let guard = cache.read().await;
        if guard.is_some() {
            return Ok(guard.clone());
        }
    }

    // Slow path: write lock, load from disk, cache it
    let mut guard = cache.write().await;
    // Double-check after acquiring write lock (another task may have filled it)
    if guard.is_some() {
        return Ok(guard.clone());
    }

    let config = load_config_from_disk(app)?;
    *guard = config.clone();
    Ok(config)
}

/// Tauri command: load config (exposed to frontend).
/// Uses the in-memory cache for fast access.
#[tauri::command]
pub async fn load_config(app: tauri::AppHandle) -> Result<Option<AppConfig>, String> {
    get_cached_config(&app).await
}

#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    config: AppConfig,
) -> Result<CommandResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // Ensure app data directory exists
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    let config_path = app_data_dir.join("config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    // Update the in-memory cache so subsequent reads are instant
    let cache = app.state::<ConfigCache>();
    let mut guard = cache.write().await;
    *guard = Some(config);

    Ok(CommandResult {
        success: true,
        error: None,
    })
}

#[tauri::command]
pub async fn reset_config(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_path = app_data_dir.join("config.json");

    if config_path.exists() {
        std::fs::remove_file(&config_path)
            .map_err(|e| format!("Failed to remove config file: {}", e))?;
    }

    // Clear the in-memory cache
    let cache = app.state::<ConfigCache>();
    let mut guard = cache.write().await;
    *guard = None;

    Ok(CommandResult {
        success: true,
        error: None,
    })
}

#[tauri::command]
pub async fn copy_watermark_to_app_data(
    app: tauri::AppHandle,
    source_path: String,
) -> Result<CommandResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // Ensure app data directory exists
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    // Create watermark folder in app data
    let watermark_dir = app_data_dir.join("watermark");
    std::fs::create_dir_all(&watermark_dir)
        .map_err(|e| format!("Failed to create watermark directory: {}", e))?;

    // Get the filename from source path
    let source = PathBuf::from(&source_path);
    let filename = source
        .file_name()
        .ok_or("Invalid source file path")?
        .to_str()
        .ok_or("Invalid filename")?;

    // Copy to app data with a fixed name
    let dest_path = watermark_dir.join("watermark.png");

    std::fs::copy(&source, &dest_path)
        .map_err(|e| format!("Failed to copy watermark image: {}", e))?;

    Ok(CommandResult {
        success: true,
        error: None,
    })
}

#[tauri::command]
pub async fn get_watermark_from_app_data(
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let watermark_path = app_data_dir.join("watermark").join("watermark.png");

    if watermark_path.exists() {
        Ok(Some(
            watermark_path
                .to_str()
                .ok_or("Invalid path")?
                .to_string(),
        ))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn setup_folder_structure(root_path: String) -> Result<CommandResult, String> {
    let root = PathBuf::from(&root_path);

    // Check if root path exists, create if not
    if !root.exists() {
        std::fs::create_dir_all(&root)
            .map_err(|e| format!("Failed to create root directory: {}", e))?;
    }

    // Create main folders
    let fotografies_done = root.join("FOTOGRAFIES - DONE");
    let fotografies_new = root.join("FOTOGRAFIES - NEW");

    std::fs::create_dir_all(&fotografies_done)
        .map_err(|e| format!("Failed to create FOTOGRAFIES - DONE: {}", e))?;

    std::fs::create_dir_all(&fotografies_new)
        .map_err(|e| format!("Failed to create FOTOGRAFIES - NEW: {}", e))?;

    // Create a test property structure to verify everything works
    let test_property_done = fotografies_done.join("TEST/EXAMPLE-1");
    let test_property_new = fotografies_new.join("TEST/EXAMPLE-1");

    for property_path in [&test_property_done, &test_property_new] {
        std::fs::create_dir_all(property_path)
            .map_err(|e| format!("Failed to create test property directory: {}", e))?;

        // Create internet and watermark folders
        let internet_path = property_path.join("INTERNET");
        let watermark_path = property_path.join("WATERMARK");

        std::fs::create_dir_all(&internet_path)
            .map_err(|e| format!("Failed to create internet folder: {}", e))?;

        std::fs::create_dir_all(&watermark_path)
            .map_err(|e| format!("Failed to create watermark folder: {}", e))?;

        // Create AGGELIA folders inside both internet and watermark
        std::fs::create_dir_all(internet_path.join("AGGELIA"))
            .map_err(|e| format!("Failed to create internet/aggelia folder: {}", e))?;

        std::fs::create_dir_all(watermark_path.join("AGGELIA"))
            .map_err(|e| format!("Failed to create watermark/aggelia folder: {}", e))?;
    }

    Ok(CommandResult {
        success: true,
        error: None,
    })
}
