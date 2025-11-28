#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod database;
mod opencv_setup;
mod perspective;

use config::{
    copy_watermark_to_app_data, get_watermark_from_app_data, load_config, reset_config,
    save_config, setup_folder_structure,
};
use database::{
    clear_aggelia_folder, clear_internet_folder, clear_watermark_folders,
    copy_and_watermark_images, copy_images_to_aggelia, copy_images_to_internet, create_property,
    debug_database_dates, delete_property, fill_aggelia_to_25, generate_watermark_preview,
    get_aggelia_image_as_base64, get_cities, get_full_property_path, get_image_as_base64,
    get_internet_image_as_base64, get_properties, get_properties_by_status, get_property_by_id,
    get_thumbnail_as_base64, get_watermark_image_as_base64, init_database, list_aggelia_images,
    list_internet_images, list_original_images, list_thumbnails, list_watermark_aggelia_images,
    list_watermark_images, open_image_in_advanced_editor, open_image_in_editor,
    open_images_in_folder, open_property_folder, rename_internet_images,
    reset_database_with_proper_dates, scan_and_import_properties, search_cities,
    set_property_code, update_property_status,
};
use opencv_setup::{
    check_opencv_status, reset_opencv_setup_skip, run_opencv_setup, skip_opencv_setup,
    was_opencv_setup_skipped,
};
use perspective::commands::{
    accept_perspective_corrections, cleanup_perspective_temp,
    get_original_image_for_comparison, process_images_for_perspective,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Initialize database synchronously in setup
            let app_handle = app.handle().clone();

            // Use block_on to make it synchronous
            match tauri::async_runtime::block_on(init_database(&app_handle)) {
                Ok(pool) => {
                    app_handle.manage(pool);
                    println!("Database initialized successfully");
                }
                Err(e) => {
                    eprintln!("Failed to initialize database: {}", e);
                    // Don't panic, just log the error
                    // The app will still start but database commands will fail gracefully
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            reset_config,
            setup_folder_structure,
            copy_watermark_to_app_data,
            get_watermark_from_app_data,
            create_property,
            get_properties,
            get_property_by_id,
            get_properties_by_status,
            update_property_status,
            set_property_code,
            delete_property,
            get_cities,
            search_cities,
            scan_and_import_properties,
            debug_database_dates,
            reset_database_with_proper_dates,
            list_original_images,
            open_images_in_folder,
            get_image_as_base64,
            list_thumbnails,
            get_thumbnail_as_base64,
            list_internet_images,
            get_internet_image_as_base64,
            copy_images_to_internet,
            clear_internet_folder,
            open_image_in_editor,
            rename_internet_images,
            list_aggelia_images,
            get_aggelia_image_as_base64,
            copy_images_to_aggelia,
            clear_aggelia_folder,
            open_image_in_advanced_editor,
            copy_and_watermark_images,
            list_watermark_images,
            list_watermark_aggelia_images,
            get_watermark_image_as_base64,
            clear_watermark_folders,
            fill_aggelia_to_25,
            open_property_folder,
            get_full_property_path,
            generate_watermark_preview,
            // Perspective correction commands (LSD + RANSAC approach)
            process_images_for_perspective,
            accept_perspective_corrections,
            cleanup_perspective_temp,
            get_original_image_for_comparison,
            // OpenCV setup commands
            check_opencv_status,
            run_opencv_setup,
            skip_opencv_setup,
            was_opencv_setup_skipped,
            reset_opencv_setup_skip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Add the main function that was missing
fn main() {
    run()
}
