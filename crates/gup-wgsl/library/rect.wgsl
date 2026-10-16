// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The Rect mark: an axis-aligned filled box in logical pixels. On each axis,
// a non-zero extent (`width`, `height`) gives a box centred on `x` (`y`) of
// that size; a zero extent gives the box between the two edges (`x`, `x_end`),
// in either order. Edges are covered analytically (`gup::coverage`).
#define_import_path gup::marks::rect

#import gup::view::{View, px_to_clip}
#import gup::coverage::{rect_quad, box_coverage}

// One field per channel, in `Rect::CHANNELS` order.
struct RectIn {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    x_end: f32,
    y_end: f32,
    fill: vec4<f32>,
    fill_opacity: f32,
    stroke: vec4<f32>,
    stroke_width: f32,
}

struct Varyings {
    // The mark contract names the position `clip`.
    @builtin(position) clip: vec4<f32>,
    // Offset from the centre, in physical pixels.
    @location(0) local: vec2<f32>,
    // Half the extent, in physical pixels.
    @location(1) half: vec2<f32>,
    // sRGB-encoded, straight alpha.
    @location(2) fill: vec4<f32>,
    // Straight alpha, stroke inside the edge, width in physical pixels.
    @location(3) stroke: vec4<f32>,
    @location(4) stroke_width: f32,
    @location(5) @interpolate(flat) row: u32,
}

// The span `lo..hi` on one axis.
fn span(centre: f32, extent: f32, edge: f32) -> vec2<f32> {
    if (extent != 0.0) {
        let h = abs(extent) * 0.5;
        return vec2<f32>(centre - h, centre + h);
    }
    return vec2<f32>(min(centre, edge), max(centre, edge));
}

fn vertex(m: RectIn, vertex_index: u32, row: u32, view: View) -> Varyings {
    let sx = span(m.x, m.width, m.x_end);
    let sy = span(m.y, m.height, m.y_end);
    let q = rect_quad(vec2<f32>(sx.x, sy.x), vec2<f32>(sx.y, sy.y), vertex_index, view.dpr);
    var out: Varyings;
    out.clip = px_to_clip(q.position, view);
    out.local = q.local;
    out.half = q.half;
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

fn shade(v: Varyings) -> vec4<f32> {
    let outer = box_coverage(v.local, v.half);
    let inner = box_coverage(v.local, max(v.half - vec2<f32>(v.stroke_width), vec2<f32>(0.0)));
    return compose(v.fill, v.stroke, outer, outer - inner);
}
