// Gradient angle histogram compute shader.
// Replaces OpenCV LSD for auto-straighten angle detection.
//
// Algorithm:
// 1. Compute 3x3 Sobel gradient (Gx, Gy) per pixel
// 2. Compute magnitude and angle
// 3. If magnitude > threshold, vote into a 3600-bin angle histogram (0.1 degree resolution)
//
// Uses workgroup-local histograms to minimize atomic contention, then
// accumulates into the global histogram.

struct Params {
    width: u32,
    height: u32,
    threshold: f32,     // Gradient magnitude threshold
    _pad: f32,
}

const HISTOGRAM_BINS: u32 = 3600u;  // 360 degrees * 10 (0.1 degree resolution)
const WG_SIZE: u32 = 256u;          // 16x16 workgroup

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input: array<u32>;      // Grayscale packed 4 pixels per u32
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;  // 3600-bin output

// Workgroup-local histogram to reduce global atomic contention
var<workgroup> local_hist: array<atomic<u32>, 3600>;

// Read a grayscale pixel value (0-255) from packed u32 buffer
fn read_pixel(x: i32, y: i32) -> f32 {
    // Clamp to image bounds
    let cx = clamp(x, 0, i32(params.width) - 1);
    let cy = clamp(y, 0, i32(params.height) - 1);
    let linear = u32(cy) * params.width + u32(cx);
    let word_idx = linear / 4u;
    let byte_idx = linear % 4u;
    let word = input[word_idx];
    return f32((word >> (byte_idx * 8u)) & 0xFFu);
}

@compute @workgroup_size(16, 16)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    // Initialize local histogram (each thread clears multiple bins)
    for (var i = lid; i < HISTOGRAM_BINS; i = i + WG_SIZE) {
        atomicStore(&local_hist[i], 0u);
    }
    workgroupBarrier();

    let x = i32(gid.x);
    let y = i32(gid.y);

    if gid.x < params.width && gid.y < params.height {
        // Skip border pixels (Sobel needs 1-pixel neighborhood)
        if x >= 1 && x < i32(params.width) - 1 && y >= 1 && y < i32(params.height) - 1 {
            // 3x3 Sobel gradient
            // Gx kernel: [-1 0 1; -2 0 2; -1 0 1]
            let gx = -read_pixel(x - 1, y - 1) - 2.0 * read_pixel(x - 1, y) - read_pixel(x - 1, y + 1)
                    + read_pixel(x + 1, y - 1) + 2.0 * read_pixel(x + 1, y) + read_pixel(x + 1, y + 1);

            // Gy kernel: [-1 -2 -1; 0 0 0; 1 2 1]
            let gy = -read_pixel(x, y - 1) * 2.0 - read_pixel(x - 1, y - 1) - read_pixel(x + 1, y - 1)
                    + read_pixel(x, y + 1) * 2.0 + read_pixel(x - 1, y + 1) + read_pixel(x + 1, y + 1);

            let magnitude = sqrt(gx * gx + gy * gy);

            if magnitude > params.threshold {
                // Compute angle in degrees [0, 360)
                var angle = atan2(gy, gx) * (180.0 / 3.14159265359);
                if angle < 0.0 {
                    angle = angle + 360.0;
                }

                // Quantize to 0.1 degree bins
                let bin = u32(angle * 10.0) % HISTOGRAM_BINS;

                // Vote with magnitude as weight (stored as fixed-point: weight * 256)
                let weight = u32(magnitude * 256.0);
                atomicAdd(&local_hist[bin], weight);
            }
        }
    }

    // Wait for all threads in workgroup to finish
    workgroupBarrier();

    // Reduce: accumulate local histogram into global histogram
    for (var i = lid; i < HISTOGRAM_BINS; i = i + WG_SIZE) {
        let val = atomicLoad(&local_hist[i]);
        if val > 0u {
            atomicAdd(&histogram[i], val);
        }
    }
}
