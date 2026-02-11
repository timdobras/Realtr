// Lens distortion correction compute shader.
// Brown-Conrady radial undistortion model: r_corrected = r * (1 + k1*r^2)
// Backward-mapping with bilinear interpolation.
// Operates on RGBA images stored as packed u32 (one pixel per u32).

struct Params {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    cx: f32,        // Center X (width/2)
    cy: f32,        // Center Y (height/2)
    k1: f32,        // Radial distortion coefficient (negative for barrel)
    max_r: f32,     // Maximum radius for normalization = sqrt(cx^2 + cy^2)
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

fn unpack_rgba(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32((packed >> 24u) & 0xFFu) / 255.0,
    );
}

fn pack_rgba(c: vec4<f32>) -> u32 {
    let r = u32(clamp(c.x, 0.0, 1.0) * 255.0);
    let g = u32(clamp(c.y, 0.0, 1.0) * 255.0);
    let b = u32(clamp(c.z, 0.0, 1.0) * 255.0);
    let a = u32(clamp(c.w, 0.0, 1.0) * 255.0);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

fn sample_bilinear(x: f32, y: f32) -> vec4<f32> {
    let x0 = u32(floor(x));
    let y0 = u32(floor(y));
    let x1 = min(x0 + 1u, params.src_width - 1u);
    let y1 = min(y0 + 1u, params.src_height - 1u);
    let fx = x - floor(x);
    let fy = y - floor(y);

    let p00 = unpack_rgba(input[y0 * params.src_width + x0]);
    let p10 = unpack_rgba(input[y0 * params.src_width + x1]);
    let p01 = unpack_rgba(input[y1 * params.src_width + x0]);
    let p11 = unpack_rgba(input[y1 * params.src_width + x1]);

    return mix(mix(p00, p10, fx), mix(p01, p11, fx), fy);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;

    if x >= params.dst_width || y >= params.dst_height {
        return;
    }

    // Convert pixel position to centered coordinates
    let dx = f32(x) - params.cx;
    let dy = f32(y) - params.cy;

    // Normalize radius by max_r
    let r_norm = sqrt(dx * dx + dy * dy) / params.max_r;

    // Brown-Conrady: apply radial distortion to find source position
    let r_distorted = r_norm * (1.0 + params.k1 * r_norm * r_norm);

    // Scale back to pixel coordinates
    let scale = select(r_distorted / r_norm, 1.0, r_norm < 0.0001);
    let src_x = dx * scale + params.cx;
    let src_y = dy * scale + params.cy;

    let idx = y * params.dst_width + x;

    if src_x >= 0.0 && src_x < f32(params.src_width - 1u) && src_y >= 0.0 && src_y < f32(params.src_height - 1u) {
        output[idx] = pack_rgba(sample_bilinear(src_x, src_y));
    } else {
        // Out of bounds: black with full alpha
        output[idx] = pack_rgba(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }
}
