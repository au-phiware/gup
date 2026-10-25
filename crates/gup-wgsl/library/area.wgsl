// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The Area mark: one instance per pair of consecutive rows, the region
// between the top edge (`y`) and the baseline (`y_end`) over the span from
// `x` to `x_next`, in logical pixels. Adjacent instances share their edge
// exactly, so a translucent area has no seam and nothing is blended twice.
// Where the top edge crosses the baseline within a segment, the region is
// the two triangles either side of the crossing.
#define_import_path gup::marks::area

#import gup::view::{View, px_to_clip}

// The channels in `Area::CHANNELS` order, then each position channel read
// at the next row (the `Polyline` geometry).
struct AreaIn {
    x: f32,
    y: f32,
    y_end: f32,
    fill: vec4<f32>,
    fill_opacity: f32,
    x_next: f32,
    y_next: f32,
    y_end_next: f32,
}

struct Varyings {
    // The mark contract names the position `clip`.
    @builtin(position) clip: vec4<f32>,
    // sRGB-encoded, straight alpha.
    @location(0) fill: vec4<f32>,
    @location(1) @interpolate(flat) row: u32,
}

fn vertex(m: AreaIn, vertex_index: u32, row: u32, view: View) -> Varyings {
    let a = vec2<f32>(m.x, m.y);
    let b = vec2<f32>(m.x_next, m.y_next);
    let a0 = vec2<f32>(m.x, m.y_end);
    let b0 = vec2<f32>(m.x_next, m.y_end_next);
    let d0 = m.y - m.y_end;
    let d1 = m.y_next - m.y_end_next;
    var p: vec2<f32>;
    if (d0 * d1 >= 0.0) {
        // No crossing: (a, b, b0) and (a, b0, a0).
        switch (vertex_index) {
            case 0u, 3u: { p = a; }
            case 1u: { p = b; }
            case 2u, 4u: { p = b0; }
            default: { p = a0; }
        }
    } else {
        // The top edge meets the baseline at `c`: (a, a0, c) and (b, b0, c).
        let t = d0 / (d0 - d1);
        let c = a + t * (b - a);
        switch (vertex_index) {
            case 0u: { p = a; }
            case 1u: { p = a0; }
            case 3u: { p = b; }
            case 4u: { p = b0; }
            default: { p = c; }
        }
    }
    var out: Varyings;
    out.clip = px_to_clip(p, view);
    out.fill = vec4<f32>(m.fill.rgb, m.fill.a * m.fill_opacity);
    out.row = row;
    return out;
}

fn shade(v: Varyings) -> vec4<f32> {
    if (v.fill.a <= 0.0) {
        discard;
    }
    return vec4<f32>(v.fill.rgb * v.fill.a, v.fill.a);
}
