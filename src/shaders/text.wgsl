// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// MSDF Text Rendering Shader
// Based on Viktor Chlumsky's "Shape Decomposition for Multi-channel Distance Fields"
//
// Key insight: The median of the three color channels preserves sharp corners
// by using edge coloring where adjacent edges at corners have different colors.

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

// Compute median of three values - the core of MSDF reconstruction
// This preserves sharp corners by selecting the channel that represents
// the actual edge at corners where edge coloring differs
fn median(a: f32, b: f32, c: f32) -> f32 {
    return max(min(a, b), min(max(a, b), c));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample all three channels of the MSDF texture
    let msdf = textureSample(font_texture, font_sampler, in.tex_coords);

    // Extract SDF parameters
    let sdf_scale = in.sdf_params.x;
    let edge_threshold = in.sdf_params.y;
    let debug_quad = in.sdf_params.w;

    // Compute the median of the three channels
    // This is the key operation that reconstructs the shape with sharp corners
    // The median selects the correct distance at corners where edge colors differ
    let median_value = median(msdf.r, msdf.g, msdf.b);

    // Convert from 0-1 range (where 0.5 is the edge) to signed distance
    // Distance is positive inside the glyph, negative outside
    let distance = (median_value - 0.5) * sdf_scale;

    // Improved antialiasing with adaptive edge width using screen-space derivatives
    // fwidth gives us the rate of change of the distance across the pixel
    let edge_width = max(length(vec2<f32>(dpdx(distance), dpdy(distance))), 0.1);
    let smoothing = edge_width * 1.5;
    let alpha = smoothstep(-smoothing, smoothing, distance - edge_threshold);

    // Apply color and alpha
    var final_color = in.color;
    final_color.a *= alpha;

    // Debug mode: show quad outlines and raw MSDF values
    if (debug_quad > 0.5) {
        let uv = in.tex_coords;
        let line_width = 0.15;

        // Check if we're near any edge of the quad
        let near_left = uv.x < line_width;
        let near_right = uv.x > (1.0 - line_width);
        let near_top = uv.y < line_width;
        let near_bottom = uv.y > (1.0 - line_width);

        // Show outline
        if (near_left || near_right || near_top || near_bottom) {
            return vec4<f32>(1.0, 0.0, 0.0, 1.0);
        }

        // Show the raw MSDF colors for debugging
        return vec4<f32>(msdf.r, msdf.g, msdf.b, 1.0);
    }

    // Discard fully transparent pixels
    if (final_color.a < 0.001) {
        discard;
    }

    return final_color;
}
