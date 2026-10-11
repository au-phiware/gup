// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Flat-coloured rectangles (plot background, legend swatches, bands): one
// instanced quad each, in logical pixels, with analytic edge coverage from
// `gup::coverage`. Edges on whole pixels are exact (no new blur); edges at
// fractional coordinates ramp over one physical pixel.
#import gup::view::{View, px_to_clip}
#import gup::coverage::{rect_quad, box_coverage}

@group(0) @binding(0) var<uniform> u_view: View;

struct RectIn {
    @location(0) lo: vec2<f32>,
    @location(1) hi: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct RectOut {
    @builtin(position) clip: vec4<f32>,
    // Premultiplied.
    @location(0) color: vec4<f32>,
    // Offset from the rect's centre, physical pixels.
    @location(1) local: vec2<f32>,
    // Half the rect's size, physical pixels.
    @location(2) @interpolate(flat) half: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, r: RectIn) -> RectOut {
    let q = rect_quad(r.lo, r.hi, vertex_index, u_view.dpr);
    var out: RectOut;
    out.clip = px_to_clip(q.position, u_view);
    out.color = vec4<f32>(r.color.rgb * r.color.a, r.color.a);
    out.local = q.local;
    out.half = q.half;
    return out;
}

@fragment
fn fs_main(v: RectOut) -> @location(0) vec4<f32> {
    let coverage = box_coverage(v.local, v.half);
    if (coverage <= 0.0) {
        discard;
    }
    return v.color * coverage;
}
