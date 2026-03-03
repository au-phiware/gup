// Sphere3D fragment shader with SDF sphere evaluation and Phong lighting.
//
// The billboard quad passes local_pos in [-1,1]. We discard fragments
// outside the unit circle and compute a per-fragment normal from the
// implicit sphere surface. Depth is reconstructed to match the actual
// sphere geometry so that spheres occlude correctly.

struct CameraUniform {
    view:       mat4x4<f32>,
    projection: mat4x4<f32>,
    model:      mat4x4<f32>,
}

struct LightUniform {
    direction: vec3<f32>,
    _pad:      f32,
    color:     vec3<f32>,
    intensity: f32,
}

@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<uniform> light:  LightUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos:           vec2<f32>,
    @location(1) color:               vec4<f32>,
    @location(2) world_center:        vec3<f32>,
    @location(3) radius:              f32,
    @location(4) mat_albedo_ambient:  vec4<f32>,
    @location(5) mat_dss:             vec4<f32>,
    @location(6) view_center:         vec3<f32>,
}

struct FragOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_main(input: VertexOutput) -> FragOutput {
    // SDF: check whether this fragment is inside the sphere silhouette.
    let dist_sq = dot(input.local_pos, input.local_pos);
    if (dist_sq > 1.0) {
        discard;
    }

    // Compute view-space normal from the implicit sphere.
    // The Z component comes from the sphere equation: z = sqrt(1 - x² - y²).
    let nz = sqrt(1.0 - dist_sq);
    let normal_view = vec3<f32>(input.local_pos.x, input.local_pos.y, nz);

    // Reconstruct view-space position on the actual sphere surface.
    let sphere_view_pos = input.view_center + normal_view * input.radius;

    // Project to get correct depth.
    let clip_pos = camera.projection * vec4<f32>(sphere_view_pos, 1.0);
    let ndc_depth = clip_pos.z / clip_pos.w;

    // World-space normal for lighting (transform view-space normal by
    // inverse-transpose of the view matrix upper-3x3, which for an
    // orthonormal basis is just the transpose → columns become rows).
    let inv_view_3 = mat3x3<f32>(
        camera.view[0].xyz,
        camera.view[1].xyz,
        camera.view[2].xyz,
    );
    // For an orthonormal view matrix: (V^{-1})^T = V, so world normal =
    // transpose(mat3(V)) * normal_view.
    let normal_world = normalize(
        vec3<f32>(
            dot(inv_view_3[0], normal_view),
            dot(inv_view_3[1], normal_view),
            dot(inv_view_3[2], normal_view),
        )
    );

    // Material properties.
    let albedo    = input.mat_albedo_ambient.xyz * input.color.rgb;
    let ambient_k = input.mat_albedo_ambient.w;
    let diffuse_k = input.mat_dss.x;
    let specular_k = input.mat_dss.y;
    let shininess  = input.mat_dss.z;

    // Phong lighting.
    let light_dir = normalize(light.direction);
    // Approximate view direction — camera looks down -Z in view space.
    let view_dir  = normalize(-sphere_view_pos);
    // Transform view_dir to world for consistency with normal_world.
    let view_dir_world = normalize(
        vec3<f32>(
            dot(inv_view_3[0], view_dir),
            dot(inv_view_3[1], view_dir),
            dot(inv_view_3[2], view_dir),
        )
    );

    let ambient_color  = albedo * ambient_k;
    let n_dot_l        = max(dot(normal_world, light_dir), 0.0);
    let diffuse_color  = albedo * diffuse_k * n_dot_l;
    let reflect_dir    = reflect(-light_dir, normal_world);
    let spec_angle     = max(dot(view_dir_world, reflect_dir), 0.0);
    let specular_color = light.color * specular_k * pow(spec_angle, shininess);

    let final_rgb = (ambient_color + (diffuse_color + specular_color) * light.intensity) * light.color;

    var out: FragOutput;
    out.color = vec4<f32>(final_rgb, input.color.a);
    out.depth = ndc_depth;
    return out;
}
