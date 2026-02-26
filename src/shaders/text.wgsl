// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// MSDF Text Rendering Shader
// Based on Viktor Chlumsky's "Shape Decomposition for Multi-channel Distance Fields"
//
// Key insight: The median of the three color channels preserves sharp corners
// by using edge coloring where adjacent edges at corners have different colors.
//
// Channel combination modes (sdf_params.w fractional):
//   0 = median(r,g,b)   — default MSDF, sharp corners preserved
//   1 = max(r,g,b)       — union, slightly dilated outline
//   2 = min(r,g,b)       — intersection, sharper corners
//
// Debug modes (sdf_params.w integer part):
//   0.0 = normal rendering
//   1.0 = quad outlines + raw MSDF colours
//   2.0 = red channel only
//   3.0 = green channel only
//   4.0 = blue channel only
//   5.0 = reconstructed median as grayscale

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

// Combine three MSDF channels into a single distance value.
// The combination mode controls the visual trade-off between
// corner sharpness and outline accuracy.
fn combine_sdf_channels(r: f32, g: f32, b: f32, mode: u32) -> f32 {
    switch mode {
        // Median: standard MSDF reconstruction.
        // At smooth edges all channels agree so median = any channel.
        // At sharp corners the channels differ and the median picks
        // the correct distance for that region of the plane.
        case 0u: {
            return median(r, g, b);
        }
        // Max (union): the outermost distance of any channel.
        // Produces a slightly dilated outline; useful for bold effects.
        case 1u: {
            return max(max(r, g), b);
        }
        // Min (intersection): the innermost distance of any channel.
        // Produces maximally sharp corners at the cost of slightly
        // thinner strokes.
        case 2u: {
            return min(min(r, g), b);
        }
        default: {
            return median(r, g, b);
        }
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample all three channels of the MSDF texture
    let msdf = textureSample(font_texture, font_sampler, in.tex_coords);

    // Extract SDF parameters
    let sdf_scale = in.sdf_params.x;
    let edge_threshold = in.sdf_params.y;
    // sdf_params.z = smoothing factor (0 means use default 1.5)
    let smoothing_factor_raw = in.sdf_params.z;
    let smoothing_factor = select(smoothing_factor_raw, 1.5, smoothing_factor_raw <= 0.0);
    // sdf_params.w packs debug_mode (integer part) and combination_mode (fractional × 10)
    let debug_mode = floor(in.sdf_params.w);
    let combination_mode = u32(round(fract(in.sdf_params.w) * 10.0));

    // Combine channels using the selected mode
    let combined_value = combine_sdf_channels(msdf.r, msdf.g, msdf.b, combination_mode);

    // Convert from 0-1 range (where 0.5 is the edge) to signed distance
    // Distance is positive inside the glyph, negative outside
    let distance = (combined_value - 0.5) * sdf_scale;

    // Improved antialiasing with adaptive edge width using screen-space derivatives
    // fwidth gives us the rate of change of the distance across the pixel
    let edge_width = max(length(vec2<f32>(dpdx(distance), dpdy(distance))), 0.1);
    let smoothing = edge_width * smoothing_factor;
    let alpha = smoothstep(-smoothing, smoothing, distance - edge_threshold);

    // Apply color and alpha
    var final_color = in.color;
    final_color.a *= alpha;

    // Debug modes for inspecting MSDF channel data
    if (debug_mode > 0.5) {
        let uv = in.tex_coords;

        // Mode 1: quad outlines + raw MSDF colours
        if (debug_mode < 1.5) {
            let line_width = 0.15;
            let near_left = uv.x < line_width;
            let near_right = uv.x > (1.0 - line_width);
            let near_top = uv.y < line_width;
            let near_bottom = uv.y > (1.0 - line_width);

            if (near_left || near_right || near_top || near_bottom) {
                return vec4<f32>(1.0, 0.0, 0.0, 1.0);
            }
            return vec4<f32>(msdf.r, msdf.g, msdf.b, 1.0);
        }

        // Mode 2: red channel only
        if (debug_mode < 2.5) {
            return vec4<f32>(msdf.r, msdf.r, msdf.r, 1.0);
        }

        // Mode 3: green channel only
        if (debug_mode < 3.5) {
            return vec4<f32>(msdf.g, msdf.g, msdf.g, 1.0);
        }

        // Mode 4: blue channel only
        if (debug_mode < 4.5) {
            return vec4<f32>(msdf.b, msdf.b, msdf.b, 1.0);
        }

        // Mode 5: reconstructed median as grayscale
        if (debug_mode < 5.5) {
            let m = median(msdf.r, msdf.g, msdf.b);
            return vec4<f32>(m, m, m, 1.0);
        }
    }

    // Discard fully transparent pixels
    if (final_color.a < 0.001) {
        discard;
    }

    return final_color;
}
