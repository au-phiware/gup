// Optimized vertex shader for filled polygon mark rendering
// Uses instanced rendering where each instance is a tessellated triangle

struct TriangleInstance {
    v0: vec2<f32>,
    v1: vec2<f32>,
    v2: vec2<f32>,
    _pad: vec2<f32>,
    color0: vec4<f32>,
    color1: vec4<f32>,
    color2: vec4<f32>,
}

struct ViewportTransform {
    scale_x: f32,
    scale_y: f32,
    translate_x: f32,
    translate_y: f32,
}

@group(0) @binding(0) var<storage, read> instances: array<TriangleInstance>;

@group(1) @binding(0) var<uniform> vp_transform: ViewportTransform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];

    // Barycentric interpolation: input.position encodes weights.
    // Vertex 0 = (0,0), Vertex 1 = (1,0), Vertex 2 = (0,1).
    let w0 = 1.0 - input.position.x - input.position.y;
    let w1 = input.position.x;
    let w2 = input.position.y;

    let pos = instance.v0 * w0 + instance.v1 * w1 + instance.v2 * w2;
    let color = instance.color0 * w0 + instance.color1 * w1 + instance.color2 * w2;

    // Apply viewport transform.
    let transformed = vec2<f32>(
        pos.x * vp_transform.scale_x + vp_transform.translate_x,
        pos.y * vp_transform.scale_y + vp_transform.translate_y,
    );

    var output: VertexOutput;
    output.clip_position = vec4<f32>(transformed, 0.0, 1.0);
    output.color = color;

    return output;
}
