// Optimized vertex shader for circle mark rendering
// Uses instanced rendering with unit quad geometry

struct CircleInstance {
    center: vec2<f32>,
    radius: f32,
    fill_color: vec4<f32>,
    stroke_width: f32,
    stroke_color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<CircleInstance>;

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
    @location(4) radius: f32,
    @location(5) stroke_width: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    
    // Calculate world position by scaling unit quad and translating to center
    let world_pos = input.position * instance.radius + instance.center;
    
    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.radius = instance.radius;
    output.stroke_width = instance.stroke_width;
    
    return output;
}