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
    let debug_quad = in.sdf_params.w;
    
    // Convert SDF value to distance
    // SDF values: 0-127 = outside (negative distance), 128-255 = inside (positive distance)
    // 128 represents the edge (distance = 0)
    let normalized_sdf = sdf_value; // Already 0-1 from R8Unorm texture
    let distance = (normalized_sdf - 0.5) * sdf_scale;
    
    // Improved antialiasing with adaptive edge width
    let edge_width = max(length(vec2<f32>(dpdx(distance), dpdy(distance))), 0.15);
    let smoothing = edge_width * 2.0; // More aggressive smoothing for better antialiasing
    let alpha = smoothstep(-smoothing, smoothing, distance - edge_threshold);
    
    // Apply color and alpha
    var final_color = in.color;
    final_color.a *= alpha;
    
    // Force debug outline visibility (when debug_quad > 0.5)
    if (debug_quad > 0.5) {
        let uv = in.tex_coords;
        let line_width = 0.2; // Even thicker outline
        
        // Check if we're near any edge of the quad
        let near_left = uv.x < line_width;
        let near_right = uv.x > (1.0 - line_width);
        let near_top = uv.y < line_width;
        let near_bottom = uv.y > (1.0 - line_width);
        
        // ALWAYS show outline if near edge, ignore alpha
        if (near_left || near_right || near_top || near_bottom) {
            return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Force bright red outline
        }
        
        // For interior, show the character color to verify it's working
        return vec4<f32>(in.color.rgb, 1.0); // Force character color with full alpha
    }
    
    // Discard fully transparent pixels (but not if we're drawing debug outline)
    if (final_color.a < 0.001 && debug_quad <= 0.5) {
        discard;
    }
    
    return final_color;
}