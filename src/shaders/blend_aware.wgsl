// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// Blend-aware shader with global alpha support

struct GlobalAlpha {
    alpha: f32,
}

@group(0) @binding(0)
var<uniform> global_alpha: GlobalAlpha;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = color;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = in.color;
    color.a *= global_alpha.alpha;
    return color;
}