// Box3D vertex shader.
// Transforms unit-cube vertices by per-instance center/half_extents,
// then applies camera view/projection.

struct CameraUniform {
    view:       mat4x4<f32>,
    projection: mat4x4<f32>,
    model:      mat4x4<f32>,
}

struct Box3DInstance {
    center:                   vec3<f32>,
    _pad0:                    f32,
    half_extents:             vec3<f32>,
    _pad1:                    f32,
    color:                    vec4<f32>,
    material_albedo_ambient:  vec4<f32>,
    material_dss:             vec4<f32>,
}

@group(0) @binding(0) var<storage, read> instances: array<Box3DInstance>;
@group(1) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal:        vec3<f32>,
    @location(1) world_position:      vec3<f32>,
    @location(2) color:               vec4<f32>,
    @location(3) mat_albedo_ambient:  vec4<f32>,
    @location(4) mat_dss:             vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let inst = instances[input.instance_index];

    // Scale the unit cube by half_extents and translate to center.
    let local_pos = input.position * inst.half_extents + inst.center;

    // Apply model + view + projection.
    let world_pos = (camera.model * vec4<f32>(local_pos, 1.0)).xyz;
    let clip_pos  = camera.projection * camera.view * vec4<f32>(world_pos, 1.0);

    // Normal: scale only by sign (axis-aligned box, no non-uniform stretch
    // correction needed since we scale positions, not normals here).
    let world_normal = normalize((camera.model * vec4<f32>(input.normal, 0.0)).xyz);

    var out: VertexOutput;
    out.clip_position       = clip_pos;
    out.world_normal        = world_normal;
    out.world_position      = world_pos;
    out.color               = inst.color;
    out.mat_albedo_ambient  = inst.material_albedo_ambient;
    out.mat_dss             = inst.material_dss;
    return out;
}
