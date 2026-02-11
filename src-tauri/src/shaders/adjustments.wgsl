// Color adjustments: brightness, exposure, contrast, highlights, shadows.
// Matches the WebGL shader (WebGLCanvas.svelte) and CPU path exactly.

struct Params {
    width: u32,
    height: u32,
    brightness: f32,    // pre-computed: value / 350.0
    exposure: f32,      // pre-computed: pow(2.0, value / 130.0)
    contrast: f32,      // pre-computed: (value + 170.0) / 170.0
    highlights: f32,    // pre-computed: value / 180.0
    shadows: f32,       // pre-computed: value / 180.0
    _padding: f32,
};

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
    let r = u32(clamp(c.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(c.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(c.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(c.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;

    if (x >= params.width || y >= params.height) {
        return;
    }

    let idx = y * params.width + x;
    let pixel = unpack_rgba(input[idx]);

    // Extract RGB (alpha passes through unchanged)
    var r = pixel.r;
    var g = pixel.g;
    var b = pixel.b;

    // 1. Exposure: multiplicative (simulates f-stops)
    r *= params.exposure;
    g *= params.exposure;
    b *= params.exposure;

    // 2. Brightness: additive
    r += params.brightness;
    g += params.brightness;
    b += params.brightness;

    // 3. Contrast: pivot around 0.5
    r = (r - 0.5) * params.contrast + 0.5;
    g = (g - 0.5) * params.contrast + 0.5;
    b = (b - 0.5) * params.contrast + 0.5;

    // 4. Highlights/Shadows: luminance-based masking with smoothstep
    if (params.highlights != 0.0 || params.shadows != 0.0) {
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let highlight_mask = smoothstep(0.3, 0.7, luminance);
        let shadow_mask = 1.0 - highlight_mask;
        let adjustment = (params.highlights * highlight_mask + params.shadows * shadow_mask) * 0.5;
        r += adjustment;
        g += adjustment;
        b += adjustment;
    }

    output[idx] = pack_rgba(vec4<f32>(
        clamp(r, 0.0, 1.0),
        clamp(g, 0.0, 1.0),
        clamp(b, 0.0, 1.0),
        pixel.a,
    ));
}
