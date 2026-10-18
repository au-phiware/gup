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
    fill_opacity: f32,
    stroke: vec4<f32>,
    stroke_width: f32,
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
    // Straight alpha, stroke inside the edge, width in physical pixels.
    @location(3) stroke: vec4<f32>,
    @location(4) stroke_width: f32,
    @location(5) @interpolate(flat) row: u32,
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
    // One extra pixel for the anti-aliased edge, and the pick margin.
    let half = m.radius + 1.0 / view.dpr + view.pick_margin;
    let offset = corner(vertex_index) * half;
    var out: Varyings;
    out.clip = px_to_clip(vec2<f32>(m.x, m.y) + offset, view);
    out.local = offset * view.dpr;
    out.radius = m.radius * view.dpr;
    out.fill = vec4<f32>(m.fill.rgb, m.fill.a * m.fill_opacity);
    out.stroke = m.stroke;
    out.stroke_width = m.stroke_width * view.dpr;
    out.row = row;
    return out;
}

// `out = S * ring + F * outer * (1 - S.a * ring)`, premultiplied, with
// `F` the fill and `S` the stroke. Discards a fully transparent fragment.
fn compose(fill: vec4<f32>, stroke: vec4<f32>, outer: f32, ring: f32) -> vec4<f32> {
    let f = vec4<f32>(fill.rgb * fill.a, fill.a);
    let s = vec4<f32>(stroke.rgb * stroke.a, stroke.a);
    let out = s * ring + f * outer * (1.0 - s.a * ring);
    if (out.a <= 0.0) {
        discard;
    }
    return out;
}

// Analytic coverage of a disc with a one-physical-pixel ramp, and of the disc
// shrunk by the stroke width; the ring between them is the stroke. Fill lies
// under the stroke. Premultiplied.
fn shade(v: Varyings) -> vec4<f32> {
    let d = length(v.local);
    let outer = clamp(0.5 - (d - v.radius), 0.0, 1.0);
    let inner = clamp(0.5 - (d - max(v.radius - v.stroke_width, 0.0)), 0.0, 1.0);
    return compose(v.fill, v.stroke, outer, outer - inner);
}

// The signed distance from the fragment to the disc, in physical pixels
// (logical in a pick pass, which draws at dpr 1): negative inside. A
// negative radius is never hit (RFC-001 S9d).
fn hit_distance(v: Varyings) -> f32 {
    return select(length(v.local) - v.radius, 1e30, v.radius < 0.0);
}

// The row a pick of this disc reports.
fn hit_row(v: Varyings) -> u32 {
    return v.row;
}
