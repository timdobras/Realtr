//! OpenCV setup and verification for the perspective correction feature.
//!
//! Handles checking if OpenCV is installed and running the setup process.

use std::path::PathBuf;
use std::process::Command;
use tauri::Manager;

/// Check if OpenCV DLLs are available
/// Returns true if OpenCV is ready to use
#[tauri::command]
pub fn check_opencv_status() -> Result<OpenCVStatus, String> {
    // Check multiple possible locations for OpenCV DLLs
    let possible_paths = [
        // Bundled with app (production)
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("opencv"))),
        // Dev mode - check src-tauri/opencv
        Some(PathBuf::from("opencv")),
        // System installation via Chocolatey
        Some(PathBuf::from(r"C:\tools\opencv\build\x64\vc16\bin")),
        Some(PathBuf::from(r"C:\tools\opencv\build\x64\vc15\bin")),
        // Environment variable
        std::env::var("OPENCV_LINK_PATHS").ok().map(PathBuf::from),
    ];

    // Look for opencv_world DLL
    for path_opt in &possible_paths {
        if let Some(path) = path_opt {
            if path.exists() {
                // Check for opencv_world*.dll
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_lowercase();
                        if name.starts_with("opencv_world") && name.ends_with(".dll") {
                            return Ok(OpenCVStatus {
                                installed: true,
                                dll_path: Some(entry.path().to_string_lossy().to_string()),
                                message: "OpenCV is installed and ready".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check if LLVM/Clang is installed (needed for building)
    let llvm_installed = Command::new("clang")
        .arg("--version")
        .output()
        .is_ok();

    Ok(OpenCVStatus {
        installed: false,
        dll_path: None,
        message: if llvm_installed {
            "OpenCV DLLs not found. Setup required.".to_string()
        } else {
            "OpenCV and LLVM not found. Full setup required.".to_string()
        },
    })
}

/// Status of OpenCV installation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenCVStatus {
    pub installed: bool,
    pub dll_path: Option<String>,
    pub message: String,
}

/// Setup progress update
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SetupProgress {
    pub step: u32,
    pub total_steps: u32,
    pub message: String,
    pub complete: bool,
    pub error: Option<String>,
}

/// Run the OpenCV setup script with admin privileges
#[tauri::command]
pub async fn run_opencv_setup(app: tauri::AppHandle) -> Result<SetupProgress, String> {
    // Get the path to the setup script
    let script_path = get_setup_script_path(&app)?;

    if !script_path.exists() {
        return Err(format!(
            "Setup script not found at: {}",
            script_path.display()
        ));
    }

    // Run PowerShell with elevation (UAC prompt)
    let status = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process powershell -Verb runAs -ArgumentList '-ExecutionPolicy Bypass -File \"{}\"' -Wait",
                script_path.display()
            ),
        ])
        .status()
        .map_err(|e| format!("Failed to run setup: {e}"))?;

    if status.success() {
        // Verify the installation worked
        let opencv_status = check_opencv_status()?;

        if opencv_status.installed {
            Ok(SetupProgress {
                step: 5,
                total_steps: 5,
                message: "Setup complete! OpenCV is ready.".to_string(),
                complete: true,
                error: None,
            })
        } else {
            Ok(SetupProgress {
                step: 5,
                total_steps: 5,
                message: "Setup completed but verification failed. Please restart the app.".to_string(),
                complete: true,
                error: Some("OpenCV DLLs not found after setup. You may need to restart your computer.".to_string()),
            })
        }
    } else {
        Err("Setup was cancelled or failed. Please try again.".to_string())
    }
}

/// Get the path to the setup script
fn get_setup_script_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // In development, look in the project scripts folder
    let dev_path = PathBuf::from("../scripts/setup-opencv.ps1");
    if dev_path.exists() {
        return Ok(dev_path.canonicalize().map_err(|e| e.to_string())?);
    }

    // In production, the script should be bundled as a resource
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {e}"))?
        .join("scripts")
        .join("setup-opencv.ps1");

    if resource_path.exists() {
        return Ok(resource_path);
    }

    // Fallback: look relative to exe
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("No parent dir")?;
    let script_path = exe_dir.join("scripts").join("setup-opencv.ps1");

    if script_path.exists() {
        return Ok(script_path);
    }

    Err(format!(
        "Setup script not found. Checked:\n- {}\n- {}\n- {}",
        dev_path.display(),
        resource_path.display(),
        script_path.display()
    ))
}

/// Skip the setup (user chooses to proceed without OpenCV)
/// This disables the perspective correction feature
#[tauri::command]
pub fn skip_opencv_setup(app: tauri::AppHandle) -> Result<(), String> {
    // Store a flag indicating the user skipped setup
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {e}"))?;

    let flag_path = app_data_dir.join(".opencv_setup_skipped");
    std::fs::write(&flag_path, "skipped")
        .map_err(|e| format!("Failed to write skip flag: {e}"))?;

    Ok(())
}

/// Check if the user previously skipped setup
#[tauri::command]
pub fn was_opencv_setup_skipped(app: tauri::AppHandle) -> Result<bool, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    let flag_path = app_data_dir.join(".opencv_setup_skipped");
    Ok(flag_path.exists())
}

/// Reset the skip flag (allow setup prompt again)
#[tauri::command]
pub fn reset_opencv_setup_skip(app: tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    let flag_path = app_data_dir.join(".opencv_setup_skipped");
    if flag_path.exists() {
        std::fs::remove_file(&flag_path)
            .map_err(|e| format!("Failed to remove skip flag: {e}"))?;
    }

    Ok(())
}
