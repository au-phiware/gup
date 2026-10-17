// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The Line mark (GUP-432): one instance per pair of consecutive rows, a
// capsule from (`x`, `y`) to (`x_next`, `y_next`) in logical pixels,
// `width` wide. Every point within `width / 2` of the segment is inside,
// so each end is a round cap and two segments meeting at a row overlap in
// a disc there: a round join, from the two endpoints alone. The overlap
// is drawn twice, which an opaque colour does not show; a translucent one
// is darker where segments meet.
#define_import_path gup::marks::line

#import gup::view::{View, px_to_clip}
#import gup::coverage::{line_quad, span}

// The channels in `Line::CHANNELS` order, then each position channel read
// at the next row (the `Polyline` geometry).
struct LineIn {
    x: f32,
    y: f32,
    width: f32,
    color: vec4<f32>,
    x_next: f32,
    y_next: f32,
}

struct Varyings {
    // The mark contract names the position `clip`.
    @builtin(position) clip: vec4<f32>,
    // Offset from the segment's centre (along, across), in physical pixels.
    @location(0) local: vec2<f32>,
    // Half the segment's length and half its width, in physical pixels.
    @location(1) half: vec2<f32>,
    // sRGB-encoded, straight alpha.
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) row: u32,
}

fn vertex(m: LineIn, vertex_index: u32, row: u32, view: View) -> Varyings {
    // The quad spans the capsule: the segment plus a `width / 2` cap at
    // each end.
    let r = m.width * 0.5;
    let q = line_quad(
        vec2<f32>(m.x, m.y),
        vec2<f32>(m.x_next, m.y_next),
        m.width,
        r,
        vertex_index,
        view.dpr,
    );
    var out: Varyings;
    out.clip = px_to_clip(q.position, view);
    out.local = q.local;
    out.half = vec2<f32>(q.half.x - q.half.y, q.half.y);
    out.color = m.color;
    out.row = row;
    return out;
}

// The coverage of the pixel centred on `local` by the capsule of half
// length `half.x` and radius `half.y`: the box coverage across the
// segment, taken at the distance from the segment, so the body matches
// the Segment mark's profile and the caps are round.
fn capsule_coverage(local: vec2<f32>, half: vec2<f32>) -> f32 {
    let beyond = max(abs(local.x) - half.x, 0.0);
    return span(length(vec2<f32>(beyond, local.y)), half.y);
}

fn shade(v: Varyings) -> vec4<f32> {
    let alpha = v.color.a * capsule_coverage(v.local, v.half);
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(v.color.rgb * alpha, alpha);
}
