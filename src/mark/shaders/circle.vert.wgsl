// Optimized vertex shader for circle mark rendering
// Uses instanced rendering with unit quad geometry

struct CircleInstance {
    center: vec2<f32>,
    radius: f32,
    fill_color: vec4<f32>,
    stroke_width: f32,
    stroke_color: vec4<f32>,
}

struct ViewportTransform {
    scale_x: f32,
    scale_y: f32,
    translate_x: f32,
    translate_y: f32,
}

@group(0) @binding(0) var<storage, read> instances: array<CircleInstance>;

@group(1) @binding(0) var<uniform> vp_transform: ViewportTransform;

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
    
    // Scale the radius by the viewport transform scale so that circles
    // grow/shrink proportionally during zoom.
    let scaled_radius = instance.radius * vp_transform.scale_x;

    // Calculate world position by scaling unit quad and translating to center,
    // then apply the viewport transform (zoom + pan) in clip space.
    let center_transformed = vec2<f32>(
        instance.center.x * vp_transform.scale_x + vp_transform.translate_x,
        instance.center.y * vp_transform.scale_y + vp_transform.translate_y,
    );
    let world_pos = input.position * scaled_radius + center_transformed;
    
    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.radius = scaled_radius;
    output.stroke_width = instance.stroke_width;
    
    return output;
}