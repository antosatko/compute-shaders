@group(0) @binding(0)
var<storage, read_write> image: array<u32>;

@compute @workgroup_size(8, 8)
fn compute_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let width: u32 = 256;
    let height: u32 = 256;

    if (id.x >= width || id.y >= height) {
        return;
    }

    let i = id.y * width + id.x;

    // Gradient: red = x, green = y, blue = 128, alpha = 255
    let r: u32 = id.x * 255u / width;
    let g: u32 = id.y * 255u / height;
    let b: u32 = 128u;
    let a: u32 = 255u;

    image[i] = (a << 24u) | (b << 16u) | (g << 8u) | r;
}
