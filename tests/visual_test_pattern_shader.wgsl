// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Simple test shader that renders a full-screen quad with pattern overlay

struct PatternUniforms {
    pattern_type: u32,
    spacing: f32,
    angle: f32,
    thickness: f32,
    foreground_color: vec4<f32>,
    background_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> pattern: PatternUniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    // Convert from [-1, 1] to [0, 1] for UV coordinates
    output.uv = (input.position + 1.0) * 0.5;
    // Flip Y to match image coordinates
    output.uv.y = 1.0 - output.uv.y;
    return output;
}

// Pattern functions (from patterns.wgsl)

fn pattern_solid(uv: vec2<f32>) -> vec4<f32> {
    return pattern.foreground_color;
}

fn pattern_dots(uv: vec2<f32>) -> vec4<f32> {
    let grid_pos = uv / pattern.spacing;
    let cell = fract(grid_pos);
    let center = vec2<f32>(0.5, 0.5);
    let dist = length(cell - center);
    let radius = 0.3;
    
    if (dist < radius) {
        return pattern.foreground_color;
    } else {
        return pattern.background_color;
    }
}

fn pattern_lines(uv: vec2<f32>) -> vec4<f32> {
    let cos_angle = cos(pattern.angle);
    let sin_angle = sin(pattern.angle);
    let rotated = uv.x * cos_angle + uv.y * sin_angle;
    let pos = rotated % pattern.spacing;
    let half_thickness = pattern.thickness * 0.5;
    
    if (pos < half_thickness || pos > pattern.spacing - half_thickness) {
        return pattern.foreground_color;
    } else {
        return pattern.background_color;
    }
}

fn pattern_crosshatch(uv: vec2<f32>) -> vec4<f32> {
    // Horizontal lines
    let h_pos = uv.y % pattern.spacing;
    let half_thickness = pattern.thickness * 0.5;
    let is_h_line = h_pos < half_thickness || h_pos > pattern.spacing - half_thickness;
    
    // Vertical lines
    let v_pos = uv.x % pattern.spacing;
    let is_v_line = v_pos < half_thickness || v_pos > pattern.spacing - half_thickness;
    
    if (is_h_line || is_v_line) {
        return pattern.foreground_color;
    } else {
        return pattern.background_color;
    }
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV to pixel coordinates for pattern functions
    let pixel_uv = input.uv * vec2<f32>(800.0, 600.0);
    
    switch (pattern.pattern_type) {
        case 0u: { return pattern_solid(pixel_uv); }
        case 1u: { return pattern_dots(pixel_uv); }
        case 2u: { return pattern_lines(pixel_uv); }
        case 3u: { return pattern_crosshatch(pixel_uv); }
        default: { return pattern.background_color; }
    }
}
