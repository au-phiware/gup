// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Guide rules (axis lines, tick marks, grid): one instanced quad per
// segment, with square caps (the segment extends by half its width at both
// ends) and analytic edge coverage from `gup::coverage`. Guides snap
// hairlines to pixel centres on the CPU, which puts their edges on pixel
// boundaries: they stay exactly as crisp as without antialiasing, while
// diagonal rules and fractional edges ramp over one physical pixel.
#import gup::view::{View, px_to_clip}
#import gup::coverage::{line_quad, box_coverage}

@group(0) @binding(0) var<uniform> u_view: View;

struct RuleIn {
    @location(0) start: vec2<f32>,
    @location(1) stop: vec2<f32>,
    @location(2) width: f32,
    @location(3) color: vec4<f32>,
}

struct RuleOut {
    @builtin(position) clip: vec4<f32>,
    // Premultiplied.
    @location(0) color: vec4<f32>,
    // Offset from the rule's centre (along, across), physical pixels.
    @location(1) local: vec2<f32>,
    // Half length (caps included) and half width, physical pixels.
    @location(2) @interpolate(flat) half: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, r: RuleIn) -> RuleOut {
    let q = line_quad(r.start, r.stop, r.width, r.width * 0.5, vertex_index, u_view.dpr);
    var out: RuleOut;
    out.clip = px_to_clip(q.position, u_view);
    out.color = vec4<f32>(r.color.rgb * r.color.a, r.color.a);
    out.local = q.local;
    out.half = q.half;
    return out;
}

@fragment
fn fs_main(v: RuleOut) -> @location(0) vec4<f32> {
    let coverage = box_coverage(v.local, v.half);
    if (coverage <= 0.0) {
        discard;
    }
    return v.color * coverage;
}
