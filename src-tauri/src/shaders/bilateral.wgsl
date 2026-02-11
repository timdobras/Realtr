// Bilateral filter compute shader for edge-preserving smoothing.
// Operates on grayscale images stored as packed u32 (4 pixels per u32).
// Workgroup size 16x16 = 256 threads.

struct Params {
    width: u32,
    height: u32,
    radius: u32,
    sigma_color: f32,
    sigma_space: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

// Read a single grayscale pixel (0-255) from the packed u32 buffer.
// Pixels are stored as individual bytes: 4 pixels per u32 in little-endian order.
fn read_pixel(x: u32, y: u32) -> f32 {
    let linear = y * params.width + x;
    let word_idx = linear / 4u;
    let byte_idx = linear % 4u;
    let word = input[word_idx];
    return f32((word >> (byte_idx * 8u)) & 0xFFu);
}

// Write a single grayscale pixel to the packed u32 buffer using atomics.
// Since multiple threads might write to the same u32, we use atomic OR.
// NOTE: This is safe only if each pixel within a u32 is written by exactly one thread.
fn write_pixel(x: u32, y: u32, value: f32) {
    let linear = y * params.width + x;
    let word_idx = linear / 4u;
    let byte_idx = linear % 4u;
    let clamped = u32(clamp(value, 0.0, 255.0));
    // Since each thread writes a unique (x,y), and workgroups don't overlap on output,
    // we can safely read-modify-write. But we need to be careful with packed u32.
    // We'll use a separate output buffer and reconstruct in groups of 4.
    let shift = byte_idx * 8u;
    // Atomic OR to set our byte within the u32
    // This works because output is initialized to 0 and each byte is written exactly once.
    output[word_idx] = output[word_idx] | (clamped << shift);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;

    if x >= params.width || y >= params.height {
        return;
    }

    let center_val = read_pixel(x, y);
    let inv_2_sigma_color_sq = 1.0 / (2.0 * params.sigma_color * params.sigma_color);
    let inv_2_sigma_space_sq = 1.0 / (2.0 * params.sigma_space * params.sigma_space);

    var weight_sum: f32 = 0.0;
    var value_sum: f32 = 0.0;
    let r = i32(params.radius);

    for (var dy: i32 = -r; dy <= r; dy = dy + 1) {
        for (var dx: i32 = -r; dx <= r; dx = dx + 1) {
            let nx = i32(x) + dx;
            let ny = i32(y) + dy;

            // Clamp to image bounds
            if nx < 0 || nx >= i32(params.width) || ny < 0 || ny >= i32(params.height) {
                continue;
            }

            let neighbor_val = read_pixel(u32(nx), u32(ny));

            // Spatial weight: Gaussian based on distance
            let dist_sq = f32(dx * dx + dy * dy);
            let spatial_weight = exp(-dist_sq * inv_2_sigma_space_sq);

            // Color/intensity weight: Gaussian based on intensity difference
            let diff = center_val - neighbor_val;
            let color_weight = exp(-(diff * diff) * inv_2_sigma_color_sq);

            let w = spatial_weight * color_weight;
            weight_sum += w;
            value_sum += neighbor_val * w;
        }
    }

    let result = select(center_val, value_sum / weight_sum, weight_sum > 0.0);
    write_pixel(x, y, result);
}
