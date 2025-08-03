// Optimized vertex shader for rectangle mark rendering
// Uses instanced rendering with unit quad geometry

struct RectangleInstance {
    center: vec2<f32>,
    size: vec2<f32>,
    fill_color: vec4<f32>,
    stroke_width: f32,
    stroke_color: vec4<f32>,
    corner_radius: f32,
    _padding: f32,
}

@group(0) @binding(0) var<storage, read> instances: array<RectangleInstance>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) size: vec2<f32>,
    @location(5) stroke_width: f32,
    @location(6) corner_radius: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    
    // Calculate world position by scaling unit quad and translating to center
    let world_pos = input.position * instance.size + instance.center;
    
    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.size = instance.size;
    output.stroke_width = instance.stroke_width;
    output.corner_radius = instance.corner_radius;
    
    return output;
}