// Fine rotation with bilinear interpolation and mathematical auto-crop.
// Matches the WebGL shader and CPU path exactly for consistent results.

struct Params {
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
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

// Unpack a u32 (little-endian RGBA) into vec4<f32> in [0,1]
fn unpack_rgba(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32((packed >> 24u) & 0xFFu) / 255.0,
    );
}

// Pack vec4<f32> [0,1] back to u32
fn pack_rgba(c: vec4<f32>) -> u32 {
    let r = u32(clamp(c.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(c.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(c.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(c.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

// Bilinear interpolation sample from the source image
fn sample_bilinear(sx: f32, sy: f32) -> vec4<f32> {
    let x0 = u32(floor(sx));
    let y0 = u32(floor(sy));
    let x1 = min(x0 + 1u, params.src_width - 1u);
    let y1 = min(y0 + 1u, params.src_height - 1u);

    let fx = sx - floor(sx);
    let fy = sy - floor(sy);

    let p00 = unpack_rgba(input[y0 * params.src_width + x0]);
    let p10 = unpack_rgba(input[y0 * params.src_width + x1]);
    let p01 = unpack_rgba(input[y1 * params.src_width + x0]);
    let p11 = unpack_rgba(input[y1 * params.src_width + x1]);

    let top = mix(p00, p10, fx);
    let bot = mix(p01, p11, fx);
    return mix(top, bot, fy);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_x = gid.x;
    let dst_y = gid.y;

    // Bounds check (workgroups may extend beyond image)
    if (dst_x >= params.dst_width || dst_y >= params.dst_height) {
        return;
    }

    let dst_cx = f32(params.dst_width) / 2.0;
    let dst_cy = f32(params.dst_height) / 2.0;
    let src_cx = f32(params.src_width) / 2.0;
    let src_cy = f32(params.src_height) / 2.0;

    // Convert output pixel to normalized coordinates within inscribed rectangle
    let u = (f32(dst_x) - dst_cx) / dst_cx;
    let v = (f32(dst_y) - dst_cy) / dst_cy;

    let x_norm = u * params.crop_half_width;
    let y_norm = v * params.crop_half_height;

    // Inverse rotation in normalized space
    let x_rot = x_norm * params.cos_r + y_norm * params.sin_r;
    let y_rot = -x_norm * params.sin_r + y_norm * params.cos_r;

    // Convert back to source pixel coordinates
    let src_x = x_rot / params.aspect * f32(params.src_width) + src_cx;
    let src_y = y_rot * f32(params.src_height) + src_cy;

    var pixel: u32;
    if (src_x >= 0.0 && src_x < f32(params.src_width - 1u) &&
        src_y >= 0.0 && src_y < f32(params.src_height - 1u)) {
        pixel = pack_rgba(sample_bilinear(src_x, src_y));
    } else {
        // Out of bounds - black with full alpha
        pixel = pack_rgba(vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }

    output[dst_y * params.dst_width + dst_x] = pixel;
}
