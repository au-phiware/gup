// Optimized vertex shader for line mark rendering
// Uses instanced rendering with oriented quad geometry

struct LineInstance {
    start: vec2<f32>,
    end: vec2<f32>,
    color: vec4<f32>,
    width: f32,
    style: u32,
    _padding: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<LineInstance>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) normal: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) width: f32,
    @location(4) style: u32,
    @location(5) line_length: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    
    // Calculate line direction and length
    let line_vec = instance.end - instance.start;
    let line_length = length(line_vec);
    let line_dir = normalize(line_vec);
    let line_normal = vec2<f32>(-line_dir.y, line_dir.x);
    
    // Calculate world position along the line
    let along_line = instance.start + line_dir * (input.position.x * line_length);
    let across_line = line_normal * (input.normal.y * instance.width * 0.5);
    let world_pos = along_line + across_line;
    
    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.color = instance.color;
    output.width = instance.width;
    output.style = instance.style;
    output.line_length = line_length;
    
    return output;
}