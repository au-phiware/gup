// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Flat-coloured rectangles (plot background, legend swatches): one
// instanced quad each, in logical pixels. No analytic antialiasing: edges
// on whole pixels are exact, and MSAA smooths the rest.
#import gup::view::{View, px_to_clip}

@group(0) @binding(0) var<uniform> u_view: View;

struct RectIn {
    @location(0) lo: vec2<f32>,
    @location(1) hi: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct RectOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, r: RectIn) -> RectOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    var out: RectOut;
    out.clip = px_to_clip(mix(r.lo, r.hi, corners[vertex_index]), u_view);
    out.color = vec4<f32>(r.color.rgb * r.color.a, r.color.a);
    return out;
}

@fragment
fn fs_main(v: RectOut) -> @location(0) vec4<f32> {
    return v.color;
}
