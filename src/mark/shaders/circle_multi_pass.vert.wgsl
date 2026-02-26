// Multi-pass vertex shader for circle mark rendering
// Supports both main and shadow passes with configurable offset
//
// This shader shares the same instance data as the standard circle shader,
// but provides two vertex entry points:
//   - vs_main:   Renders circles at their original position
//   - vs_shadow: Renders circles offset by a fixed amount (for drop shadow)

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

// Main pass: render circles at their original position
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
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

// Shadow pass: render circles with a fixed offset and expanded radius
@vertex
fn vs_shadow(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];

    // Offset the shadow down-right in clip space
    let shadow_offset = vec2<f32>(0.015, -0.015);
    // Make shadow slightly larger than the main circle
    let shadow_radius = instance.radius * 1.15;
    let world_pos = input.position * shadow_radius + instance.center + shadow_offset;

    // Semi-transparent dark shadow colour
    let shadow_color = vec4<f32>(0.0, 0.0, 0.0, 0.35);

    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.fill_color = shadow_color;
    output.stroke_color = shadow_color;
    output.radius = shadow_radius;
    output.stroke_width = 0.0;

    return output;
}
