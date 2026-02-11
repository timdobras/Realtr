//! GPU-accelerated image processing using wgpu compute shaders.
//!
//! Provides transparent GPU acceleration for:
//! - Fine rotation with bilinear interpolation and auto-crop
//! - Color adjustments (brightness, exposure, contrast, highlights, shadows)
//! - Watermark alpha blending
//!
//! Falls back to CPU processing if no GPU is available or if GPU operations fail.

use bytemuck::{Pod, Zeroable};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::mem::size_of;
use std::sync::Arc;

// ============================================================================
// GPU Context and Pipeline Cache
// ============================================================================

/// Holds the wgpu device, queue, and pre-compiled compute pipelines.
/// Created once at app startup and shared across all Tauri commands.
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    // Pre-compiled pipelines (shader compilation is expensive, ~10-50ms each)
    pub rotation_pipeline: wgpu::ComputePipeline,
    pub adjustment_pipeline: wgpu::ComputePipeline,
    pub watermark_pipeline: wgpu::ComputePipeline,
    pub bilateral_pipeline: wgpu::ComputePipeline,
    pub clahe_histogram_pipeline: wgpu::ComputePipeline,
    pub clahe_apply_pipeline: wgpu::ComputePipeline,
    pub undistort_pipeline: wgpu::ComputePipeline,
    pub gradient_histogram_pipeline: wgpu::ComputePipeline,
    // Bind group layouts (reused per dispatch)
    pub rotation_bgl: wgpu::BindGroupLayout,
    pub adjustment_bgl: wgpu::BindGroupLayout,
    pub watermark_bgl: wgpu::BindGroupLayout,
    pub bilateral_bgl: wgpu::BindGroupLayout,
    pub clahe_bgl: wgpu::BindGroupLayout,
    pub undistort_bgl: wgpu::BindGroupLayout,
    pub gradient_histogram_bgl: wgpu::BindGroupLayout,
}

/// Top-level image processor that routes to GPU or CPU.
/// All wgpu types (Device, Queue, ComputePipeline, etc.) are Send + Sync,
/// so this enum is automatically Send + Sync through its fields.
pub enum ImageProcessor {
    Gpu(Arc<GpuContext>),
    Cpu,
}

impl ImageProcessor {
    /// Try to initialize GPU; fall back to CPU if unavailable.
    pub fn new() -> Self {
        match GpuContext::try_new() {
            Some(ctx) => {
                eprintln!(
                    "[GPU] Initialized GPU image processing: {}",
                    ctx.adapter_name
                );
                Self::Gpu(Arc::new(ctx))
            }
            None => {
                eprintln!("[GPU] No GPU available, using CPU image processing");
                Self::Cpu
            }
        }
    }

    /// Check if GPU is available.
    #[allow(dead_code)]
    pub fn is_gpu(&self) -> bool {
        matches!(self, Self::Gpu(_))
    }

    /// Get a description of the processor for logging.
    pub fn description(&self) -> String {
        match self {
            Self::Gpu(ctx) => format!("GPU ({})", ctx.adapter_name),
            Self::Cpu => "CPU".to_string(),
        }
    }

    // ========================================================================
    // Public API: Fine Rotation
    // ========================================================================

    /// Apply fine rotation with bilinear interpolation and mathematical auto-crop.
    /// GPU path: single compute dispatch. CPU fallback: existing pixel loop.
    pub fn rotate_image(
        &self,
        img: &DynamicImage,
        angle_degrees: f32,
    ) -> Result<DynamicImage, String> {
        if angle_degrees.abs() < 0.01 {
            return Ok(img.clone());
        }

        match self {
            Self::Gpu(ctx) => match gpu_fine_rotation(ctx, img, angle_degrees) {
                Ok(result) => Ok(result),
                Err(e) => {
                    eprintln!("[GPU] Rotation failed, falling back to CPU: {e}");
                    cpu_fine_rotation(img, angle_degrees)
                }
            },
            Self::Cpu => cpu_fine_rotation(img, angle_degrees),
        }
    }

    // ========================================================================
    // Public API: Color Adjustments
    // ========================================================================

    /// Apply brightness, exposure, contrast, highlights, and shadows adjustments.
    /// GPU path: single compute dispatch. CPU fallback: per-pixel processing.
    pub fn adjust_image(
        &self,
        img: &DynamicImage,
        brightness: i32,
        exposure: i32,
        contrast: i32,
        highlights: i32,
        shadows: i32,
    ) -> DynamicImage {
        // Skip if all adjustments are zero
        if brightness == 0 && exposure == 0 && contrast == 0 && highlights == 0 && shadows == 0 {
            return img.clone();
        }

        match self {
            Self::Gpu(ctx) => match gpu_adjustments(
                ctx, img, brightness, exposure, contrast, highlights, shadows,
            ) {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("[GPU] Adjustments failed, falling back to CPU: {e}");
                    cpu_adjustments(img, brightness, exposure, contrast, highlights, shadows)
                }
            },
            Self::Cpu => cpu_adjustments(img, brightness, exposure, contrast, highlights, shadows),
        }
    }

    // ========================================================================
    // Public API: Fused Rotation + Adjustments (single PCIe round-trip)
    // ========================================================================

    /// Apply rotation and adjustments in a single GPU pipeline.
    /// Uploads the image once, dispatches rotation, feeds output directly
    /// into adjustments, downloads final result once.
    /// Saves ~20-30ms per full-res image by eliminating 2 redundant PCIe transfers.
    pub fn rotate_and_adjust(
        &self,
        img: &DynamicImage,
        angle_degrees: f32,
        brightness: i32,
        exposure: i32,
        contrast: i32,
        highlights: i32,
        shadows: i32,
    ) -> Result<DynamicImage, String> {
        let needs_rotation = angle_degrees.abs() > 0.01;
        let needs_adjust =
            brightness != 0 || exposure != 0 || contrast != 0 || highlights != 0 || shadows != 0;

        if !needs_rotation && !needs_adjust {
            return Ok(img.clone());
        }
        if !needs_rotation {
            return Ok(self.adjust_image(img, brightness, exposure, contrast, highlights, shadows));
        }
        if !needs_adjust {
            return self.rotate_image(img, angle_degrees);
        }

        match self {
            Self::Gpu(ctx) => match gpu_rotate_and_adjust(
                ctx,
                img,
                angle_degrees,
                brightness,
                exposure,
                contrast,
                highlights,
                shadows,
            ) {
                Ok(result) => Ok(result),
                Err(e) => {
                    eprintln!("[GPU] Fused pipeline failed, falling back to separate ops: {e}");
                    let rotated = self.rotate_image(img, angle_degrees)?;
                    Ok(self.adjust_image(
                        &rotated, brightness, exposure, contrast, highlights, shadows,
                    ))
                }
            },
            Self::Cpu => {
                let rotated = cpu_fine_rotation(img, angle_degrees)?;
                Ok(cpu_adjustments(
                    &rotated, brightness, exposure, contrast, highlights, shadows,
                ))
            }
        }
    }

    // ========================================================================
    // Public API: GPU Gradient Angle Histogram (replaces OpenCV LSD)
    // ========================================================================

    /// Compute a gradient angle histogram on a grayscale image.
    /// Returns a 3600-element vector (0.1 degree bins) with magnitude-weighted votes.
    /// Used to detect dominant line angles for auto-straightening.
    pub fn gradient_histogram(
        &self,
        gray_pixels: &[u8],
        width: u32,
        height: u32,
        magnitude_threshold: f32,
    ) -> Result<Vec<f32>, String> {
        match self {
            Self::Gpu(ctx) => {
                eprintln!("[GPU] Computing gradient histogram on {width}x{height} grayscale image");
                let start = std::time::Instant::now();
                match gpu_gradient_histogram(ctx, gray_pixels, width, height, magnitude_threshold) {
                    Ok(hist) => {
                        eprintln!(
                            "[GPU] Gradient histogram completed in {:?}",
                            start.elapsed()
                        );
                        Ok(hist)
                    }
                    Err(e) => {
                        eprintln!("[GPU] Gradient histogram failed, falling back to CPU: {e}");
                        Ok(cpu_gradient_histogram(
                            gray_pixels,
                            width,
                            height,
                            magnitude_threshold,
                        ))
                    }
                }
            }
            Self::Cpu => Ok(cpu_gradient_histogram(
                gray_pixels,
                width,
                height,
                magnitude_threshold,
            )),
        }
    }

    // ========================================================================
    // Public API: GPU Bilateral Filter
    // ========================================================================

    /// Apply bilateral filter to a grayscale image on GPU.
    /// Returns the filtered grayscale pixels.
    pub fn bilateral_filter(
        &self,
        gray_pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        match self {
            Self::Gpu(ctx) => match gpu_bilateral(ctx, gray_pixels, width, height) {
                Ok(result) => Ok(result),
                Err(e) => {
                    eprintln!("[GPU] Bilateral filter failed, falling back to CPU: {e}");
                    Ok(cpu_bilateral(gray_pixels, width, height))
                }
            },
            Self::Cpu => Ok(cpu_bilateral(gray_pixels, width, height)),
        }
    }

    // ========================================================================
    // Public API: GPU CLAHE
    // ========================================================================

    /// Apply CLAHE to a grayscale image on GPU.
    /// Returns the equalized grayscale pixels.
    pub fn clahe(&self, gray_pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
        match self {
            Self::Gpu(ctx) => match gpu_clahe(ctx, gray_pixels, width, height) {
                Ok(result) => Ok(result),
                Err(e) => {
                    eprintln!("[GPU] CLAHE failed, falling back to CPU: {e}");
                    Ok(cpu_clahe(gray_pixels, width, height))
                }
            },
            Self::Cpu => Ok(cpu_clahe(gray_pixels, width, height)),
        }
    }

    // ========================================================================
    // Public API: GPU Lens Undistortion
    // ========================================================================

    /// Apply radial lens undistortion on GPU.
    pub fn undistort(&self, img: &DynamicImage, k1: f32) -> Result<DynamicImage, String> {
        if k1.abs() < 0.0001 {
            return Ok(img.clone());
        }

        match self {
            Self::Gpu(ctx) => match gpu_undistort(ctx, img, k1) {
                Ok(result) => Ok(result),
                Err(e) => {
                    eprintln!("[GPU] Undistort failed, falling back to CPU: {e}");
                    Ok(cpu_undistort(img, k1))
                }
            },
            Self::Cpu => Ok(cpu_undistort(img, k1)),
        }
    }

    // ========================================================================
    // Public API: Watermark Blending
    // ========================================================================

    /// Blend a watermark onto a base image at the given position with opacity.
    /// GPU path: single compute dispatch. CPU fallback: per-pixel alpha blend.
    pub fn blend_watermark(
        &self,
        base_img: &mut RgbaImage,
        watermark: &RgbaImage,
        pos_x: u32,
        pos_y: u32,
        opacity: f32,
        use_alpha: bool,
    ) {
        match self {
            Self::Gpu(ctx) => {
                match gpu_watermark_blend(
                    ctx, base_img, watermark, pos_x, pos_y, opacity, use_alpha,
                ) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("[GPU] Watermark blend failed, falling back to CPU: {e}");
                        cpu_blend_watermark(base_img, watermark, pos_x, pos_y, opacity, use_alpha);
                    }
                }
            }
            Self::Cpu => {
                cpu_blend_watermark(base_img, watermark, pos_x, pos_y, opacity, use_alpha);
            }
        }
    }
}

// ============================================================================
// Shader Parameter Structs (must match WGSL struct layouts exactly)
// ============================================================================

/// Parameters for the rotation compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct RotationParams {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    angle_rad: f32,
    cos_r: f32,
    sin_r: f32,
    crop_half_width: f32,
    crop_half_height: f32,
    aspect: f32,
    _padding: [f32; 2],
}

/// Parameters for the adjustment compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct AdjustmentParams {
    width: u32,
    height: u32,
    brightness: f32,
    exposure: f32,
    contrast: f32,
    highlights: f32,
    shadows: f32,
    _padding: f32,
}

/// Parameters for the watermark compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct WatermarkParams {
    base_width: u32,
    base_height: u32,
    wm_width: u32,
    wm_height: u32,
    pos_x: u32,
    pos_y: u32,
    opacity: f32,
    use_alpha: u32, // 0 or 1 (booleans not allowed in uniform buffers)
}

/// Parameters for the bilateral filter compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct BilateralParams {
    width: u32,
    height: u32,
    radius: u32,
    sigma_color: f32,
    sigma_space: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

/// Parameters for the CLAHE compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ClaheParams {
    width: u32,
    height: u32,
    grid_w: u32,
    grid_h: u32,
    clip_limit: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

/// Parameters for the undistort compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct UndistortParams {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    cx: f32,
    cy: f32,
    k1: f32,
    max_r: f32,
}

/// Parameters for the gradient histogram compute shader.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GradientHistogramParams {
    width: u32,
    height: u32,
    threshold: f32,
    _pad: f32,
}

/// Number of bins in the gradient angle histogram (0.1 degree resolution).
pub const GRADIENT_HISTOGRAM_BINS: usize = 3600;

// ============================================================================
// GPU Context Initialization
// ============================================================================

impl GpuContext {
    /// Try to initialize a GPU context for headless compute.
    /// Returns None if no suitable GPU adapter is found.
    pub fn try_new() -> Option<Self> {
        // Create wgpu instance with all backends
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Request high-performance adapter (discrete GPU preferred)
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok()?;

        let adapter_info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", adapter_info.name, adapter_info.backend);

        eprintln!("[GPU] Found adapter: {adapter_name}");

        // Request device with default limits
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("image-processor"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .ok()?;

        // Compile shaders and create pipelines
        let rotation_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rotation-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rotation.wgsl").into()),
        });

        let adjustment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("adjustment-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/adjustments.wgsl").into()),
        });

        let watermark_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("watermark-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/watermark.wgsl").into()),
        });

        // Create bind group layouts
        let rotation_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rotation-bgl"),
            entries: &[
                bgl_entry(0, wgpu::BufferBindingType::Uniform),
                bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });

        let adjustment_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("adjustment-bgl"),
            entries: &[
                bgl_entry(0, wgpu::BufferBindingType::Uniform),
                bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });

        let watermark_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("watermark-bgl"),
            entries: &[
                bgl_entry(0, wgpu::BufferBindingType::Uniform),
                bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });

        // -- New shaders for Phase 2 --
        let bilateral_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bilateral-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bilateral.wgsl").into()),
        });

        let clahe_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("clahe-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/clahe.wgsl").into()),
        });

        let undistort_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("undistort-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/undistort.wgsl").into()),
        });

        let gradient_histogram_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient-histogram-shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/gradient_histogram.wgsl").into(),
            ),
        });

        // -- New bind group layouts --
        let bilateral_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bilateral-bgl"),
            entries: &[
                bgl_entry(0, wgpu::BufferBindingType::Uniform),
                bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });

        // CLAHE uses 4 bindings: uniform, input, luts, output
        let clahe_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clahe-bgl"),
            entries: &[
                bgl_entry(0, wgpu::BufferBindingType::Uniform),
                bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
                bgl_entry(3, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });

        let undistort_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("undistort-bgl"),
            entries: &[
                bgl_entry(0, wgpu::BufferBindingType::Uniform),
                bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });

        let gradient_histogram_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gradient-histogram-bgl"),
                entries: &[
                    bgl_entry(0, wgpu::BufferBindingType::Uniform),
                    bgl_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                    bgl_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
                ],
            });

        // Create compute pipelines
        let rotation_pipeline =
            create_pipeline(&device, &rotation_shader, &rotation_bgl, "rotation");
        let adjustment_pipeline =
            create_pipeline(&device, &adjustment_shader, &adjustment_bgl, "adjustment");
        let watermark_pipeline =
            create_pipeline(&device, &watermark_shader, &watermark_bgl, "watermark");
        let bilateral_pipeline =
            create_pipeline(&device, &bilateral_shader, &bilateral_bgl, "bilateral");
        let clahe_histogram_pipeline = create_pipeline_with_entry(
            &device,
            &clahe_shader,
            &clahe_bgl,
            "clahe-histogram",
            "main_histogram",
        );
        let clahe_apply_pipeline = create_pipeline_with_entry(
            &device,
            &clahe_shader,
            &clahe_bgl,
            "clahe-apply",
            "main_apply",
        );
        let undistort_pipeline =
            create_pipeline(&device, &undistort_shader, &undistort_bgl, "undistort");
        let gradient_histogram_pipeline = create_pipeline(
            &device,
            &gradient_histogram_shader,
            &gradient_histogram_bgl,
            "gradient-histogram",
        );

        Some(Self {
            device,
            queue,
            adapter_name,
            rotation_pipeline,
            adjustment_pipeline,
            watermark_pipeline,
            bilateral_pipeline,
            clahe_histogram_pipeline,
            clahe_apply_pipeline,
            undistort_pipeline,
            gradient_histogram_pipeline,
            rotation_bgl,
            adjustment_bgl,
            watermark_bgl,
            bilateral_bgl,
            clahe_bgl,
            undistort_bgl,
            gradient_histogram_bgl,
        })
    }
}

/// Helper: create a bind group layout entry for a compute buffer.
fn bgl_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Helper: create a compute pipeline from a shader module and bind group layout.
fn create_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    bgl: &wgpu::BindGroupLayout,
    label: &str,
) -> wgpu::ComputePipeline {
    create_pipeline_with_entry(device, shader, bgl, label, "main")
}

/// Helper: create a compute pipeline with a custom entry point name.
fn create_pipeline_with_entry(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    bgl: &wgpu::BindGroupLayout,
    label: &str,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label}-layout")),
        bind_group_layouts: &[bgl],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(&format!("{label}-pipeline")),
        layout: Some(&layout),
        module: shader,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    })
}

// ============================================================================
// GPU Operations
// ============================================================================

/// Execute a compute shader and read back the result buffer.
/// This is the shared dispatch + readback pattern used by all GPU operations.
fn gpu_dispatch_and_readback(
    ctx: &GpuContext,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    output_buffer: &wgpu::Buffer,
    output_size: u64,
    workgroups: (u32, u32, u32),
) -> Result<Vec<u8>, String> {
    // Create staging buffer for CPU readback
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging-readback"),
        size: output_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Encode compute pass + copy to staging
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute-encoder"),
        });

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("compute-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(pipeline);
        cpass.set_bind_group(0, bind_group, &[]);
        cpass.dispatch_workgroups(workgroups.0, workgroups.1, workgroups.2);
    }

    encoder.copy_buffer_to_buffer(output_buffer, 0, &staging, 0, output_size);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    // Read back to CPU
    let buffer_slice = staging.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::Wait);

    receiver
        .recv()
        .map_err(|e| format!("GPU readback channel error: {e}"))?
        .map_err(|e| format!("GPU buffer map failed: {e}"))?;

    let data = buffer_slice.get_mapped_range();
    let result = data.to_vec();
    drop(data);
    staging.unmap();

    Ok(result)
}

// ============================================================================
// GPU Fine Rotation
// ============================================================================

fn gpu_fine_rotation(
    ctx: &GpuContext,
    img: &DynamicImage,
    angle_degrees: f32,
) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();
    let aspect = width as f32 / height as f32;

    // Calculate auto-crop scale (same formula as WebGL shader and CPU path)
    let abs_angle = angle_degrees.abs().to_radians();
    let cos_a = abs_angle.cos();
    let sin_a = abs_angle.sin();
    let scale_w = 1.0 / (cos_a + sin_a / aspect);
    let scale_h = 1.0 / (cos_a + sin_a * aspect);
    let crop_scale = scale_w.min(scale_h);

    let new_width = ((width as f32) * crop_scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * crop_scale).round().max(1.0) as u32;

    let angle_rad = angle_degrees.to_radians();

    let params = RotationParams {
        src_width: width,
        src_height: height,
        dst_width: new_width,
        dst_height: new_height,
        angle_rad,
        cos_r: angle_rad.cos(),
        sin_r: angle_rad.sin(),
        crop_half_width: aspect * crop_scale * 0.5,
        crop_half_height: crop_scale * 0.5,
        aspect,
        _padding: [0.0; 2],
    };

    let rgba = img.to_rgba8();
    let src_pixels = rgba.as_raw();
    let src_size = src_pixels.len() as u64;
    let dst_size = (new_width * new_height * 4) as u64;

    // Create buffers
    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rotation-params"),
        size: size_of::<RotationParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rotation-input"),
        size: src_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rotation-output"),
        size: dst_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Upload data
    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue.write_buffer(&input_buf, 0, src_pixels);

    // Create bind group
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rotation-bg"),
        layout: &ctx.rotation_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch: workgroup size is 16x16
    let wg_x = (new_width + 15) / 16;
    let wg_y = (new_height + 15) / 16;

    let result_bytes = gpu_dispatch_and_readback(
        ctx,
        &ctx.rotation_pipeline,
        &bind_group,
        &output_buf,
        dst_size,
        (wg_x, wg_y, 1),
    )?;

    RgbaImage::from_raw(new_width, new_height, result_bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "Failed to reconstruct rotated image from GPU output".to_string())
}

// ============================================================================
// GPU Color Adjustments
// ============================================================================

fn gpu_adjustments(
    ctx: &GpuContext,
    img: &DynamicImage,
    brightness: i32,
    exposure: i32,
    contrast: i32,
    highlights: i32,
    shadows: i32,
) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();

    // Pre-compute factors matching the WebGL shader and CPU path
    let params = AdjustmentParams {
        width,
        height,
        brightness: brightness as f32 / 350.0,
        exposure: 2.0_f32.powf(exposure as f32 / 130.0),
        contrast: (contrast as f32 + 170.0) / 170.0,
        highlights: highlights as f32 / 180.0,
        shadows: shadows as f32 / 180.0,
        _padding: 0.0,
    };

    let rgba = img.to_rgba8();
    let pixels = rgba.as_raw();
    let buf_size = pixels.len() as u64;

    // Create buffers
    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("adj-params"),
        size: size_of::<AdjustmentParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("adj-input"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("adj-output"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Upload
    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue.write_buffer(&input_buf, 0, pixels);

    // Bind group
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("adj-bg"),
        layout: &ctx.adjustment_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buf.as_entire_binding(),
            },
        ],
    });

    let wg_x = (width + 15) / 16;
    let wg_y = (height + 15) / 16;

    let result_bytes = gpu_dispatch_and_readback(
        ctx,
        &ctx.adjustment_pipeline,
        &bind_group,
        &output_buf,
        buf_size,
        (wg_x, wg_y, 1),
    )?;

    RgbaImage::from_raw(width, height, result_bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "Failed to reconstruct adjusted image from GPU output".to_string())
}

// ============================================================================
// GPU Watermark Blending
// ============================================================================

fn gpu_watermark_blend(
    ctx: &GpuContext,
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    pos_x: u32,
    pos_y: u32,
    opacity: f32,
    use_alpha: bool,
) -> Result<(), String> {
    let (base_w, base_h) = base_img.dimensions();
    let (wm_w, wm_h) = watermark.dimensions();

    let params = WatermarkParams {
        base_width: base_w,
        base_height: base_h,
        wm_width: wm_w,
        wm_height: wm_h,
        pos_x,
        pos_y,
        opacity,
        use_alpha: u32::from(use_alpha),
    };

    let base_pixels = base_img.as_raw();
    let wm_pixels = watermark.as_raw();
    let base_size = base_pixels.len() as u64;
    let wm_size = wm_pixels.len() as u64;

    // Create buffers
    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wm-params"),
        size: size_of::<WatermarkParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Base image is both input and output (read-write storage)
    let base_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wm-base"),
        size: base_size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let wm_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wm-overlay"),
        size: wm_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Upload
    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue.write_buffer(&base_buf, 0, base_pixels);
    ctx.queue.write_buffer(&wm_buf, 0, wm_pixels);

    // Bind group
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("wm-bg"),
        layout: &ctx.watermark_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: base_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wm_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch over watermark dimensions (only those pixels need processing)
    let wg_x = (wm_w + 15) / 16;
    let wg_y = (wm_h + 15) / 16;

    let result_bytes = gpu_dispatch_and_readback(
        ctx,
        &ctx.watermark_pipeline,
        &bind_group,
        &base_buf,
        base_size,
        (wg_x, wg_y, 1),
    )?;

    // Copy result back into the mutable base image
    let result_img = RgbaImage::from_raw(base_w, base_h, result_bytes)
        .ok_or_else(|| "Failed to reconstruct watermarked image from GPU output".to_string())?;
    *base_img = result_img;

    Ok(())
}

// ============================================================================
// GPU Fused Rotation + Adjustments (single round-trip)
// ============================================================================

fn gpu_rotate_and_adjust(
    ctx: &GpuContext,
    img: &DynamicImage,
    angle_degrees: f32,
    brightness: i32,
    exposure: i32,
    contrast: i32,
    highlights: i32,
    shadows: i32,
) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();
    let aspect = width as f32 / height as f32;

    // Rotation parameters
    let abs_angle = angle_degrees.abs().to_radians();
    let cos_a = abs_angle.cos();
    let sin_a = abs_angle.sin();
    let scale_w = 1.0 / (cos_a + sin_a / aspect);
    let scale_h = 1.0 / (cos_a + sin_a * aspect);
    let crop_scale = scale_w.min(scale_h);
    let new_width = ((width as f32) * crop_scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * crop_scale).round().max(1.0) as u32;
    let angle_rad = angle_degrees.to_radians();

    let rot_params = RotationParams {
        src_width: width,
        src_height: height,
        dst_width: new_width,
        dst_height: new_height,
        angle_rad,
        cos_r: angle_rad.cos(),
        sin_r: angle_rad.sin(),
        crop_half_width: aspect * crop_scale * 0.5,
        crop_half_height: crop_scale * 0.5,
        aspect,
        _padding: [0.0; 2],
    };

    let adj_params = AdjustmentParams {
        width: new_width,
        height: new_height,
        brightness: brightness as f32 / 350.0,
        exposure: 2.0_f32.powf(exposure as f32 / 130.0),
        contrast: (contrast as f32 + 170.0) / 170.0,
        highlights: highlights as f32 / 180.0,
        shadows: shadows as f32 / 180.0,
        _padding: 0.0,
    };

    let rgba = img.to_rgba8();
    let src_pixels = rgba.as_raw();
    let src_size = src_pixels.len() as u64;
    let mid_size = (new_width * new_height * 4) as u64;

    // Create all buffers
    let rot_param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused-rot-params"),
        size: size_of::<RotationParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let adj_param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused-adj-params"),
        size: size_of::<AdjustmentParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused-input"),
        size: src_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // Intermediate buffer: rotation output = adjustment input (stays on GPU)
    let mid_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused-mid"),
        size: mid_size,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    // Final output buffer
    let output_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused-output"),
        size: mid_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Upload once
    ctx.queue
        .write_buffer(&rot_param_buf, 0, bytemuck::bytes_of(&rot_params));
    ctx.queue
        .write_buffer(&adj_param_buf, 0, bytemuck::bytes_of(&adj_params));
    ctx.queue.write_buffer(&input_buf, 0, src_pixels);

    // Rotation bind group: input -> mid
    let rot_bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("fused-rot-bg"),
        layout: &ctx.rotation_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: rot_param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: mid_buf.as_entire_binding(),
            },
        ],
    });

    // Adjustment bind group: mid -> output
    let adj_bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("fused-adj-bg"),
        layout: &ctx.adjustment_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: adj_param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: mid_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buf.as_entire_binding(),
            },
        ],
    });

    // Staging for readback
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused-staging"),
        size: mid_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Encode both dispatches in a single command buffer
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fused-encoder"),
        });

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("fused-rotation-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&ctx.rotation_pipeline);
        cpass.set_bind_group(0, &rot_bg, &[]);
        cpass.dispatch_workgroups((new_width + 15) / 16, (new_height + 15) / 16, 1);
    }

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("fused-adjustment-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&ctx.adjustment_pipeline);
        cpass.set_bind_group(0, &adj_bg, &[]);
        cpass.dispatch_workgroups((new_width + 15) / 16, (new_height + 15) / 16, 1);
    }

    encoder.copy_buffer_to_buffer(&output_buf, 0, &staging, 0, mid_size);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    // Readback
    let buffer_slice = staging.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::Wait);

    receiver
        .recv()
        .map_err(|e| format!("GPU readback error: {e}"))?
        .map_err(|e| format!("GPU buffer map failed: {e}"))?;

    let data = buffer_slice.get_mapped_range();
    let result = data.to_vec();
    drop(data);
    staging.unmap();

    RgbaImage::from_raw(new_width, new_height, result)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "Failed to reconstruct fused result from GPU output".to_string())
}

// ============================================================================
// GPU Gradient Histogram
// ============================================================================

fn gpu_gradient_histogram(
    ctx: &GpuContext,
    gray_pixels: &[u8],
    width: u32,
    height: u32,
    threshold: f32,
) -> Result<Vec<f32>, String> {
    let params = GradientHistogramParams {
        width,
        height,
        threshold,
        _pad: 0.0,
    };

    // Pack grayscale pixels into u32 (4 pixels per u32)
    let packed = pack_grayscale(gray_pixels);
    let packed_size = (packed.len() * 4) as u64;
    let hist_size = (GRADIENT_HISTOGRAM_BINS * 4) as u64; // u32 per bin

    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("grad-hist-params"),
        size: size_of::<GradientHistogramParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("grad-hist-input"),
        size: packed_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let hist_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("grad-hist-output"),
        size: hist_size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Zero out histogram buffer
    ctx.queue
        .write_buffer(&hist_buf, 0, &vec![0u8; hist_size as usize]);

    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue
        .write_buffer(&input_buf, 0, bytemuck::cast_slice(&packed));

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("grad-hist-bg"),
        layout: &ctx.gradient_histogram_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: hist_buf.as_entire_binding(),
            },
        ],
    });

    let wg_x = (width + 15) / 16;
    let wg_y = (height + 15) / 16;

    let result_bytes = gpu_dispatch_and_readback(
        ctx,
        &ctx.gradient_histogram_pipeline,
        &bind_group,
        &hist_buf,
        hist_size,
        (wg_x, wg_y, 1),
    )?;

    // Convert u32 histogram to f32 (divide by 256 to undo fixed-point scaling)
    let u32_hist: &[u32] = bytemuck::cast_slice(&result_bytes);
    Ok(u32_hist.iter().map(|&v| v as f32 / 256.0).collect())
}

// ============================================================================
// GPU Bilateral Filter
// ============================================================================

fn gpu_bilateral(
    ctx: &GpuContext,
    gray_pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let params = BilateralParams {
        width,
        height,
        radius: 5,
        sigma_color: 25.0,
        sigma_space: 5.0,
        _pad0: 0.0,
        _pad1: 0.0,
        _pad2: 0.0,
    };

    let packed = pack_grayscale(gray_pixels);
    let packed_size = (packed.len() * 4) as u64;

    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-params"),
        size: size_of::<BilateralParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-input"),
        size: packed_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-output"),
        size: packed_size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Zero output buffer (needed because write_pixel uses OR)
    ctx.queue
        .write_buffer(&output_buf, 0, &vec![0u8; packed_size as usize]);

    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue
        .write_buffer(&input_buf, 0, bytemuck::cast_slice(&packed));

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bilateral-bg"),
        layout: &ctx.bilateral_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buf.as_entire_binding(),
            },
        ],
    });

    let result_bytes = gpu_dispatch_and_readback(
        ctx,
        &ctx.bilateral_pipeline,
        &bind_group,
        &output_buf,
        packed_size,
        ((width + 15) / 16, (height + 15) / 16, 1),
    )?;

    Ok(unpack_grayscale(&result_bytes, width, height))
}

// ============================================================================
// GPU CLAHE
// ============================================================================

fn gpu_clahe(
    ctx: &GpuContext,
    gray_pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let grid_w: u32 = 8;
    let grid_h: u32 = 8;

    let params = ClaheParams {
        width,
        height,
        grid_w,
        grid_h,
        clip_limit: 2.0,
        _pad0: 0.0,
        _pad1: 0.0,
        _pad2: 0.0,
    };

    let packed = pack_grayscale(gray_pixels);
    let packed_size = (packed.len() * 4) as u64;
    let lut_size = (grid_w * grid_h * 256 * 4) as u64; // u32 per LUT entry

    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clahe-params"),
        size: size_of::<ClaheParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clahe-input"),
        size: packed_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let lut_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clahe-luts"),
        size: lut_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clahe-output"),
        size: packed_size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Zero buffers
    ctx.queue
        .write_buffer(&lut_buf, 0, &vec![0u8; lut_size as usize]);
    ctx.queue
        .write_buffer(&output_buf, 0, &vec![0u8; packed_size as usize]);

    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue
        .write_buffer(&input_buf, 0, bytemuck::cast_slice(&packed));

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("clahe-bg"),
        layout: &ctx.clahe_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: lut_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output_buf.as_entire_binding(),
            },
        ],
    });

    // Two-dispatch CLAHE: histogram build, then apply
    let staging = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clahe-staging"),
        size: packed_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("clahe-encoder"),
        });

    // Dispatch 1: Build histograms (one workgroup per tile, 256 threads each)
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("clahe-histogram-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&ctx.clahe_histogram_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(grid_w, grid_h, 1);
    }

    // Dispatch 2: Apply LUTs with bilinear interpolation
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("clahe-apply-pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&ctx.clahe_apply_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups((width + 15) / 16, (height + 15) / 16, 1);
    }

    encoder.copy_buffer_to_buffer(&output_buf, 0, &staging, 0, packed_size);
    ctx.queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = ctx.device.poll(wgpu::PollType::Wait);

    receiver
        .recv()
        .map_err(|e| format!("GPU readback error: {e}"))?
        .map_err(|e| format!("GPU buffer map failed: {e}"))?;

    let data = buffer_slice.get_mapped_range();
    let result = data.to_vec();
    drop(data);
    staging.unmap();

    Ok(unpack_grayscale(&result, width, height))
}

// ============================================================================
// GPU Lens Undistortion
// ============================================================================

fn gpu_undistort(ctx: &GpuContext, img: &DynamicImage, k1: f32) -> Result<DynamicImage, String> {
    let (width, height) = img.dimensions();
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let max_r = (cx * cx + cy * cy).sqrt();

    let params = UndistortParams {
        src_width: width,
        src_height: height,
        dst_width: width,
        dst_height: height,
        cx,
        cy,
        k1,
        max_r,
    };

    let rgba = img.to_rgba8();
    let pixels = rgba.as_raw();
    let buf_size = pixels.len() as u64;

    let param_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("undistort-params"),
        size: size_of::<UndistortParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let input_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("undistort-input"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let output_buf = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("undistort-output"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    ctx.queue
        .write_buffer(&param_buf, 0, bytemuck::bytes_of(&params));
    ctx.queue.write_buffer(&input_buf, 0, pixels);

    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("undistort-bg"),
        layout: &ctx.undistort_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: param_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buf.as_entire_binding(),
            },
        ],
    });

    let result_bytes = gpu_dispatch_and_readback(
        ctx,
        &ctx.undistort_pipeline,
        &bind_group,
        &output_buf,
        buf_size,
        ((width + 15) / 16, (height + 15) / 16, 1),
    )?;

    RgbaImage::from_raw(width, height, result_bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "Failed to reconstruct undistorted image from GPU output".to_string())
}

// ============================================================================
// Grayscale Packing Utilities (for bilateral, CLAHE, gradient histogram)
// ============================================================================

/// Pack grayscale pixels (1 byte each) into u32 values (4 pixels per u32).
fn pack_grayscale(pixels: &[u8]) -> Vec<u32> {
    let padded_len = (pixels.len() + 3) / 4;
    let mut packed = vec![0u32; padded_len];
    for (i, &p) in pixels.iter().enumerate() {
        let word_idx = i / 4;
        let byte_idx = i % 4;
        packed[word_idx] |= (p as u32) << (byte_idx * 8);
    }
    packed
}

/// Unpack u32 buffer back to grayscale pixels.
fn unpack_grayscale(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let total_pixels = (width * height) as usize;
    let u32_data: &[u32] = bytemuck::cast_slice(data);
    let mut result = Vec::with_capacity(total_pixels);
    for i in 0..total_pixels {
        let word_idx = i / 4;
        let byte_idx = i % 4;
        if word_idx < u32_data.len() {
            result.push(((u32_data[word_idx] >> (byte_idx * 8)) & 0xFF) as u8);
        } else {
            result.push(0);
        }
    }
    result
}

// ============================================================================
// CPU Fallback Implementations
// ============================================================================

/// CPU fallback for fine rotation with bilinear interpolation.
/// Uses rayon to parallelize row processing for better performance.
pub fn cpu_fine_rotation(img: &DynamicImage, angle_degrees: f32) -> Result<DynamicImage, String> {
    use rayon::prelude::*;

    let (width, height) = img.dimensions();
    let aspect = width as f32 / height as f32;

    let abs_angle = angle_degrees.abs().to_radians();
    let cos_a = abs_angle.cos();
    let sin_a = abs_angle.sin();
    let scale_w = 1.0 / (cos_a + sin_a / aspect);
    let scale_h = 1.0 / (cos_a + sin_a * aspect);
    let crop_scale = scale_w.min(scale_h);

    let new_width = ((width as f32) * crop_scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * crop_scale).round().max(1.0) as u32;

    let angle_radians = angle_degrees.to_radians();
    let cos_r = angle_radians.cos();
    let sin_r = angle_radians.sin();
    let crop_half_width = aspect * crop_scale * 0.5;
    let crop_half_height = crop_scale * 0.5;
    let src_cx = width as f32 / 2.0;
    let src_cy = height as f32 / 2.0;
    let dst_cx = new_width as f32 / 2.0;
    let dst_cy = new_height as f32 / 2.0;

    let rgba = img.to_rgba8();

    // Process rows in parallel using rayon
    let row_pixels: Vec<Vec<Rgba<u8>>> = (0..new_height)
        .into_par_iter()
        .map(|dst_y| {
            let mut row = Vec::with_capacity(new_width as usize);
            for dst_x in 0..new_width {
                let u = (dst_x as f32 - dst_cx) / dst_cx;
                let v = (dst_y as f32 - dst_cy) / dst_cy;
                let x_norm = u * crop_half_width;
                let y_norm = v * crop_half_height;
                let x_rot = x_norm * cos_r + y_norm * sin_r;
                let y_rot = -x_norm * sin_r + y_norm * cos_r;
                let src_x = x_rot / aspect * (width as f32) + src_cx;
                let src_y = y_rot * (height as f32) + src_cy;

                if src_x >= 0.0
                    && src_x < (width - 1) as f32
                    && src_y >= 0.0
                    && src_y < (height - 1) as f32
                {
                    row.push(bilinear_sample(&rgba, src_x, src_y));
                } else {
                    row.push(Rgba([0, 0, 0, 255]));
                }
            }
            row
        })
        .collect();

    // Reconstruct image from parallel rows
    let mut result = RgbaImage::new(new_width, new_height);
    for (y, row) in row_pixels.into_iter().enumerate() {
        for (x, pixel) in row.into_iter().enumerate() {
            result.put_pixel(x as u32, y as u32, pixel);
        }
    }

    Ok(DynamicImage::ImageRgba8(result))
}

/// Bilinear interpolation sampling (shared with CPU fallback).
#[inline]
fn bilinear_sample(img: &RgbaImage, x: f32, y: f32) -> Rgba<u8> {
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = img.get_pixel(x0, y0);
    let p10 = img.get_pixel(x1, y0);
    let p01 = img.get_pixel(x0, y1);
    let p11 = img.get_pixel(x1, y1);

    let mut result = [0u8; 4];
    for i in 0..4 {
        let v = p00[i] as f32 * (1.0 - fx) * (1.0 - fy)
            + p10[i] as f32 * fx * (1.0 - fy)
            + p01[i] as f32 * (1.0 - fx) * fy
            + p11[i] as f32 * fx * fy;
        result[i] = v.round().clamp(0.0, 255.0) as u8;
    }
    Rgba(result)
}

/// GLSL-style smoothstep function.
#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// CPU fallback for color adjustments (parallelized with rayon).
pub fn cpu_adjustments(
    img: &DynamicImage,
    brightness: i32,
    exposure: i32,
    contrast: i32,
    highlights: i32,
    shadows: i32,
) -> DynamicImage {
    use rayon::prelude::*;

    if brightness == 0 && exposure == 0 && contrast == 0 && highlights == 0 && shadows == 0 {
        return img.clone();
    }

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let b_factor = brightness as f32 / 350.0;
    let e_factor = 2.0_f32.powf(exposure as f32 / 130.0);
    let c_factor = (contrast as f32 + 170.0) / 170.0;
    let h_factor = highlights as f32 / 180.0;
    let s_factor = shadows as f32 / 180.0;

    let pixels: Vec<Rgba<u8>> = rgba
        .pixels()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|p| {
            let mut r = p[0] as f32 / 255.0;
            let mut g = p[1] as f32 / 255.0;
            let mut b = p[2] as f32 / 255.0;

            r *= e_factor;
            g *= e_factor;
            b *= e_factor;
            r += b_factor;
            g += b_factor;
            b += b_factor;
            r = (r - 0.5) * c_factor + 0.5;
            g = (g - 0.5) * c_factor + 0.5;
            b = (b - 0.5) * c_factor + 0.5;

            if h_factor != 0.0 || s_factor != 0.0 {
                let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let h_mask = smoothstep(0.3, 0.7, lum);
                let s_mask = 1.0 - h_mask;
                let adj = (h_factor * h_mask + s_factor * s_mask) * 0.5;
                r += adj;
                g += adj;
                b += adj;
            }

            Rgba([
                (r.clamp(0.0, 1.0) * 255.0) as u8,
                (g.clamp(0.0, 1.0) * 255.0) as u8,
                (b.clamp(0.0, 1.0) * 255.0) as u8,
                p[3],
            ])
        })
        .collect();

    let mut result = RgbaImage::new(width, height);
    for (i, pixel) in pixels.into_iter().enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        result.put_pixel(x, y, pixel);
    }

    DynamicImage::ImageRgba8(result)
}

/// CPU fallback for watermark blending.
pub fn cpu_blend_watermark(
    base_img: &mut RgbaImage,
    watermark: &RgbaImage,
    pos_x: u32,
    pos_y: u32,
    opacity: f32,
    use_alpha: bool,
) {
    let (base_width, base_height) = base_img.dimensions();
    let (wm_width, wm_height) = watermark.dimensions();

    for y in 0..wm_height {
        for x in 0..wm_width {
            let base_x = pos_x + x;
            let base_y = pos_y + y;

            if base_x < base_width && base_y < base_height {
                let base_pixel = base_img.get_pixel_mut(base_x, base_y);
                let wm_pixel = watermark.get_pixel(x, y);

                let wm_alpha = if use_alpha {
                    (wm_pixel[3] as f32 / 255.0 * opacity).min(1.0)
                } else {
                    opacity
                };

                for c in 0..3 {
                    let base_val = base_pixel[c] as f32 / 255.0;
                    let wm_val = wm_pixel[c] as f32 / 255.0;
                    let blended = base_val * (1.0 - wm_alpha) + wm_val * wm_alpha;
                    base_pixel[c] = (blended * 255.0) as u8;
                }
            }
        }
    }
}

// ============================================================================
// CPU Fallback: Gradient Histogram (replaces OpenCV LSD)
// ============================================================================

/// CPU fallback for gradient histogram computation.
/// Computes 3x3 Sobel gradients and votes magnitude-weighted into 3600 angle bins.
fn cpu_gradient_histogram(
    gray_pixels: &[u8],
    width: u32,
    height: u32,
    magnitude_threshold: f32,
) -> Vec<f32> {
    use rayon::prelude::*;

    let w = width as usize;
    let h = height as usize;
    let num_bins = GRADIENT_HISTOGRAM_BINS as usize;

    // Each thread accumulates a local histogram, then we reduce
    let local_hists: Vec<Vec<f64>> = (1..h - 1)
        .into_par_iter()
        .map(|y| {
            let mut local = vec![0.0_f64; num_bins];
            for x in 1..w - 1 {
                // 3x3 Sobel
                let tl = gray_pixels[(y - 1) * w + (x - 1)] as f64;
                let tc = gray_pixels[(y - 1) * w + x] as f64;
                let tr = gray_pixels[(y - 1) * w + (x + 1)] as f64;
                let ml = gray_pixels[y * w + (x - 1)] as f64;
                let mr = gray_pixels[y * w + (x + 1)] as f64;
                let bl = gray_pixels[(y + 1) * w + (x - 1)] as f64;
                let bc = gray_pixels[(y + 1) * w + x] as f64;
                let br = gray_pixels[(y + 1) * w + (x + 1)] as f64;

                let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
                let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

                let magnitude = (gx * gx + gy * gy).sqrt();
                if magnitude < magnitude_threshold as f64 {
                    continue;
                }

                // atan2 gives angle in [-pi, pi], map to [0, 360) degrees
                let angle_rad = gy.atan2(gx);
                let mut angle_deg = angle_rad.to_degrees();
                if angle_deg < 0.0 {
                    angle_deg += 360.0;
                }

                // Bin at 0.1 degree resolution
                let bin = ((angle_deg * 10.0).round() as usize).min(num_bins - 1);
                local[bin] += magnitude;
            }
            local
        })
        .collect();

    // Reduce all local histograms into one
    let mut histogram = vec![0.0_f64; num_bins];
    for local in &local_hists {
        for (i, val) in local.iter().enumerate() {
            histogram[i] += val;
        }
    }

    // Convert to f32
    histogram.iter().map(|&v| v as f32).collect()
}

// ============================================================================
// CPU Fallback: Bilateral Filter
// ============================================================================

/// CPU fallback for bilateral filter on grayscale data.
/// 11x11 kernel, sigma_color=25, sigma_space=5 (matching the GPU shader defaults).
fn cpu_bilateral(gray_pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    use rayon::prelude::*;

    let w = width as usize;
    let h = height as usize;
    let radius: i32 = 5; // 11x11 kernel
    let sigma_color: f64 = 25.0;
    let sigma_space: f64 = 5.0;

    // Precompute spatial weights
    let kernel_size = (2 * radius + 1) as usize;
    let mut spatial_weights = vec![0.0_f64; kernel_size * kernel_size];
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let dist_sq = (dx * dx + dy * dy) as f64;
            let w_val = (-dist_sq / (2.0 * sigma_space * sigma_space)).exp();
            let ky = (dy + radius) as usize;
            let kx = (dx + radius) as usize;
            spatial_weights[ky * kernel_size + kx] = w_val;
        }
    }

    let rows: Vec<Vec<u8>> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![0u8; w];
            for x in 0..w {
                let center_val = gray_pixels[y * w + x] as f64;
                let mut sum = 0.0_f64;
                let mut weight_sum = 0.0_f64;

                for dy in -radius..=radius {
                    let ny = y as i32 + dy;
                    if ny < 0 || ny >= h as i32 {
                        continue;
                    }
                    for dx in -radius..=radius {
                        let nx = x as i32 + dx;
                        if nx < 0 || nx >= w as i32 {
                            continue;
                        }

                        let neighbor_val = gray_pixels[ny as usize * w + nx as usize] as f64;
                        let ky = (dy + radius) as usize;
                        let kx = (dx + radius) as usize;
                        let spatial_w = spatial_weights[ky * kernel_size + kx];

                        let color_diff = (neighbor_val - center_val).abs();
                        let color_w =
                            (-color_diff * color_diff / (2.0 * sigma_color * sigma_color)).exp();

                        let weight = spatial_w * color_w;
                        sum += neighbor_val * weight;
                        weight_sum += weight;
                    }
                }

                row[x] = if weight_sum > 0.0 {
                    (sum / weight_sum).round().clamp(0.0, 255.0) as u8
                } else {
                    center_val as u8
                };
            }
            row
        })
        .collect();

    rows.into_iter().flatten().collect()
}

// ============================================================================
// CPU Fallback: CLAHE (Contrast Limited Adaptive Histogram Equalization)
// ============================================================================

/// CPU fallback for CLAHE on grayscale data.
/// Uses 8x8 grid, clip limit 2.0 (matching the GPU shader defaults).
fn cpu_clahe(gray_pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let grid_size: usize = 8;
    let clip_limit: f32 = 2.0;

    let tile_width = (w + grid_size - 1) / grid_size;
    let tile_height = (h + grid_size - 1) / grid_size;

    // Compute LUT for each tile
    let mut tile_mappings: Vec<Vec<[u8; 256]>> = vec![vec![[0u8; 256]; grid_size]; grid_size];

    for ty in 0..grid_size {
        for tx in 0..grid_size {
            let x_start = tx * tile_width;
            let y_start = ty * tile_height;
            let x_end = ((tx + 1) * tile_width).min(w);
            let y_end = ((ty + 1) * tile_height).min(h);

            // Build histogram
            let mut hist = [0u32; 256];
            let mut pixel_count = 0u32;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    if x < w && y < h {
                        let val = gray_pixels[y * w + x] as usize;
                        hist[val] += 1;
                        pixel_count += 1;
                    }
                }
            }

            if pixel_count == 0 {
                for i in 0..256 {
                    tile_mappings[ty][tx][i] = i as u8;
                }
                continue;
            }

            // Clip histogram
            let clip_threshold = (clip_limit * (pixel_count as f32) / 256.0) as u32;
            let mut excess = 0u32;
            for h_val in &mut hist {
                if *h_val > clip_threshold {
                    excess += *h_val - clip_threshold;
                    *h_val = clip_threshold;
                }
            }

            // Redistribute excess
            let redistrib = excess / 256;
            for h_val in &mut hist {
                *h_val += redistrib;
            }

            // Build CDF and mapping
            let mut cdf = [0u32; 256];
            cdf[0] = hist[0];
            for i in 1..256 {
                cdf[i] = cdf[i - 1] + hist[i];
            }

            let cdf_min = cdf.iter().copied().find(|&v| v > 0).unwrap_or(0);
            let scale = if pixel_count > cdf_min {
                255.0 / (pixel_count - cdf_min) as f32
            } else {
                1.0
            };

            for i in 0..256 {
                let mapped = if cdf[i] > cdf_min {
                    ((cdf[i] - cdf_min) as f32 * scale)
                        .round()
                        .clamp(0.0, 255.0) as u8
                } else {
                    0
                };
                tile_mappings[ty][tx][i] = mapped;
            }
        }
    }

    // Apply with bilinear interpolation between tiles
    let mut output = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let val = gray_pixels[y * w + x] as usize;

            let fx = (x as f32) / (tile_width as f32) - 0.5;
            let fy = (y as f32) / (tile_height as f32) - 0.5;

            let tx0 = (fx.floor() as i32).clamp(0, grid_size as i32 - 1) as usize;
            let ty0 = (fy.floor() as i32).clamp(0, grid_size as i32 - 1) as usize;
            let tx1 = (tx0 + 1).min(grid_size - 1);
            let ty1 = (ty0 + 1).min(grid_size - 1);

            let wx = (fx - tx0 as f32).clamp(0.0, 1.0);
            let wy = (fy - ty0 as f32).clamp(0.0, 1.0);

            let v00 = tile_mappings[ty0][tx0][val] as f32;
            let v10 = tile_mappings[ty0][tx1][val] as f32;
            let v01 = tile_mappings[ty1][tx0][val] as f32;
            let v11 = tile_mappings[ty1][tx1][val] as f32;

            let interpolated = v00 * (1.0 - wx) * (1.0 - wy)
                + v10 * wx * (1.0 - wy)
                + v01 * (1.0 - wx) * wy
                + v11 * wx * wy;

            output[y * w + x] = interpolated.round().clamp(0.0, 255.0) as u8;
        }
    }

    output
}

// ============================================================================
// CPU Fallback: Lens Undistortion (Brown-Conrady radial model)
// ============================================================================

/// CPU fallback for radial lens undistortion (rayon-parallelized).
/// Applies Brown-Conrady model: r_corrected = r * (1 + k1*r²)
fn cpu_undistort(img: &DynamicImage, k1: f32) -> DynamicImage {
    use rayon::prelude::*;

    let (width, height) = img.dimensions();
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let max_r = (cx * cx + cy * cy).sqrt();

    let rgba = img.to_rgba8();
    let w = width as usize;

    // Process rows in parallel
    let rows: Vec<Vec<u8>> = (0..height)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![0u8; w * 4];
            for x in 0..width {
                let dx = (x as f32 - cx) / max_r;
                let dy = (y as f32 - cy) / max_r;
                let r_sq = dx * dx + dy * dy;

                let factor = 1.0 + k1 * r_sq;
                let src_x = cx + dx * max_r * factor;
                let src_y = cy + dy * max_r * factor;

                let pixel = if src_x >= 0.0
                    && src_x < (width - 1) as f32
                    && src_y >= 0.0
                    && src_y < (height - 1) as f32
                {
                    bilinear_sample(&rgba, src_x, src_y)
                } else {
                    Rgba([0, 0, 0, 255])
                };

                let off = x as usize * 4;
                row[off] = pixel[0];
                row[off + 1] = pixel[1];
                row[off + 2] = pixel[2];
                row[off + 3] = pixel[3];
            }
            row
        })
        .collect();

    let flat: Vec<u8> = rows.into_iter().flatten().collect();
    let output = RgbaImage::from_raw(width, height, flat)
        .expect("cpu_undistort: output buffer size mismatch");
    DynamicImage::ImageRgba8(output)
}
