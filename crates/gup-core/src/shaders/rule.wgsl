// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Guide rules (axis lines, tick marks): one instanced quad per segment.
// Endpoints are snapped to pixel centres on the CPU, so axis-aligned
// hairlines are crisp without anti-aliasing.
#import gup::view::{View, px_to_clip}

@group(0) @binding(0) var<uniform> u_view: View;

struct RuleIn {
    @location(0) start: vec2<f32>,
    @location(1) stop: vec2<f32>,
    @location(2) width: f32,
    @location(3) color: vec4<f32>,
}

struct RuleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, r: RuleIn) -> RuleOut {
    var along = array<f32, 6>(0.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    var across = array<f32, 6>(-1.0, -1.0, 1.0, 1.0, -1.0, 1.0);
    let dir = normalize(r.stop - r.start);
    let normal = vec2<f32>(-dir.y, dir.x) * (r.width * 0.5);
    // Extend along the segment by half the width so joins are square.
    let ext = dir * (r.width * 0.5);
    let a = r.start - ext;
    let b = r.stop + ext;
    let p = mix(a, b, along[vertex_index]) + normal * across[vertex_index];
    var out: RuleOut;
    out.clip = px_to_clip(p, u_view);
    out.color = vec4<f32>(r.color.rgb * r.color.a, r.color.a);
    return out;
}

@fragment
fn fs_main(v: RuleOut) -> @location(0) vec4<f32> {
    return v.color;
}
