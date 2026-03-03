// Sphere3D billboard vertex shader.
// Each instance is a camera-facing quad positioned and scaled by the
// view/projection matrices and the per-instance position + radius.

struct CameraUniform {
    view:       mat4x4<f32>,
    projection: mat4x4<f32>,
    model:      mat4x4<f32>,
}

struct Sphere3DInstance {
    position:                 vec3<f32>,
    radius:                   f32,
    color:                    vec4<f32>,
    material_albedo_ambient:  vec4<f32>,
    material_dss:             vec4<f32>,   // diffuse, specular, shininess, _pad
}

@group(0) @binding(0) var<storage, read> instances: array<Sphere3DInstance>;
@group(1) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,           // unit-quad corner
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos:           vec2<f32>,  // [-1,1] on the quad
    @location(1) color:               vec4<f32>,
    @location(2) world_center:        vec3<f32>,
    @location(3) radius:              f32,
    @location(4) mat_albedo_ambient:  vec4<f32>,
    @location(5) mat_dss:             vec4<f32>,
    @location(6) view_center:         vec3<f32>,  // center in view space
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let inst = instances[input.instance_index];

    // Transform sphere centre to view space.
    let model_pos  = camera.model * vec4<f32>(inst.position, 1.0);
    let view_pos   = camera.view  * model_pos;

    // Billboard: offset the quad corners in view-space X/Y by radius,
    // keeping the sphere centre's Z unchanged.
    let offset = vec4<f32>(input.position * inst.radius, 0.0, 0.0);
    let billboard_pos = view_pos + offset;

    var out: VertexOutput;
    out.clip_position       = camera.projection * billboard_pos;
    out.local_pos           = input.position;
    out.color               = inst.color;
    out.world_center        = model_pos.xyz;
    out.radius              = inst.radius;
    out.mat_albedo_ambient  = inst.material_albedo_ambient;
    out.mat_dss             = inst.material_dss;
    out.view_center         = view_pos.xyz;
    return out;
}
