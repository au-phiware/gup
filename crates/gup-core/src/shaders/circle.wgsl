// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The Circle mark. Works entirely in logical pixels: the quad is expanded by
// the radius in pixel space and only then converted to clip space, so
// circles stay round on any aspect ratio.
#define_import_path gup::marks::circle

#import gup::view::{View, px_to_clip}

// One field per channel, in `Circle::CHANNELS` order.
struct CircleIn {
    x: f32,
    y: f32,
    radius: f32,
    fill: vec4<f32>,
}

struct Varyings {
    // The mark contract names the position `clip`: the glue overrides it to
    // hide rows with a null position or size (RFC-001 S4b).
    @builtin(position) clip: vec4<f32>,
    // Offset from the centre, in physical pixels.
    @location(0) local: vec2<f32>,
    // Radius, in physical pixels.
    @location(1) radius: f32,
    // sRGB-encoded, straight alpha.
    @location(2) fill: vec4<f32>,
    @location(3) @interpolate(flat) row: u32,
}

// Corner of the instance quad for vertex 0..5 (two triangles).
fn corner(vertex_index: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    return corners[vertex_index];
}

fn vertex(m: CircleIn, vertex_index: u32, row: u32, view: View) -> Varyings {
    // One extra pixel for the anti-aliased edge.
    let half = m.radius + 1.0 / view.dpr;
    let offset = corner(vertex_index) * half;
    var out: Varyings;
    out.clip = px_to_clip(vec2<f32>(m.x, m.y) + offset, view);
    out.local = offset * view.dpr;
    out.radius = m.radius * view.dpr;
    out.fill = m.fill;
    out.row = row;
    return out;
}

// Analytic coverage of a disc with a one-physical-pixel ramp; premultiplied.
fn shade(v: Varyings) -> vec4<f32> {
    let distance = length(v.local) - v.radius;
    let coverage = clamp(0.5 - distance, 0.0, 1.0);
    let alpha = v.fill.a * coverage;
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(v.fill.rgb * alpha, alpha);
}
