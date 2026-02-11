//! Built-in image editor module for cropping, rotation, and adjustments.
//!
//! Provides Tauri commands for non-destructive image editing with real-time preview.
//! Uses image caching and resize-first pipeline for <30ms preview response times.

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::gpu::ImageProcessor;

// ============================================================================
// Image Cache for Real-Time Preview
// ============================================================================

/// Cached image data for fast preview generation.
/// Stores both the full-resolution image (for saving without re-decode)
/// and a pre-resized preview (for fast edits).
pub struct ImageCache {
    pub path: String,
    pub full_image: DynamicImage,    // Full-resolution cached image (~50-100MB, saves disk I/O on save)
    pub preview_image: DynamicImage, // Pre-resized to ~800px for fast processing
    pub preview_size: u32,
}

/// Type alias for the cached image state
pub type ImageCacheState = Mutex<Option<ImageCache>>;

/// Result from loading an image into the cache
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorLoadResult {
    pub width: u32,
    pub height: u32,
    pub preview_base64: String,
}

/// Parameters for image editing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditParams {
    // Crop (normalized coordinates 0-1)
    pub crop_enabled: bool,
    pub crop_x: f32,
    pub crop_y: f32,
    pub crop_width: f32,
    pub crop_height: f32,

    // Rotation
    pub fine_rotation: f32, // -45 to +45 degrees
    pub quarter_turns: u8,  // 0, 1, 2, or 3 (0°, 90°, 180°, 270°)

    // Adjustments (-100 to 100, default 0)
    pub brightness: i32,
    pub exposure: i32,
    pub contrast: i32,
    pub highlights: i32,
    pub shadows: i32,
}

impl Default for EditParams {
    fn default() -> Self {
        Self {
            crop_enabled: false,
            crop_x: 0.0,
            crop_y: 0.0,
            crop_width: 1.0,
            crop_height: 1.0,
            fine_rotation: 0.0,
            quarter_turns: 0,
            brightness: 0,
            exposure: 0,
            contrast: 0,
            highlights: 0,
            shadows: 0,
        }
    }
}

/// Result returned by editor commands
#[derive(Debug, Serialize)]
pub struct EditorCommandResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Auto-adjustment suggestions based on image analysis
#[derive(Debug, Serialize)]
pub struct AutoAdjustments {
    pub brightness: i32,
    pub exposure: i32,
    pub contrast: i32,
    pub highlights: i32,
    pub shadows: i32,
}

/// Auto-straighten result
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoStraightenResult {
    pub angle: f32,      // Suggested rotation angle in degrees
    pub confidence: f32, // Confidence level 0-1
    pub lines_used: usize,
    pub vh_agreement: bool,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the dimensions of an image
#[tauri::command]
pub async fn editor_get_dimensions(image_path: String) -> Result<(u32, u32), String> {
    let img = crate::turbo::load_image(&image_path).map_err(|e| format!("Failed to open image: {e}"))?;
    Ok(img.dimensions())
}

/// Load an image into the cache for fast preview generation.
/// This should be called once when opening the editor.
/// Returns the original dimensions and initial preview.
#[tauri::command]
pub async fn editor_load_image(
    app: AppHandle,
    image_path: String,
    preview_size: u32,
) -> Result<EditorLoadResult, String> {
    let path_clone = image_path.clone();

    // Heavy I/O + decode + resize runs on a blocking thread so we don't
    // stall the Tauri async runtime (which would freeze the UI).
    let (img, preview_img, preview_base64) = tokio::task::spawn_blocking(move || {
        // Load the original image from disk (turbojpeg for JPEG files)
        let img = crate::turbo::load_image(&path_clone)
            .map_err(|e| format!("Failed to open image: {e}"))?;

        // Create pre-resized preview version for fast processing
        let preview_img = resize_for_preview(&img, preview_size);

        // Generate initial preview (no edits applied)
        let preview_base64 = encode_to_base64_jpeg(&preview_img)?;

        Ok::<_, String>((img, preview_img, preview_base64))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    let (width, height) = img.dimensions();

    // Store in cache (including full-resolution image to avoid re-decode on save)
    let cache = app.state::<ImageCacheState>();
    let mut guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    *guard = Some(ImageCache {
        path: image_path,
        full_image: img,
        preview_image: preview_img,
        preview_size,
    });

    Ok(EditorLoadResult {
        width,
        height,
        preview_base64,
    })
}

/// Generate a preview of the edited image using the cached preview image.
/// This is optimized for speed - processes the small preview image, not full resolution.
#[tauri::command]
pub async fn editor_generate_preview(
    app: AppHandle,
    params: EditParams,
) -> Result<String, String> {
    // Clone the preview image out of the lock quickly so we don't hold
    // the mutex during GPU work (which would block load/save).
    let preview_img = {
        let cache = app.state::<ImageCacheState>();
        let guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
        let cached = guard.as_ref().ok_or("No image loaded. Call editor_load_image first.")?;
        cached.preview_image.clone()
    };

    // Get GPU processor
    let processor = app.state::<Arc<ImageProcessor>>();

    // Apply edits to the pre-resized preview image (fast!)
    let edited = apply_all_edits_gpu(&preview_img, &params, &processor)?;

    // Encode to base64 JPEG
    encode_to_base64_jpeg(&edited)
}

/// Legacy preview generation that loads from disk (slower, but works without cache)
#[allow(dead_code)]
pub async fn editor_generate_preview_legacy(
    image_path: String,
    params: EditParams,
    preview_size: u32,
) -> Result<String, String> {
    // Load the original image
    let img = crate::turbo::load_image(&image_path).map_err(|e| format!("Failed to open image: {e}"))?;

    // Apply all edits
    let edited = apply_all_edits(&img, &params)?;

    // Resize for preview
    let preview = resize_for_preview(&edited, preview_size);

    // Encode to base64 JPEG
    encode_to_base64_jpeg(&preview)
}

/// Save the edited image (replaces original).
/// Uses the cached full-resolution image when available to avoid expensive re-decode.
#[tauri::command]
pub async fn editor_save_image(
    app: AppHandle,
    image_path: String,
    params: EditParams,
) -> Result<EditorCommandResult, String> {
    // Take the full-res image from cache (avoids clone of ~80MB) or load from disk.
    // We take ownership so we don't hold the lock during the expensive GPU + save work.
    let img = {
        let cache = app.state::<ImageCacheState>();
        let mut guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
        if let Some(cached) = guard.as_mut() {
            if cached.path == image_path {
                // Take the full image out of the cache (replace with a 1x1 placeholder).
                // This avoids cloning ~80MB. The cache will be repopulated on next load.
                std::mem::replace(
                    &mut cached.full_image,
                    DynamicImage::new_rgba8(1, 1),
                )
            } else {
                crate::turbo::load_image(&image_path)
                    .map_err(|e| format!("Failed to open image: {e}"))?
            }
        } else {
            crate::turbo::load_image(&image_path)
                .map_err(|e| format!("Failed to open image: {e}"))?
        }
    };

    // Get GPU processor
    let processor = app.state::<Arc<ImageProcessor>>();
    let processor_ref = processor.inner().clone();

    let path_clone = image_path.clone();

    // Run GPU processing + save on a blocking thread
    tokio::task::spawn_blocking(move || {
        // Apply all edits at full resolution using GPU acceleration
        let edited = apply_all_edits_gpu(&img, &params, &processor_ref)?;
        drop(img); // Free source image (~80MB) before encoding

        // Determine output format from original file extension
        let path = Path::new(&path_clone);
        let format = get_image_format(path)?;

        // Save the edited image, replacing the original
        save_image(&edited, path, format)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))??;

    // Invalidate the cache since the image on disk has changed
    {
        let cache = app.state::<ImageCacheState>();
        let mut guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
        *guard = None;
    }

    Ok(EditorCommandResult {
        success: true,
        error: None,
    })
}

/// Analyze the cached image and suggest auto-adjustments based on histogram analysis.
/// Uses percentile-based approach to determine optimal brightness, exposure, and contrast.
#[tauri::command]
pub async fn editor_analyze_image(app: AppHandle) -> Result<AutoAdjustments, String> {
    // Get the cached preview image
    let cache = app.state::<ImageCacheState>();
    let guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    let cached = guard
        .as_ref()
        .ok_or("No image loaded. Call editor_load_image first.")?;

    // Analyze the preview image (fast since it's pre-resized)
    let adjustments = analyze_image_histogram(&cached.preview_image);

    Ok(adjustments)
}

/// Analyze image histogram and calculate optimal adjustments
pub fn analyze_image_histogram(img: &DynamicImage) -> AutoAdjustments {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Sample pixels (every 4th pixel for speed on larger previews)
    let step = 4;
    let mut luminances: Vec<u8> = Vec::with_capacity((width * height / (step * step)) as usize);

    for y in (0..height).step_by(step as usize) {
        for x in (0..width).step_by(step as usize) {
            let pixel = rgba.get_pixel(x, y);
            // Calculate luminance: 0.299*R + 0.587*G + 0.114*B
            let luminance = (0.299 * f32::from(pixel[0])
                + 0.587 * f32::from(pixel[1])
                + 0.114 * f32::from(pixel[2])) as u8;
            luminances.push(luminance);
        }
    }

    if luminances.is_empty() {
        return AutoAdjustments {
            brightness: 0,
            exposure: 0,
            contrast: 0,
            highlights: 0,
            shadows: 0,
        };
    }

    // Sort for percentile calculation
    luminances.sort_unstable();

    let len = luminances.len();
    let p5_idx = len * 5 / 100;
    let p95_idx = (len * 95 / 100).min(len - 1);
    let p5 = luminances[p5_idx] as f32;
    let p95 = luminances[p95_idx] as f32;

    // Calculate mean brightness
    let sum: u64 = luminances.iter().map(|&v| u64::from(v)).sum();
    let mean = sum as f32 / len as f32;

    // Calculate dynamic range
    let dynamic_range = p95 - p5;

    // Target values for web-optimized real estate photos
    // Slight brightness boost without washing out
    let target_mean: f32 = 135.0; // Moderately above neutral (128)

    // Brightness adjustment: gentle correction with small base boost
    let base_brightness_boost: f32 = 5.0;
    let brightness_adj =
        (base_brightness_boost + (target_mean - mean) * 0.2).clamp(-20.0, 30.0) as i32;

    // Exposure adjustment: only for notably dark images
    let exposure_adj = if mean < 100.0 {
        // Notably dark image - increase exposure
        ((100.0 - mean) * 0.2).clamp(0.0, 20.0) as i32
    } else if p5 > 60.0 {
        // Already very bright/washed out - decrease
        ((60.0 - p5) * 0.25).clamp(-15.0, 0.0) as i32
    } else {
        0
    };

    // Contrast adjustment: slight reduction for softer web look
    let contrast_adj = if dynamic_range > 200.0 {
        // High contrast - reduce
        ((200.0 - dynamic_range) * 0.12).clamp(-25.0, 0.0) as i32
    } else if dynamic_range < 80.0 {
        // Very flat image - slight boost
        ((80.0 - dynamic_range) * 0.1).clamp(0.0, 10.0) as i32
    } else {
        // Normal range - minimal adjustment
        -5
    };

    // Highlights adjustment: recover blown highlights
    let highlights_adj = if p95 > 245.0 {
        // Highlights are blown - reduce them
        ((240.0 - p95) * 0.6).clamp(-25.0, 0.0) as i32
    } else if p95 < 180.0 {
        // Image is quite dark - slight highlight boost
        ((180.0 - p95) * 0.15).clamp(0.0, 10.0) as i32
    } else {
        0
    };

    // Shadows adjustment: lift only crushed shadows
    let shadows_adj = if p5 < 10.0 {
        // Shadows are crushed - lift them
        ((20.0 - p5) * 0.5).clamp(0.0, 25.0) as i32
    } else if p5 > 60.0 {
        // Image is washed out - add shadow depth
        ((60.0 - p5) * 0.3).clamp(-20.0, 0.0) as i32
    } else {
        0
    };

    AutoAdjustments {
        brightness: brightness_adj,
        exposure: exposure_adj,
        contrast: contrast_adj,
        highlights: highlights_adj,
        shadows: shadows_adj,
    }
}

/// Analyze the cached image and detect the optimal straightening angle.
/// Uses LSD + RANSAC + Vanishing Point validation for robust detection.
#[tauri::command]
pub async fn editor_auto_straighten(app: AppHandle) -> Result<AutoStraightenResult, String> {
    use crate::perspective::straighten::analyze_straighten;
    use std::path::Path;

    // Get the cached preview image
    let cache = app.state::<ImageCacheState>();
    let guard = cache.lock().map_err(|e| format!("Failed to lock cache: {e}"))?;
    let cached = guard
        .as_ref()
        .ok_or("No image loaded. Call editor_load_image first.")?;

    // Get GPU processor for gradient histogram computation
    let processor = app.state::<Arc<ImageProcessor>>();

    // Use GPU gradient histogram + RANSAC + VP straightening algorithm
    // Pass the original path for EXIF focal length extraction
    let image_path = Path::new(&cached.path);
    let (pw, ph) = cached.preview_image.dimensions();
    eprintln!("[auto-straighten] preview_image: {pw}x{ph}, path: {}", cached.path);

    let result = analyze_straighten(&cached.preview_image, Some(image_path), &processor);

    eprintln!(
        "[auto-straighten] result: rotation={:.3}°, confidence={:.3}, lines={}, vh={}",
        result.suggested_rotation, result.confidence, result.lines_used, result.vh_agreement
    );

    Ok(AutoStraightenResult {
        angle: result.suggested_rotation as f32,
        confidence: result.confidence,
        lines_used: result.lines_used,
        vh_agreement: result.vh_agreement,
    })
}

// ============================================================================
// Batch Auto-Enhance Commands
// ============================================================================

use crate::perspective::{
    AdjustmentAnalysis, EnhanceAnalysisResult, EnhanceApplyResult, EnhanceRequest,
    StraightenAnalysis,
};

/// Progress event payload emitted during batch analysis and apply.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnhanceProgressEvent {
    phase: String,      // "analyze" or "apply"
    current: usize,     // 1-based index of current image
    total: usize,       // total images to process
    filename: String,   // name of the image just completed
}

/// Simple counting semaphore to limit concurrent operations.
/// Uses `Mutex<usize>` + `Condvar` to block threads when the limit is reached.
struct CountingSemaphore {
    state: Mutex<usize>,
    condvar: Condvar,
    max_permits: usize,
}

impl CountingSemaphore {
    fn new(max_permits: usize) -> Self {
        Self {
            state: Mutex::new(0),
            condvar: Condvar::new(),
            max_permits,
        }
    }

    /// Acquire a permit, blocking until one is available.
    fn acquire(&self) -> SemaphoreGuard<'_> {
        let mut active = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while *active >= self.max_permits {
            active = self.condvar.wait(active).unwrap_or_else(|e| e.into_inner());
        }
        *active += 1;
        SemaphoreGuard { semaphore: self }
    }
}

struct SemaphoreGuard<'a> {
    semaphore: &'a CountingSemaphore,
}

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        let mut active = self
            .semaphore
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *active -= 1;
        self.semaphore.condvar.notify_one();
    }
}

/// Build a rayon thread pool optimized for background batch processing.
/// Uses half the available CPU cores to keep the system responsive.
///
/// Note: We'd ideally set thread priority to below-normal via Windows API,
/// but `unsafe_code = "forbid"` in Cargo.toml prevents this. The reduced
/// thread count (cores/2) and the load semaphore are sufficient to keep
/// the system responsive during batch processing.
fn build_background_pool() -> Result<rayon::ThreadPool, String> {
    let available = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(4);
    // Use half the cores, minimum 2, to leave headroom for UI + OS
    let num_threads = (available / 2).max(2);

    eprintln!(
        "[batch-enhance] Building thread pool: {num_threads} threads (of {available} available)"
    );

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .thread_name(|idx| format!("enhance-worker-{idx}"))
        .build()
        .map_err(|e| format!("Failed to create thread pool: {e}"))
}

/// Analyze all images in a property's INTERNET folder for batch enhancement.
/// Returns analysis results with before/after previews for each image.
#[tauri::command]
pub async fn batch_analyze_for_enhance(
    app: AppHandle,
    folder_path: String,
    status: String,
) -> Result<Vec<EnhanceAnalysisResult>, String> {
    use crate::perspective::straighten::analyze_straighten;

    // Build the INTERNET folder path - load config async
    let config = crate::config::get_cached_config(&app)
        .await?
        .ok_or("App configuration not found. Please configure in Settings.")?;

    // Get the correct base folder path based on status
    let base_folder_path = match status.as_str() {
        "NEW" => &config.new_folder_path,
        "DONE" => &config.done_folder_path,
        "NOT_FOUND" => &config.not_found_folder_path,
        "ARCHIVE" => &config.archive_folder_path,
        _ => return Err(format!("Unknown status: {status}")),
    };

    if base_folder_path.is_empty() {
        return Err(format!(
            "Folder path for status '{status}' is not configured. Please set it in Settings."
        ));
    }

    let internet_path = std::path::Path::new(base_folder_path)
        .join(&folder_path)
        .join("INTERNET");

    if !internet_path.exists() {
        return Err(format!(
            "INTERNET folder not found: {}",
            internet_path.display()
        ));
    }

    // List all image files
    let image_extensions = ["jpg", "jpeg", "png", "webp", "bmp", "gif"];
    let mut image_paths: Vec<std::path::PathBuf> = std::fs::read_dir(&internet_path)
        .map_err(|e| format!("Failed to read INTERNET folder: {e}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| image_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename for consistent ordering
    image_paths.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });

    // Get GPU processor for accelerated image editing
    let processor = app.state::<Arc<ImageProcessor>>();
    let processor_ref = processor.inner().clone();

    // ========================================================================
    // Throttled parallel analysis pipeline:
    //
    // Uses a custom rayon thread pool with half the CPU cores to keep the
    // system responsive. A counting semaphore limits concurrent full-res
    // image decodes to 2, capping peak RAM usage at ~200MB regardless of
    // thread count. Images are processed in chunks of 6 with brief yields
    // between chunks to let the GPU queue drain and the OS breathe.
    //
    // Progress events are emitted to the frontend after each image completes,
    // enabling a real progress bar instead of an indeterminate spinner.
    // ========================================================================

    let total_count = image_paths.len();
    eprintln!(
        "[batch-analyze] Starting throttled analysis of {total_count} images"
    );
    let total_start = std::time::Instant::now();

    // Build a dedicated thread pool with reduced core count
    let pool = build_background_pool()?;

    // Limit concurrent full-res image loads to 2 to cap RAM spikes.
    // Each full-res image is ~80-100MB RGBA; with 2 permits, peak decode
    // RAM is ~200MB instead of potentially 800MB+ with unlimited threads.
    let load_semaphore = Arc::new(CountingSemaphore::new(2));

    // Atomic counter for progress events
    let completed_count = Arc::new(AtomicUsize::new(0));

    // Clone app handle for progress events from within rayon threads
    let app_for_progress = app.clone();

    // Process images in chunks to allow periodic yielding
    let chunk_size: usize = 6;
    let mut final_results: Vec<EnhanceAnalysisResult> = Vec::with_capacity(total_count);

    for chunk in image_paths.chunks(chunk_size) {
        let sem = Arc::clone(&load_semaphore);
        let counter = Arc::clone(&completed_count);
        let proc = processor_ref.clone();
        let app_ref = app_for_progress.clone();

        let chunk_results: Vec<Result<EnhanceAnalysisResult, String>> = pool.install(|| {
            chunk
                .par_iter()
                .map(|path| {
                    let img_start = std::time::Instant::now();

                    // Acquire semaphore permit before loading full-res image.
                    // This blocks the thread until a permit is available, limiting
                    // the number of concurrent ~80MB allocations.
                    let preview_img = {
                        let _permit = sem.acquire();
                        let img = crate::turbo::load_image(path)
                            .map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
                        let preview = resize_for_preview(&img, 600);
                        drop(img); // Free ~80MB before releasing permit
                        preview
                        // _permit drops here, releasing the semaphore
                    };

                    // GPU-accelerated straighten analysis on small preview
                    let straighten_result =
                        analyze_straighten(&preview_img, Some(path), &proc);
                    let adjustments = analyze_image_histogram(&preview_img);

                    // Calculate adjustment magnitude (normalized 0-1)
                    let adj_magnitude = ((adjustments.brightness.abs() as f32 / 100.0).powi(2)
                        + (adjustments.exposure.abs() as f32 / 100.0).powi(2)
                        + (adjustments.contrast.abs() as f32 / 100.0).powi(2)
                        + (adjustments.highlights.abs() as f32 / 100.0).powi(2)
                        + (adjustments.shadows.abs() as f32 / 100.0).powi(2))
                    .sqrt()
                        / 2.24;

                    let needs_straighten = straighten_result.suggested_rotation.abs() > 0.3
                        && straighten_result.confidence > 0.4;
                    let needs_adjustment = adjustments.brightness.abs() > 10
                        || adjustments.exposure.abs() > 10
                        || adjustments.contrast.abs() > 10
                        || adjustments.highlights.abs() > 10
                        || adjustments.shadows.abs() > 10;
                    let needs_enhancement = needs_straighten || needs_adjustment;

                    let rotation_weight = if straighten_result.suggested_rotation.abs() > 0.5 {
                        0.6
                    } else {
                        0.3
                    };
                    let combined_confidence = straighten_result.confidence * rotation_weight
                        + adj_magnitude.min(1.0) * (1.0 - rotation_weight);

                    // GPU-accelerated preview generation
                    let preview_params = EditParams {
                        fine_rotation: straighten_result.suggested_rotation as f32,
                        brightness: adjustments.brightness,
                        exposure: adjustments.exposure,
                        contrast: adjustments.contrast,
                        highlights: adjustments.highlights,
                        shadows: adjustments.shadows,
                        ..EditParams::default()
                    };

                    let enhanced_preview =
                        apply_all_edits_gpu(&preview_img, &preview_params, &proc)
                            .map_err(|e| format!("Failed to generate preview: {e}"))?;

                    // Encode both previews to base64
                    let preview_base64 = encode_to_base64_jpeg(&enhanced_preview)?;
                    let original_preview_base64 = encode_to_base64_jpeg(&preview_img)?;

                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    // Emit progress event
                    let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                    eprintln!(
                        "[batch-analyze] {filename} completed in {:?} ({done}/{total_count})",
                        img_start.elapsed()
                    );
                    let _ = app_ref.emit(
                        "enhance-progress",
                        EnhanceProgressEvent {
                            phase: "analyze".to_string(),
                            current: done,
                            total: total_count,
                            filename: filename.clone(),
                        },
                    );

                    Ok(EnhanceAnalysisResult {
                        filename,
                        original_path: path.to_string_lossy().to_string(),
                        straighten: StraightenAnalysis {
                            rotation: straighten_result.suggested_rotation,
                            confidence: straighten_result.confidence,
                            lines_used: straighten_result.lines_used,
                            vh_agreement: straighten_result.vh_agreement,
                        },
                        adjustments: AdjustmentAnalysis {
                            brightness: adjustments.brightness,
                            exposure: adjustments.exposure,
                            contrast: adjustments.contrast,
                            highlights: adjustments.highlights,
                            shadows: adjustments.shadows,
                            magnitude: adj_magnitude,
                        },
                        combined_confidence,
                        needs_enhancement,
                        preview_base64,
                        original_preview_base64,
                    })
                })
                .collect()
        });

        // Collect chunk results, filtering out errors
        for result in chunk_results {
            match result {
                Ok(r) => final_results.push(r),
                Err(e) => eprintln!("Warning: Failed to analyze image: {e}"),
            }
        }

        // Brief yield between chunks to let GPU queue drain and OS breathe.
        // Only yield if there are more chunks to process.
        if final_results.len() < total_count {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    eprintln!(
        "[batch-analyze] Completed {} images in {:?}",
        final_results.len(),
        total_start.elapsed()
    );

    Ok(final_results)
}

/// Apply enhancements to selected images, overwriting the originals.
#[tauri::command]
pub async fn batch_apply_enhancements(
    app: AppHandle,
    enhancements: Vec<EnhanceRequest>,
) -> Result<Vec<EnhanceApplyResult>, String> {
    // Get GPU processor for accelerated image editing
    let processor = app.state::<Arc<ImageProcessor>>();

    // Process images SEQUENTIALLY to minimize RAM usage.
    // Each full-res image is ~80MB RGBA; only one is in memory at a time.
    // GPU operations serialize on a single device anyway, so parallel loading
    // would only waste RAM without improving throughput.
    let mut results: Vec<EnhanceApplyResult> = Vec::with_capacity(enhancements.len());
    let total = enhancements.len();

    for (i, request) in enhancements.iter().enumerate() {
        eprintln!(
            "[batch-apply] Processing image {}/{total}: {}",
            i + 1,
            request.filename
        );

        // Emit progress event for apply phase
        let _ = app.emit(
            "enhance-progress",
            EnhanceProgressEvent {
                phase: "apply".to_string(),
                current: i + 1,
                total,
                filename: request.filename.clone(),
            },
        );

        let result = (|| -> Result<(), String> {
            // Load the original image at full resolution using turbojpeg
            let img = crate::turbo::load_image(&request.original_path)
                .map_err(|e| format!("Failed to open image: {e}"))?;

            // Build EditParams
            let params = EditParams {
                fine_rotation: request.rotation as f32,
                brightness: request.brightness,
                exposure: request.exposure,
                contrast: request.contrast,
                highlights: request.highlights,
                shadows: request.shadows,
                ..EditParams::default()
            };

            // Apply all edits at full resolution (GPU-accelerated)
            let edited = apply_all_edits_gpu(&img, &params, &processor)?;

            // Drop the source image before saving to free ~80MB
            drop(img);

            // Determine output format from original file extension
            let path = Path::new(&request.original_path);
            let format = get_image_format(path)?;

            // Save over the original
            save_image(&edited, path, format)?;

            Ok(())
        })();

        results.push(EnhanceApplyResult {
            filename: request.filename.clone(),
            success: result.is_ok(),
            error: result.err(),
        });
    }

    Ok(results)
}

// ============================================================================
// Image Processing Pipeline
// ============================================================================

/// Apply all edits using GPU acceleration with CPU fallback.
/// This is the primary entry point for all image editing operations.
///
/// Optimized pipeline:
/// - Uses fused rotate+adjust when both are needed (single GPU upload/download)
/// - Avoids unnecessary clones when no quarter rotation or crop is needed
/// - Quarter rotation and crop are fast CPU ops (lossless transpose / sub-image view)
pub fn apply_all_edits_gpu(
    img: &DynamicImage,
    params: &EditParams,
    processor: &ImageProcessor,
) -> Result<DynamicImage, String> {
    let needs_quarter = params.quarter_turns % 4 != 0;
    let needs_fine_rotation = params.fine_rotation.abs() > 0.01;
    let needs_crop = params.crop_enabled;
    let needs_adjust = params.brightness != 0
        || params.exposure != 0
        || params.contrast != 0
        || params.highlights != 0
        || params.shadows != 0;

    // Fast path: nothing to do
    if !needs_quarter && !needs_fine_rotation && !needs_crop && !needs_adjust {
        return Ok(img.clone());
    }

    // 1. Apply quarter-turn rotations (0°, 90°, 180°, 270°) - lossless, CPU
    //    Uses Cow to avoid cloning ~96MB when no quarter rotation is needed.
    let after_quarter: std::borrow::Cow<'_, DynamicImage> = if needs_quarter {
        std::borrow::Cow::Owned(apply_quarter_rotation(img, params.quarter_turns))
    } else {
        std::borrow::Cow::Borrowed(img)
    };

    // 2. Apply crop before GPU ops (reduces pixel count for faster GPU processing)
    let after_crop: std::borrow::Cow<'_, DynamicImage> = if needs_crop {
        std::borrow::Cow::Owned(apply_crop(&after_quarter, params)?)
    } else {
        after_quarter
    };

    // 3. Fused GPU pipeline: rotation + adjustments in a single upload/download
    //    This saves ~20-30ms per image by eliminating redundant PCIe transfers.
    if needs_fine_rotation || needs_adjust {
        processor.rotate_and_adjust(
            &after_crop,
            if needs_fine_rotation { params.fine_rotation } else { 0.0 },
            params.brightness,
            params.exposure,
            params.contrast,
            params.highlights,
            params.shadows,
        )
    } else {
        Ok(after_crop.into_owned())
    }
}

/// Apply all edits in the correct order (CPU-only fallback, used when no GPU state is available)
pub fn apply_all_edits(img: &DynamicImage, params: &EditParams) -> Result<DynamicImage, String> {
    let mut result = img.clone();

    // 1. Apply quarter-turn rotations (0°, 90°, 180°, 270°) - lossless
    result = apply_quarter_rotation(&result, params.quarter_turns);

    // 2. Apply fine rotation with auto-crop (if non-zero)
    if params.fine_rotation.abs() > 0.01 {
        result = apply_fine_rotation(&result, params.fine_rotation)?;
    }

    // 3. Apply crop (if enabled)
    if params.crop_enabled {
        result = apply_crop(&result, params)?;
    }

    // 4. Apply adjustments
    result = apply_adjustments(&result, params);

    Ok(result)
}

// ============================================================================
// Rotation
// ============================================================================

/// Apply 90-degree rotation increments (lossless)
fn apply_quarter_rotation(img: &DynamicImage, turns: u8) -> DynamicImage {
    match turns % 4 {
        0 => img.clone(),
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => unreachable!(),
    }
}

/// Apply fine rotation (arbitrary angle) with mathematical auto-crop.
/// Delegates to the rayon-parallelized CPU implementation in gpu.rs.
fn apply_fine_rotation(img: &DynamicImage, angle_degrees: f32) -> Result<DynamicImage, String> {
    crate::gpu::cpu_fine_rotation(img, angle_degrees)
}

// ============================================================================
// Cropping
// ============================================================================

/// Apply crop using normalized coordinates (0-1)
fn apply_crop(img: &DynamicImage, params: &EditParams) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();

    // Convert normalized coordinates to pixel coordinates
    let x = (params.crop_x * width as f32) as u32;
    let y = (params.crop_y * height as f32) as u32;
    let crop_width = (params.crop_width * width as f32) as u32;
    let crop_height = (params.crop_height * height as f32) as u32;

    // Ensure we don't exceed image bounds
    let x = x.min(width.saturating_sub(1));
    let y = y.min(height.saturating_sub(1));
    let crop_width = crop_width.min(width - x).max(1);
    let crop_height = crop_height.min(height - y).max(1);

    Ok(img.crop_imm(x, y, crop_width, crop_height))
}

// ============================================================================
// Adjustments
// ============================================================================

/// GLSL-style smoothstep function for smooth interpolation
#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Apply all image adjustments
fn apply_adjustments(img: &DynamicImage, params: &EditParams) -> DynamicImage {
    // Skip if all adjustments are zero
    if params.brightness == 0
        && params.exposure == 0
        && params.contrast == 0
        && params.highlights == 0
        && params.shadows == 0
    {
        return img.clone();
    }

    let mut rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Pre-compute adjustment factors (calibrated to match Windows 11 Photo Editor)
    // These values match the WebGL shader exactly
    let brightness_factor = params.brightness as f32 / 350.0;      // -0.29 to 0.29 (softer)
    let exposure_factor = 2.0_f32.powf(params.exposure as f32 / 130.0); // -0.77 to 0.77 f-stops
    let contrast_factor = (params.contrast as f32 + 170.0) / 170.0; // 0.41 to 1.59 (softer)
    let highlight_adjust = params.highlights as f32 / 180.0;       // -0.56 to 0.56 (softer)
    let shadow_adjust = params.shadows as f32 / 180.0;             // -0.56 to 0.56 (softer)

    for y in 0..height {
        for x in 0..width {
            let pixel = rgba.get_pixel_mut(x, y);

            // Convert to 0-1 range
            let mut r = pixel[0] as f32 / 255.0;
            let mut g = pixel[1] as f32 / 255.0;
            let mut b = pixel[2] as f32 / 255.0;

            // 1. Exposure: multiplicative (simulates f-stops) - apply first for most natural results
            r *= exposure_factor;
            g *= exposure_factor;
            b *= exposure_factor;

            // 2. Brightness: additive
            r += brightness_factor;
            g += brightness_factor;
            b += brightness_factor;

            // 3. Contrast: pivot around 0.5
            r = (r - 0.5) * contrast_factor + 0.5;
            g = (g - 0.5) * contrast_factor + 0.5;
            b = (b - 0.5) * contrast_factor + 0.5;

            // 4. Highlights/Shadows: luminance-based masking with smoothstep transition
            if highlight_adjust != 0.0 || shadow_adjust != 0.0 {
                let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                // Smooth mask transition for more natural look (matches WebGL shader)
                let highlight_mask = smoothstep(0.3, 0.7, luminance);
                let shadow_mask = 1.0 - highlight_mask;

                let adjustment = highlight_adjust * highlight_mask + shadow_adjust * shadow_mask;
                // Scale down for subtler effect (matches WebGL shader)
                r += adjustment * 0.5;
                g += adjustment * 0.5;
                b += adjustment * 0.5;
            }

            // Clamp and convert back to u8
            pixel[0] = (r.clamp(0.0, 1.0) * 255.0) as u8;
            pixel[1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
            pixel[2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
            // Alpha channel remains unchanged
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Adjustment factors pre-computed for parallel processing
#[allow(dead_code)]
struct AdjustmentFactors {
    brightness: f32,
    exposure: f32,
    contrast: f32,
    highlights: f32,
    shadows: f32,
}

impl AdjustmentFactors {
    fn from_params(params: &EditParams) -> Self {
        // Ranges calibrated to match Windows 11 Photo Editor behavior
        // These values match the WebGL shader exactly
        Self {
            brightness: params.brightness as f32 / 350.0,      // -0.29 to 0.29 (softer)
            exposure: 2.0_f32.powf(params.exposure as f32 / 130.0), // -0.77 to 0.77 f-stops (~0.59x to 1.7x)
            contrast: (params.contrast as f32 + 170.0) / 170.0, // 0.41 to 1.59 (softer)
            highlights: params.highlights as f32 / 180.0,      // -0.56 to 0.56 (softer)
            shadows: params.shadows as f32 / 180.0,            // -0.56 to 0.56 (softer)
        }
    }

    fn is_identity(&self, params: &EditParams) -> bool {
        params.brightness == 0
            && params.exposure == 0
            && params.contrast == 0
            && params.highlights == 0
            && params.shadows == 0
    }
}

/// Process a single pixel with the given adjustment factors
#[inline]
#[allow(dead_code)]
fn process_pixel(pixel: Rgba<u8>, factors: &AdjustmentFactors) -> Rgba<u8> {
    // Convert to 0-1 range
    let mut r = pixel[0] as f32 / 255.0;
    let mut g = pixel[1] as f32 / 255.0;
    let mut b = pixel[2] as f32 / 255.0;

    // 1. Exposure: multiplicative (simulates f-stops) - apply first for most natural results
    r *= factors.exposure;
    g *= factors.exposure;
    b *= factors.exposure;

    // 2. Brightness: additive
    r += factors.brightness;
    g += factors.brightness;
    b += factors.brightness;

    // 3. Contrast: pivot around 0.5
    r = (r - 0.5) * factors.contrast + 0.5;
    g = (g - 0.5) * factors.contrast + 0.5;
    b = (b - 0.5) * factors.contrast + 0.5;

    // 4. Highlights/Shadows: luminance-based masking with smoothstep transition
    if factors.highlights != 0.0 || factors.shadows != 0.0 {
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        // Smooth mask transition for more natural look (matches WebGL shader)
        let highlight_mask = smoothstep(0.3, 0.7, luminance);
        let shadow_mask = 1.0 - highlight_mask;

        let adjustment = factors.highlights * highlight_mask + factors.shadows * shadow_mask;
        // Scale down for subtler effect (matches WebGL shader)
        r += adjustment * 0.5;
        g += adjustment * 0.5;
        b += adjustment * 0.5;
    }

    // Clamp and convert back to u8
    Rgba([
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
        pixel[3], // Alpha unchanged
    ])
}

/// Apply all image adjustments using parallel processing (rayon)
#[allow(dead_code)]
fn apply_adjustments_parallel(img: &DynamicImage, params: &EditParams) -> DynamicImage {
    let factors = AdjustmentFactors::from_params(params);

    // Skip if all adjustments are zero
    if factors.is_identity(params) {
        return img.clone();
    }

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Process all pixels in parallel using rayon
    let pixels: Vec<Rgba<u8>> = rgba
        .pixels()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|p| process_pixel(**p, &factors))
        .collect();

    // Reconstruct the image
    let mut result = RgbaImage::new(width, height);
    for (i, pixel) in pixels.into_iter().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        result.put_pixel(x, y, pixel);
    }

    DynamicImage::ImageRgba8(result)
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Resize image for preview while maintaining aspect ratio.
/// Uses SIMD-accelerated resize for 14-23x faster performance.
fn resize_for_preview(img: &DynamicImage, max_size: u32) -> DynamicImage {
    crate::fast_resize::resize_to_fit(img, max_size)
}

/// Encode image to base64 JPEG using turbojpeg for faster encoding
fn encode_to_base64_jpeg(img: &DynamicImage) -> Result<String, String> {
    crate::turbo::encode_jpeg_base64(&img.to_rgb8(), 85)
}

/// Get the image format from file extension
fn get_image_format(path: &Path) -> Result<ImageFormat, String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .ok_or("Could not determine file extension")?;

    match extension.as_str() {
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        "gif" => Ok(ImageFormat::Gif),
        "webp" => Ok(ImageFormat::WebP),
        "bmp" => Ok(ImageFormat::Bmp),
        _ => Ok(ImageFormat::Jpeg), // Default to JPEG
    }
}

/// Save image to disk (uses turbojpeg for JPEG files)
fn save_image(img: &DynamicImage, path: &Path, format: ImageFormat) -> Result<(), String> {
    if format == ImageFormat::Jpeg {
        // Use turbojpeg for faster JPEG encoding
        crate::turbo::save_jpeg(&img.to_rgb8(), path, 92)
    } else {
        img.save_with_format(path, format)
            .map_err(|e| format!("Failed to save image: {e}"))?;
        Ok(())
    }
}
