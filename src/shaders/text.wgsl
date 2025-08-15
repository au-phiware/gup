// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// SDF Text Rendering Shader

struct Uniforms {
    projection: mat4x4<f32>,
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) sdf_params: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) sdf_params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var font_texture: texture_2d<f32>;
@group(0) @binding(2) var font_sampler: sampler;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform position using projection matrix
    out.clip_position = uniforms.projection * vec4<f32>(vertex.position, 0.0, 1.0);
    
    // Pass through other attributes
    out.tex_coords = vertex.tex_coords;
    out.color = vertex.color;
    out.sdf_params = vertex.sdf_params;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the SDF texture
    let sdf_value = textureSample(font_texture, font_sampler, in.tex_coords).r;
    
    // Extract SDF parameters
    let sdf_scale = in.sdf_params.x;
    let edge_threshold = in.sdf_params.y;
    let outline_width = in.sdf_params.z;
    
    // Calculate distance in world space
    let distance = (sdf_value - 0.5) * sdf_scale;
    
    // Anti-aliased edge
    let edge_width = length(vec2<f32>(dpdx(distance), dpdy(distance)));
    let alpha = smoothstep(-edge_width, edge_width, distance - edge_threshold);
    
    // Apply color and alpha
    var final_color = in.color;
    final_color.a *= alpha;
    
    // Optional outline effect
    if (outline_width > 0.0) {
        let outline_alpha = smoothstep(-edge_width, edge_width, distance + outline_width);
        let outline_factor = outline_alpha - alpha;
        
        if (outline_factor > 0.0) {
            // Mix with outline color (black)
            final_color = mix(final_color, vec4<f32>(0.0, 0.0, 0.0, outline_alpha), outline_factor);
        }
    }
    
    // Discard fully transparent pixels
    if (final_color.a < 0.001) {
        discard;
    }
    
    return final_color;
}