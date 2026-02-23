// Vertex shader for path mark rendering
// Transforms path vertices and passes attributes to fragment shader

struct PathInstance {
    transform: mat4x4<f32>,
    fill_color: vec4<f32>,
    stroke_color: vec4<f32>,
    stroke_width: f32,
}

@group(0) @binding(0)
var<storage, read> instances: array<PathInstance>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let instance = instances[instance_index];
    var output: VertexOutput;
    
    // Transform path vertex to world space
    let world_pos_4d = instance.transform * vec4<f32>(position, 0.0, 1.0);
    let world_pos_2d = world_pos_4d.xy;
    
    output.position = world_pos_4d;
    output.world_position = world_pos_2d;
    output.tex_coords = tex_coords;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.stroke_width = instance.stroke_width;
    
    return output;
}
