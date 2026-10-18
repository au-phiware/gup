// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The Segment mark: a straight line from (`x`, `y`) to (`x_end`, `y_end`) in
// logical pixels, `width` wide, with square caps (it extends `width / 2`
// past each end) and analytic edge coverage (`gup::coverage`).
#define_import_path gup::marks::segment

#import gup::view::{View, px_to_clip}
#import gup::coverage::{line_quad, grow_quad, box_coverage, box_distance}

// One field per channel, in `Segment::CHANNELS` order.
struct SegmentIn {
    x: f32,
    y: f32,
    x_end: f32,
    y_end: f32,
    width: f32,
    color: vec4<f32>,
}

struct Varyings {
    // The mark contract names the position `clip`.
    @builtin(position) clip: vec4<f32>,
    // Offset from the centre (along, across), in physical pixels.
    @location(0) local: vec2<f32>,
    // Half the extent (caps included), in physical pixels.
    @location(1) half: vec2<f32>,
    // sRGB-encoded, straight alpha.
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) row: u32,
}

fn vertex(m: SegmentIn, vertex_index: u32, row: u32, view: View) -> Varyings {
    let q = grow_quad(
        line_quad(
            vec2<f32>(m.x, m.y),
            vec2<f32>(m.x_end, m.y_end),
            m.width,
            m.width * 0.5,
            vertex_index,
            view.dpr,
        ),
        view.pick_margin,
        vertex_index,
        view.dpr,
    );
    var out: Varyings;
    out.clip = px_to_clip(q.position, view);
    out.local = q.local;
    out.half = q.half;
    out.color = m.color;
    out.row = row;
    return out;
}

fn shade(v: Varyings) -> vec4<f32> {
    let alpha = v.color.a * box_coverage(v.local, v.half);
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(v.color.rgb * alpha, alpha);
}

// The signed distance from the fragment to the stroke's box (caps
// included), in physical pixels: negative inside. A negative width is
// never hit (RFC-001 S9d).
fn hit_distance(v: Varyings) -> f32 {
    return select(box_distance(v.local, v.half), 1e30, v.half.y < 0.0);
}

// The row a pick of this segment reports.
fn hit_row(v: Varyings) -> u32 {
    return v.row;
}
