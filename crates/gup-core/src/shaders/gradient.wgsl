// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A colour-legend bar: a rectangle filled through the same
// `gup::color::sequential::map` and palette LUT that colour the data, with
// the normalised value running from 0 at one end to 1 at the other.
#import gup::view::{View, px_to_clip}
#import gup::color::sequential as sequential

@group(0) @binding(0) var<uniform> u_view: View;
@group(1) @binding(0) var lut: texture_2d<f32>;
@group(1) @binding(1) var lut_smp: sampler;

struct GradientIn {
    @location(0) lo: vec2<f32>,
    @location(1) hi: vec2<f32>,
    // Non-zero: the value runs bottom (0) to top (1); else left to right.
    @location(2) vertical: u32,
    // Non-zero to run the palette backwards (the scale's `reversed`).
    @location(3) reverse: u32,
}

struct GradientOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) t: f32,
    @location(1) @interpolate(flat) reverse: u32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, g: GradientIn) -> GradientOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[vertex_index];
    var out: GradientOut;
    out.clip = px_to_clip(mix(g.lo, g.hi, c), u_view);
    out.t = select(c.x, 1.0 - c.y, g.vertical != 0u);
    out.reverse = g.reverse;
    return out;
}

@fragment
fn fs_main(v: GradientOut) -> @location(0) vec4<f32> {
    var p: sequential::Params;
    p.lo = 0.0;
    p.inv_span = 1.0;
    p.reverse = v.reverse;
    let c = sequential::map(v.t, p, lut, lut_smp);
    return vec4<f32>(c.rgb * c.a, c.a);
}
