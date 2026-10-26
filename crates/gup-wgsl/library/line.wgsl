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
// A step curve (GUP-550) draws each instance as 2 or 3 pieces of the step
// between the two rows, 6 vertices each: the glue calls `vertex_variant`
// with the layer's curve and its stored row count, and the piece is
// `vertex_index / 6`. The pieces' ends are computed here, in logical
// pixels, so a step is level and plumb under any scale. Where two pieces
// meet, every pixel is drawn by exactly one of them, so no rim pixel is
// blended twice: one piece ends butt (it draws only the pixels whose
// centre lies on its own side of the line through the joint, across its
// axis, with no ramp along it), and the other either ends butt too (the
// pieces are collinear) or keeps the half of its round cap outside the
// butt piece (a corner, rounded on the outside).
//
// A monotone curve (GUP-551) draws each instance as 16 pieces of the cubic
// Bezier from row to row: the glue calls `vertex_controls` with the row's
// two control values (its control columns through the y scale), and the
// control points lie at one and two thirds of the segment in x. Every
// joint, between pieces and at rows, is a plane through the joint point
// perpendicular to the curve's tangent there: each piece draws the pixels
// on its side, as a strip without a cap, so no pixel is drawn twice and
// none is left out. Both pieces compute the same plane: within a segment
// from the same cubic, and at a row because the curve's tangent there is
// continuous under a linear or time y scale (the control values are
// computed from one tangent per row).
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
    // How each end joins the next piece: `JOIN_*` at the start (bits 0-1)
    // and at the end (bits 2-3). A straight segment's are 0 (round caps).
    @location(5) @interpolate(flat) joins: u32,
    // For a `JOIN_KEEP` end, the butt piece's direction away from the
    // joint, in the local (along, across) frame: the start's in `xy`, the
    // end's in `zw`.
    @location(6) @interpolate(flat) others: vec4<f32>,
}

// The curve a layer's glue passes to `vertex_variant` or
// `vertex_controls` (`Curve::variant`).
const CURVE_STEP: u32 = 1u;
const CURVE_STEP_BEFORE: u32 = 2u;
const CURVE_STEP_AFTER: u32 = 3u;
const CURVE_MONOTONE_X: u32 = 4u;

// The pieces a monotone segment is drawn as (`marks::MONOTONE_PIECES`).
const MONOTONE_PIECES: u32 = 16u;

// How a piece's end meets its neighbour.
// A round cap: the line ends here.
const JOIN_CAP: u32 = 0u;
// Butt: the pixels whose centres lie beyond the end are the neighbour's.
const JOIN_BUTT: u32 = 1u;
// A round cap less the pixels on the (butt) neighbour's side.
const JOIN_KEEP: u32 = 2u;
// No cap: the pixels on this piece's side of the plane through the end
// whose normal (pointing along the curve) `others` holds, at the strip's
// across-axis coverage. The neighbour draws the other side.
const JOIN_PLANE: u32 = 3u;

// Pixels within this distance (physical pixels) of the line between two
// pieces are drawn by both, so a pixel centre that each piece places a
// rounding error apart is never left out.
const JOIN_SLACK: f32 = 0.01;

// One capsule from `start` to `stop`, corner `corner` (0..5) of its quad,
// its `side` the vertex's x right of `turn`, with round caps.
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
    out.joins = JOIN_CAP;
    out.others = vec4<f32>(0.0);
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

// The unit direction from `p` to `q`, along x when they coincide (as
// `line_quad` lays out a piece of no length).
fn direction(p: vec2<f32>, q: vec2<f32>) -> vec2<f32> {
    let d = q - p;
    let len = length(d);
    return select(vec2<f32>(1.0, 0.0), d / len, len > 0.0);
}

// `u` (logical pixels) in the local frame of a piece along `axis`.
fn to_local(u: vec2<f32>, axis: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(dot(u, axis), dot(u, vec2<f32>(-axis.y, axis.x)));
}

// Vertex `vertex_index` of a segment drawn as `curve`: piece
// `vertex_index / 6`, corner `vertex_index % 6`. `rows` is the layer's
// stored row count: the first and last segments end in round caps.
//
// The joints: within a segment the earlier piece keeps its cap and the
// later is butt. Between segments a step's pieces cannot see the
// neighbour segment's, so the joint's rule follows from what each side
// knows, assuming x runs the same way in both segments: StepAfter's
// riser keeps its cap and the next level is butt, StepBefore's riser
// (the later piece) keeps its cap and the level before is butt, and
// Step's collinear levels are both butt. A null row is not seen: the
// ends either side of it are drawn as joints.
fn vertex_variant(m: LineIn, vertex_index: u32, row: u32, view: View, curve: u32, rows: u32) -> Varyings {
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

    let axis = direction(start, stop);
    // The x direction of the levels.
    let run = vec2<f32>(select(1.0, -1.0, b.x < a.x), 0.0);
    var start_join = JOIN_BUTT;
    var end_join = JOIN_KEEP;
    var start_other = vec2<f32>(0.0);
    // Within the segment, the next piece.
    var end_other = direction(stop, step_point(curve, a, b, n, min(k + 2u, n)));
    if (k == 0u) {
        if (row == 0u) {
            start_join = JOIN_CAP;
        } else if (curve == CURVE_STEP_BEFORE) {
            // The riser keeps its cap; the level before runs back along x.
            start_join = JOIN_KEEP;
            start_other = -run;
        }
    }
    if (k + 1u == n) {
        if (row + 2u >= rows) {
            end_join = JOIN_CAP;
        } else if (curve == CURVE_STEP_AFTER) {
            // The riser keeps its cap; the next level runs on along x.
            end_other = run;
        } else {
            end_join = JOIN_BUTT;
        }
    }
    out.joins = start_join | (end_join << 2u);
    out.others = vec4<f32>(to_local(start_other, axis), to_local(end_other, axis));
    return out;
}

// The cubic Bezier from `p0` to `p3` with control points `p1`, `p2`, at
// `t` (as `monotone::bezier` on the CPU).
fn bezier(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let u = 1.0 - t;
    return u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3;
}

// The unit tangent of the same cubic at `t`; the chord's direction where
// the derivative vanishes.
fn bezier_tangent(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let u = 1.0 - t;
    let d = u * u * (p1 - p0) + 2.0 * u * t * (p2 - p1) + t * t * (p3 - p2);
    let len = length(d);
    return select(direction(p0, p3), d / len, len > 0.0);
}

// Vertex `vertex_index` of a segment drawn as a monotone cubic (`curve`
// is `CURVE_MONOTONE_X`): piece `vertex_index / 6` of 16, corner
// `vertex_index % 6`. `controls` are the y values (logical pixels) of the
// control points at one and two thirds of the segment in x. `rows` is the
// layer's stored row count: the first and last segments end in round
// caps; every other end is a `JOIN_PLANE` perpendicular to the curve. A
// null row is not seen: the ends either side of it end flat, at the row.
fn vertex_controls(m: LineIn, controls: vec2<f32>, vertex_index: u32, row: u32, view: View, curve: u32, rows: u32) -> Varyings {
    let a = vec2<f32>(m.x, m.y);
    let b = vec2<f32>(m.x_next, m.y_next);
    let dx = b.x - a.x;
    let p1 = vec2<f32>(a.x + dx / 3.0, controls.x);
    let p2 = vec2<f32>(a.x + dx * 2.0 / 3.0, controls.y);
    let k = vertex_index / 6u;
    let t0 = f32(k) / f32(MONOTONE_PIECES);
    let t1 = f32(k + 1u) / f32(MONOTONE_PIECES);
    let start = select(bezier(a, p1, p2, b, t0), a, k == 0u);
    let stop = select(bezier(a, p1, p2, b, t1), b, k + 1u == MONOTONE_PIECES);
    var out = capsule(m, start, stop, vertex_index % 6u, row, (a.x + b.x) * 0.5, view);
    // The row nearer in x: the fragment's x past the midpoint towards the
    // next row (exact, as the quad's position is linear in screen space).
    out.side = out.side * sign(dx);

    let axis = direction(start, stop);
    var start_join = JOIN_PLANE;
    var end_join = JOIN_PLANE;
    if (k == 0u && row == 0u) {
        start_join = JOIN_CAP;
    }
    if (k + 1u == MONOTONE_PIECES && row + 2u >= rows) {
        end_join = JOIN_CAP;
    }
    let start_normal = bezier_tangent(a, p1, p2, b, t0);
    let end_normal = bezier_tangent(a, p1, p2, b, t1);
    out.joins = start_join | (end_join << 2u);
    out.others = vec4<f32>(to_local(start_normal, axis), to_local(end_normal, axis));
    return out;
}

// The coverage of the pixel centred on `local` by the capsule of half
// length `half.x` and radius `half.y`: the box coverage across the
// segment, taken at the distance from the segment, so the body matches
// the Segment mark's profile and the caps are round. A butt end has no
// cap and no ramp: the segment runs on past it at full coverage, and the
// pixels past it are the neighbour's (`owned`).
fn capsule_coverage(local: vec2<f32>, half: vec2<f32>, joins: u32) -> f32 {
    let s = joins & 3u;
    let e = joins >> 2u;
    let before = select(max(-local.x - half.x, 0.0), 0.0, s == JOIN_BUTT || s == JOIN_PLANE);
    let after = select(max(local.x - half.x, 0.0), 0.0, e == JOIN_BUTT || e == JOIN_PLANE);
    return span(length(vec2<f32>(before + after, local.y)), half.y);
}

// Whether this piece draws the pixel centred on `local`, or its
// neighbour at a joint does.
fn owned(v: Varyings) -> bool {
    let start = vec2<f32>(-v.half.x, 0.0);
    let end = vec2<f32>(v.half.x, 0.0);
    let s = v.joins & 3u;
    let e = v.joins >> 2u;
    let past_start = v.local.x < -v.half.x;
    let past_end = v.local.x > v.half.x;
    return !(
        (s == JOIN_BUTT && v.local.x < -v.half.x - JOIN_SLACK)
            || (e == JOIN_BUTT && v.local.x > v.half.x + JOIN_SLACK)
            || (s == JOIN_KEEP && past_start && dot(v.local - start, v.others.xy) > JOIN_SLACK)
            || (e == JOIN_KEEP && past_end && dot(v.local - end, v.others.zw) > JOIN_SLACK)
            || (s == JOIN_PLANE && dot(v.local - start, v.others.xy) < -JOIN_SLACK)
            || (e == JOIN_PLANE && dot(v.local - end, v.others.zw) > JOIN_SLACK)
    );
}

fn shade(v: Varyings) -> vec4<f32> {
    let alpha = v.color.a * capsule_coverage(v.local, v.half, v.joins);
    if (alpha <= 0.0 || !owned(v)) {
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
// middle, else the next; for a step the curve's rule; for a monotone
// curve the row nearer in x.
fn hit_row(v: Varyings) -> u32 {
    return select(v.row + 1u, v.row, v.side <= 0.0);
}
