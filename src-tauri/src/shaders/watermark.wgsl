// Watermark alpha blending.
// Composites a watermark image onto a base image at a given position with configurable opacity.

struct Params {
    base_width: u32,
    base_height: u32,
    wm_width: u32,
    wm_height: u32,
    pos_x: u32,
    pos_y: u32,
    opacity: f32,
    use_alpha: u32,    // 0 = uniform opacity, 1 = use watermark alpha channel
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> base: array<u32>;   // base image (read-write)
@group(0) @binding(2) var<storage, read> watermark: array<u32>;    // watermark overlay (read-only)

fn unpack_rgba(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32((packed >> 24u) & 0xFFu) / 255.0,
    );
}

fn pack_rgba(c: vec4<f32>) -> u32 {
    let r = u32(clamp(c.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(c.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(c.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(c.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let wm_x = gid.x;
    let wm_y = gid.y;

    // Bounds check: within watermark dimensions
    if (wm_x >= params.wm_width || wm_y >= params.wm_height) {
        return;
    }

    // Compute base image coordinates
    let base_x = params.pos_x + wm_x;
    let base_y = params.pos_y + wm_y;

    // Bounds check: within base image
    if (base_x >= params.base_width || base_y >= params.base_height) {
        return;
    }

    let base_idx = base_y * params.base_width + base_x;
    let wm_idx = wm_y * params.wm_width + wm_x;

    let base_pixel = unpack_rgba(base[base_idx]);
    let wm_pixel = unpack_rgba(watermark[wm_idx]);

    // Calculate effective alpha
    var wm_alpha: f32;
    if (params.use_alpha == 1u) {
        wm_alpha = min(wm_pixel.a * params.opacity, 1.0);
    } else {
        wm_alpha = params.opacity;
    }

    // Alpha blend: result = base * (1 - alpha) + watermark * alpha
    let blended = vec4<f32>(
        base_pixel.r * (1.0 - wm_alpha) + wm_pixel.r * wm_alpha,
        base_pixel.g * (1.0 - wm_alpha) + wm_pixel.g * wm_alpha,
        base_pixel.b * (1.0 - wm_alpha) + wm_pixel.b * wm_alpha,
        base_pixel.a,  // Keep base alpha unchanged
    );

    base[base_idx] = pack_rgba(blended);
}
