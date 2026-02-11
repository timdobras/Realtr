// CLAHE (Contrast Limited Adaptive Histogram Equalization) compute shader.
// Two-dispatch approach:
//   Dispatch 1 (main_histogram): Build per-tile histograms, apply clip limit, compute CDF LUTs
//   Dispatch 2 (main_apply): Apply LUTs with bilinear interpolation between tiles

struct Params {
    width: u32,
    height: u32,
    grid_w: u32,        // Number of tiles horizontally (8)
    grid_h: u32,        // Number of tiles vertically (8)
    clip_limit: f32,    // CLAHE clip limit (2.0)
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

const HIST_SIZE: u32 = 256u;

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;        // Grayscale packed 4px/u32
@group(0) @binding(2) var<storage, read_write> luts: array<u32>;   // grid_w*grid_h*256 LUT values
@group(0) @binding(3) var<storage, read_write> output: array<u32>; // Output grayscale packed 4px/u32

// Workgroup-shared histogram for the tile
var<workgroup> tile_hist: array<atomic<u32>, 256>;

// Read a grayscale pixel (0-255)
fn read_pixel(x: u32, y: u32) -> u32 {
    let linear = y * params.width + x;
    let word_idx = linear / 4u;
    let byte_idx = linear % 4u;
    let word = input[word_idx];
    return (word >> (byte_idx * 8u)) & 0xFFu;
}

// ============================================================================
// Dispatch 1: Build histogram + CDF for one tile per workgroup
// Launch with (grid_w, grid_h, 1) workgroups, each with 256 threads
// ============================================================================
@compute @workgroup_size(256)
fn main_histogram(
    @builtin(workgroup_id) wg_id: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    let tile_x = wg_id.x;
    let tile_y = wg_id.y;

    // Initialize histogram
    atomicStore(&tile_hist[lid], 0u);
    workgroupBarrier();

    // Compute tile boundaries
    let tile_w = params.width / params.grid_w;
    let tile_h = params.height / params.grid_h;
    let x_start = tile_x * tile_w;
    let y_start = tile_y * tile_h;
    let x_end = min(x_start + tile_w, params.width);
    let y_end = min(y_start + tile_h, params.height);

    // Each thread processes multiple pixels in the tile
    let total_pixels = (x_end - x_start) * (y_end - y_start);
    let pixels_per_thread = (total_pixels + 255u) / 256u;

    for (var i = 0u; i < pixels_per_thread; i = i + 1u) {
        let pixel_idx = lid + i * 256u;
        if pixel_idx < total_pixels {
            let local_x = pixel_idx % (x_end - x_start);
            let local_y = pixel_idx / (x_end - x_start);
            let px = x_start + local_x;
            let py = y_start + local_y;
            if px < params.width && py < params.height {
                let val = read_pixel(px, py);
                atomicAdd(&tile_hist[val], 1u);
            }
        }
    }
    workgroupBarrier();

    // Now thread 0 applies clip limit and computes CDF
    if lid == 0u {
        let clip_val = u32(params.clip_limit * f32(total_pixels) / 256.0);

        // Clip and compute excess
        var excess: u32 = 0u;
        for (var i = 0u; i < HIST_SIZE; i = i + 1u) {
            let count = atomicLoad(&tile_hist[i]);
            if count > clip_val {
                excess = excess + count - clip_val;
                atomicStore(&tile_hist[i], clip_val);
            }
        }

        // Redistribute excess evenly
        let redistrib = excess / HIST_SIZE;
        for (var i = 0u; i < HIST_SIZE; i = i + 1u) {
            atomicStore(&tile_hist[i], atomicLoad(&tile_hist[i]) + redistrib);
        }

        // Compute CDF and store as LUT
        let lut_offset = (tile_y * params.grid_w + tile_x) * HIST_SIZE;
        var cdf: u32 = 0u;
        let scale = 255.0 / f32(total_pixels);
        for (var i = 0u; i < HIST_SIZE; i = i + 1u) {
            cdf = cdf + atomicLoad(&tile_hist[i]);
            let mapped = u32(clamp(f32(cdf) * scale, 0.0, 255.0));
            luts[lut_offset + i] = mapped;
        }
    }
}

// ============================================================================
// Dispatch 2: Apply LUTs with bilinear interpolation
// Launch with (ceil(width/16), ceil(height/16), 1) workgroups
// ============================================================================
@compute @workgroup_size(16, 16)
fn main_apply(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;

    if x >= params.width || y >= params.height {
        return;
    }

    let pixel_val = read_pixel(x, y);

    let tile_w = f32(params.width) / f32(params.grid_w);
    let tile_h = f32(params.height) / f32(params.grid_h);

    // Find the floating-point tile coordinates
    // Tile centers are at (tile_x + 0.5) * tile_w, (tile_y + 0.5) * tile_h
    let tile_fx = (f32(x) / tile_w) - 0.5;
    let tile_fy = (f32(y) / tile_h) - 0.5;

    let tx0 = u32(max(floor(tile_fx), 0.0));
    let ty0 = u32(max(floor(tile_fy), 0.0));
    let tx1 = min(tx0 + 1u, params.grid_w - 1u);
    let ty1 = min(ty0 + 1u, params.grid_h - 1u);

    let fx = clamp(tile_fx - floor(tile_fx), 0.0, 1.0);
    let fy = clamp(tile_fy - floor(tile_fy), 0.0, 1.0);

    // Look up the mapped value in each of the 4 surrounding tile LUTs
    let v00 = f32(luts[(ty0 * params.grid_w + tx0) * HIST_SIZE + pixel_val]);
    let v10 = f32(luts[(ty0 * params.grid_w + tx1) * HIST_SIZE + pixel_val]);
    let v01 = f32(luts[(ty1 * params.grid_w + tx0) * HIST_SIZE + pixel_val]);
    let v11 = f32(luts[(ty1 * params.grid_w + tx1) * HIST_SIZE + pixel_val]);

    // Bilinear interpolation
    let result = mix(mix(v00, v10, fx), mix(v01, v11, fx), fy);

    // Write output pixel
    let linear = y * params.width + x;
    let word_idx = linear / 4u;
    let byte_idx = linear % 4u;
    let shift = byte_idx * 8u;
    let clamped = u32(clamp(result, 0.0, 255.0));

    // Safe because each pixel is written by exactly one thread
    output[word_idx] = output[word_idx] | (clamped << shift);
}
