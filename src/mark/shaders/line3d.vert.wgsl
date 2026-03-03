// Line3D vertex shader.
// Renders a camera-facing quad between two 3D endpoints.

struct CameraUniform {
    view:       mat4x4<f32>,
    projection: mat4x4<f32>,
    model:      mat4x4<f32>,
}

struct Line3DInstance {
    start: vec3<f32>,
    width: f32,
    end:   vec3<f32>,
    _pad:  f32,
    color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<Line3DInstance>;
@group(1) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) selector: vec2<f32>,          // (endpoint, side)
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let inst = instances[input.instance_index];

    // Choose the endpoint.
    let world_pos = mix(inst.start, inst.end, vec3<f32>(input.selector.x));

    // Transform to clip space.
    let model_pos = (camera.model * vec4<f32>(world_pos, 1.0)).xyz;
    let view_pos  = camera.view * vec4<f32>(model_pos, 1.0);

    // Line direction in view space.
    let start_view = (camera.view * camera.model * vec4<f32>(inst.start, 1.0)).xyz;
    let end_view   = (camera.view * camera.model * vec4<f32>(inst.end,   1.0)).xyz;
    let dir_view   = normalize(end_view - start_view);

    // Perpendicular direction in the view-space XY plane (camera-facing).
    let perp = normalize(vec2<f32>(-dir_view.y, dir_view.x));

    // Offset the vertex sideways in view space.
    let offset_view = vec4<f32>(perp * input.selector.y * inst.width, 0.0, 0.0);
    let final_view  = view_pos + offset_view;

    var out: VertexOutput;
    out.clip_position = camera.projection * final_view;
    out.color         = inst.color;
    return out;
}
