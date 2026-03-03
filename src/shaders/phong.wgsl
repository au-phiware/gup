// Phong and Blinn-Phong lighting functions for 3D mark shaders.
//
// Include this module in any 3D fragment shader that needs lighting.
// The structs below must match the Rust-side Material and LightUniform layouts.

struct Material {
    albedo:    vec3<f32>,
    ambient:   f32,
    diffuse:   f32,
    specular:  f32,
    shininess: f32,
    _pad:      f32,
}

struct LightUniform {
    direction: vec3<f32>,
    _pad:      f32,
    color:     vec3<f32>,
    intensity: f32,
}

/// Classic Phong lighting: ambient + diffuse + specular.
///
/// * `normal`    – unit surface normal (world space).
/// * `view_dir`  – unit vector from surface point toward the camera.
/// * `light_dir` – unit vector from surface point toward the light.
/// * `material`  – surface material properties.
/// * `light`     – directional light properties.
fn phong_lighting(
    normal:    vec3<f32>,
    view_dir:  vec3<f32>,
    light_dir: vec3<f32>,
    material:  Material,
    light:     LightUniform,
) -> vec4<f32> {
    // Ambient
    let ambient_color = material.albedo * material.ambient;

    // Diffuse (Lambert)
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let diffuse_color = material.albedo * material.diffuse * n_dot_l;

    // Specular (Phong reflection)
    let reflect_dir = reflect(-light_dir, normal);
    let spec_angle  = max(dot(view_dir, reflect_dir), 0.0);
    let spec_factor = pow(spec_angle, material.shininess);
    let specular_color = light.color * material.specular * spec_factor;

    let final_rgb = (ambient_color + (diffuse_color + specular_color) * light.intensity) * light.color;
    return vec4<f32>(final_rgb, 1.0);
}

/// Blinn-Phong variant using the half-vector for specular.
fn blinn_phong_lighting(
    normal:    vec3<f32>,
    view_dir:  vec3<f32>,
    light_dir: vec3<f32>,
    material:  Material,
    light:     LightUniform,
) -> vec4<f32> {
    let ambient_color = material.albedo * material.ambient;

    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let diffuse_color = material.albedo * material.diffuse * n_dot_l;

    let half_dir    = normalize(light_dir + view_dir);
    let spec_angle  = max(dot(normal, half_dir), 0.0);
    let spec_factor = pow(spec_angle, material.shininess);
    let specular_color = light.color * material.specular * spec_factor;

    let final_rgb = (ambient_color + (diffuse_color + specular_color) * light.intensity) * light.color;
    return vec4<f32>(final_rgb, 1.0);
}
