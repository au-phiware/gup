// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The Line mark (GUP-432): one instance per pair of consecutive rows, a
// capsule from (`x`, `y`) to (`x_next`, `y_next`) in logical pixels,
// `width` wide. Every point within `width / 2` of the segment is inside,
// so each end is a round cap and two segments meeting at a row overlap in
// a disc there: a round join, from the two endpoints alone. The overlap
// is drawn twice, which an opaque colour does not show; a translucent one
// is darker where segments meet.
//
// A step curve (GUP-550) draws each instance as 2 or 3 such capsules, the
// pieces of the step between the two rows, 6 vertices each: the glue calls
// `vertex_variant` with the layer's curve and the piece is
// `vertex_index / 6`. The pieces' ends are computed here, in logical
// pixels, so a step is level and plumb under any scale.
#define_import_path gup::marks::line

#import gup::view::{View, px_to_clip}
#import gup::coverage::{line_quad, grow_quad, span}

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
    // The pick's side, in physical pixels: at or below 0 the fragment
    // reports `row`, above it the next row (`hit_row`).
    @location(4) side: f32,
}

// The curve a layer's glue passes to `vertex_variant` (`Curve::variant`).
const CURVE_STEP: u32 = 1u;
const CURVE_STEP_BEFORE: u32 = 2u;
const CURVE_STEP_AFTER: u32 = 3u;

// One capsule from `start` to `stop`, corner `corner` (0..5) of its quad,
// its `side` the vertex's x right of `turn`.
fn capsule(m: LineIn, start: vec2<f32>, stop: vec2<f32>, corner: u32, row: u32, turn: f32, view: View) -> Varyings {
    // The quad spans the capsule: the segment plus a `width / 2` cap at
    // each end.
    let r = m.width * 0.5;
    let q = grow_quad(
        line_quad(start, stop, m.width, r, corner, view.dpr),
        view.pick_margin,
        corner,
        view.dpr,
    );
    var out: Varyings;
    out.clip = px_to_clip(q.position, view);
    out.local = q.local;
    out.half = vec2<f32>(q.half.x - q.half.y, q.half.y);
    out.color = m.color;
    out.row = row;
    out.side = (q.position.x - turn) * view.dpr;
    return out;
}

fn vertex(m: LineIn, vertex_index: u32, row: u32, view: View) -> Varyings {
    var out = capsule(m, vec2<f32>(m.x, m.y), vec2<f32>(m.x_next, m.y_next), vertex_index, row, 0.0, view);
    // The nearer end: this row at or before the middle, else the next. A
    // segment of no length reports this row.
    out.side = select(out.local.x, -1.0, out.half.x <= 0.0);
    return out;
}

// The x at which a step leaves row `a`'s level for row `b`'s.
fn step_turn(curve: u32, a: vec2<f32>, b: vec2<f32>) -> f32 {
    if (curve == CURVE_STEP) {
        return (a.x + b.x) * 0.5;
    }
    return select(b.x, a.x, curve == CURVE_STEP_BEFORE);
}

// Point `k` (0 to `n`) of the step from `a` to `b` in `n` pieces: at
// `a`'s x first, `b`'s last and the turn's between; at `a`'s level up to
// the first vertical piece (none for StepBefore), `b`'s after.
fn step_point(curve: u32, a: vec2<f32>, b: vec2<f32>, n: u32, k: u32) -> vec2<f32> {
    let x = select(select(step_turn(curve, a, b), b.x, k == n), a.x, k == 0u);
    let level = select(1u, 0u, curve == CURVE_STEP_BEFORE);
    return vec2<f32>(x, select(b.y, a.y, k <= level));
}

// Vertex `vertex_index` of a segment drawn as `curve`: piece
// `vertex_index / 6`, corner `vertex_index % 6`.
fn vertex_variant(m: LineIn, vertex_index: u32, row: u32, view: View, curve: u32) -> Varyings {
    let a = vec2<f32>(m.x, m.y);
    let b = vec2<f32>(m.x_next, m.y_next);
    let n = select(2u, 3u, curve == CURVE_STEP);
    let k = vertex_index / 6u;
    let start = step_point(curve, a, b, n, k);
    let stop = step_point(curve, a, b, n, k + 1u);
    var out = capsule(m, start, stop, vertex_index % 6u, row, (a.x + b.x) * 0.5, view);
    // Step reports the row on the fragment's side of the midpoint (the
    // quad's position is linear in screen space, so the interpolated side
    // is exact), StepAfter this row and StepBefore the next.
    if (curve != CURVE_STEP) {
        out.side = select(-1.0, 1.0, curve == CURVE_STEP_BEFORE);
    }
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

// The distance from the fragment to the segment, less the half width,
// in physical pixels: negative inside the capsule. A negative width is
// never hit (RFC-001 S9d).
fn hit_distance(v: Varyings) -> f32 {
    let beyond = max(abs(v.local.x) - v.half.x, 0.0);
    return select(length(vec2<f32>(beyond, v.local.y)) - v.half.y, 1e30, v.half.y < 0.0);
}

// The row a fragment picks (`side`): for a straight segment the end
// nearer the fragment, this row (the segment's first) at or before the
// middle, else the next; for a step the curve's rule.
fn hit_row(v: Varyings) -> u32 {
    return select(v.row + 1u, v.row, v.side <= 0.0);
}
