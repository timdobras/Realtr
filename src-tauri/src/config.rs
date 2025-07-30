use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(rename = "rootPath")]
    pub root_path: String,
    #[serde(rename = "isValidPath")]
    pub is_valid_path: bool,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    pub fast_editor_path: Option<String>,
    pub fast_editor_name: Option<String>,
    pub complex_editor_path: Option<String>,
    pub complex_editor_name: Option<String>,
    pub watermark_image_path: Option<String>,
    pub watermark_opacity: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            root_path: String::new(),
            is_valid_path: false,
            fast_editor_path: None,
            fast_editor_name: None,
            complex_editor_path: None,
            complex_editor_name: None,
            watermark_image_path: None,
            watermark_opacity: 0.15, // Default 30% opacity
            last_updated: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub error: Option<String>,
}

// Rest of your commands remain the same
#[tauri::command]
pub async fn load_config(app: tauri::AppHandle) -> Result<Option<AppConfig>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let config_path = app_data_dir.join("config.json");
    
    if !config_path.exists() {
        return Ok(None);
    }
    
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    
    let config: AppConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;
    
    Ok(Some(config))
}

#[tauri::command]
pub async fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<CommandResult, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    
    // Ensure app data directory exists
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    
    let config_path = app_data_dir.join("config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config file: {}", e))?;
    
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
    
    Ok(CommandResult {
        success: true,
        error: None,
    })
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

