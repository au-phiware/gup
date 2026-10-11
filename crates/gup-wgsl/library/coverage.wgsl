// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Analytic edge coverage for boxes: axis-aligned rects and lines of any
// direction with a real width (GUP-421). A shader expands its instance quad
// with `rect_quad` or `line_quad`, passes `local` and `half` to its
// fragment stage and multiplies its alpha by `box_coverage`. The coverage
// is the fraction of a one-physical-pixel footprint, centred on the
// fragment, inside the box, measured along each of the box's own axes:
//
// - an edge on a pixel boundary covers that pixel fully and its
//   neighbour not at all, so snapped hairlines and whole-pixel rects stay
//   exactly as crisp as without antialiasing;
// - an edge elsewhere ramps over one physical pixel, `clamp(0.5 - d, 0, 1)`
//   for a pixel centre `d` pixels outside it, as the circle mark's disc
//   edge does;
// - a box thinner than a pixel covers it by its width, so a half-pixel
//   rule is drawn at half strength rather than vanishing or doubling.
//
// For an axis-aligned box this is the exact area of the pixel inside it.
// For a rotated one it is the usual approximation: the footprint is taken
// along the box's axes, so a diagonal edge ramps over one pixel, not the
// up to √2 pixels a pixel square spans across it.
#define_import_path gup::coverage

// One corner of an instance quad, in the quad's own frame.
struct Quad {
    // Where the vertex goes, in logical pixels.
    position: vec2<f32>,
    // The vertex's offset from the box's centre along the box's axes
    // (along the line, across it), in physical pixels.
    local: vec2<f32>,
    // Half the box's extent along the same axes, in physical pixels.
    half: vec2<f32>,
}

// Corner of a quad for vertex 0..5 (two triangles), in -1..1.
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

// The quad of a box centred on `centre` with axes `axis` (unit length, the
// box's first axis) and its perpendicular, `half` logical pixels to each
// side, grown by one physical pixel on every side for the coverage ramp.
fn oriented_quad(centre: vec2<f32>, axis: vec2<f32>, half: vec2<f32>, vertex_index: u32, dpr: f32) -> Quad {
    let normal = vec2<f32>(-axis.y, axis.x);
    let offset = corner(vertex_index) * (half + vec2<f32>(1.0 / dpr));
    var q: Quad;
    q.position = centre + axis * offset.x + normal * offset.y;
    q.local = offset * dpr;
    q.half = half * dpr;
    return q;
}

// The quad of the axis-aligned rect from `lo` to `hi` (logical pixels, in
// either order).
fn rect_quad(lo: vec2<f32>, hi: vec2<f32>, vertex_index: u32, dpr: f32) -> Quad {
    return oriented_quad((lo + hi) * 0.5, vec2<f32>(1.0, 0.0), abs(hi - lo) * 0.5, vertex_index, dpr);
}

// The quad of a line from `start` to `stop` (logical pixels), `width`
// wide, extending `cap` logical pixels past each end: `width / 2` for
// square caps, 0 for butt caps. The box's first axis runs along the line.
// A line with no length runs along x, so a square-capped point is a
// `width`-sized square.
fn line_quad(start: vec2<f32>, stop: vec2<f32>, width: f32, cap: f32, vertex_index: u32, dpr: f32) -> Quad {
    let d = stop - start;
    let len = length(d);
    let axis = select(vec2<f32>(1.0, 0.0), d / len, len > 0.0);
    let half = vec2<f32>(len * 0.5 + cap, width * 0.5);
    return oriented_quad((start + stop) * 0.5, axis, half, vertex_index, dpr);
}

// The fraction of the one-pixel footprint centred on `t` inside
// `-half..half` (physical pixels).
fn span(t: f32, half: f32) -> f32 {
    return clamp(min(t + 0.5, half) - max(t - 0.5, -half), 0.0, 1.0);
}

// The coverage of the pixel centred on `local` by the box `half` (both
// from a `Quad`, interpolated), in 0..1.
fn box_coverage(local: vec2<f32>, half: vec2<f32>) -> f32 {
    return span(local.x, half.x) * span(local.y, half.y);
}
