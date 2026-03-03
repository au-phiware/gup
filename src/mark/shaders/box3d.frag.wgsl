// Box3D fragment shader with Phong lighting.

struct LightUniform {
    direction: vec3<f32>,
    _pad:      f32,
    color:     vec3<f32>,
    intensity: f32,
}

struct CameraUniform {
    view:       mat4x4<f32>,
    projection: mat4x4<f32>,
    model:      mat4x4<f32>,
}

@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<uniform> light:  LightUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal:        vec3<f32>,
    @location(1) world_position:      vec3<f32>,
    @location(2) color:               vec4<f32>,
    @location(3) mat_albedo_ambient:  vec4<f32>,
    @location(4) mat_dss:             vec4<f32>,
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.world_normal);

    let albedo    = input.mat_albedo_ambient.xyz * input.color.rgb;
    let ambient_k = input.mat_albedo_ambient.w;
    let diffuse_k = input.mat_dss.x;
    let specular_k = input.mat_dss.y;
    let shininess  = input.mat_dss.z;

    let light_dir = normalize(light.direction);

    // Approximate camera position from inverse view matrix.
    let cam_pos = vec3<f32>(
        camera.view[3][0],
        camera.view[3][1],
        camera.view[3][2],
    );
    // Because the view matrix is column-major and translation is stored
    // differently, we extract the camera world position as:
    // -transpose(R) * t  →  but for the view_dir we just need the direction.
    let view_dir = normalize(-input.world_position);

    // Phong shading
    let ambient_color  = albedo * ambient_k;
    let n_dot_l        = max(dot(normal, light_dir), 0.0);
    let diffuse_color  = albedo * diffuse_k * n_dot_l;
    let reflect_dir    = reflect(-light_dir, normal);
    let spec_angle     = max(dot(view_dir, reflect_dir), 0.0);
    let specular_color = light.color * specular_k * pow(spec_angle, shininess);

    let final_rgb = (ambient_color + (diffuse_color + specular_color) * light.intensity) * light.color;

    return vec4<f32>(final_rgb, input.color.a);
}
