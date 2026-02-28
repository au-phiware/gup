// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// General-purpose UI quad shader — renders rounded rectangles with optional
// border, drop shadow, and triangular arrow pointer.  Used for tooltips,
// legends, annotation callouts, focus highlights, and other UI chrome.

struct Uniforms {
    projection: mat4x4<f32>,
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    // Per-instance attributes
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_max: vec2<f32>,
    @location(3) bg_color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) params: vec4<f32>,  // (corner_radius, border_width, opacity, shadow_radius)
    @location(6) shadow_color: vec4<f32>,
    @location(7) shadow_offset: vec2<f32>,
    @location(8) arrow_params: vec4<f32>, // (direction, size, offset_along_edge, 0)
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) rect_size: vec2<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) shadow_color: vec4<f32>,
    @location(6) arrow_params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let rect_size = vertex.rect_max - vertex.rect_min;
    let shadow_radius = vertex.params.w;
    let border_width = vertex.params.y;
    let arrow_size = vertex.arrow_params.y;

    // Expand the quad to cover shadow, border, and arrow
    let shadow_expand = shadow_radius + max(abs(vertex.shadow_offset.x), abs(vertex.shadow_offset.y));
    let expand = max(max(shadow_expand, border_width), arrow_size);
    let expanded_min = vertex.rect_min - vec2<f32>(expand, expand);
    let expanded_max = vertex.rect_max + vec2<f32>(expand, expand);
    let expanded_size = expanded_max - expanded_min;

    // Map unit-quad position [0,1] to expanded world-space
    let world_pos = expanded_min + vertex.position * expanded_size;

    out.clip_position = uniforms.projection * vec4<f32>(world_pos, 0.0, 1.0);

    // local_pos is relative to rect_min so the fragment shader can do SDF math
    out.local_pos = world_pos - vertex.rect_min;
    out.rect_size = rect_size;
    out.bg_color = vertex.bg_color;
    out.border_color = vertex.border_color;
    out.params = vertex.params;
    out.shadow_color = vertex.shadow_color;
    out.arrow_params = vertex.arrow_params;

    return out;
}

// Signed distance to a rounded rectangle centred at the origin.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius, radius);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// Signed distance to an arbitrary triangle (exact).
// Uses the Inigo Quilez formulation.
fn sdf_triangle(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0;
    let e1 = p2 - p1;
    let e2 = p0 - p2;
    let v0 = p - p0;
    let v1 = p - p1;
    let v2 = p - p2;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(min(
        vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
        vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
        vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// Compute the combined SDF for the tooltip shape (rectangle + optional arrow).
//
// `p` is relative to the rect centre.
// `half_size` is half the rect dimensions.
// `corner_radius` is the rounded-rect corner radius.
// `arrow_params`: (direction, size, offset_along_edge, 0).
//   direction: 0=none, 1=top, 2=bottom, 3=left, 4=right.
fn sdf_tooltip(
    p: vec2<f32>,
    half_size: vec2<f32>,
    corner_radius: f32,
    arrow_params: vec4<f32>,
) -> f32 {
    var d = sdf_rounded_rect(p, half_size, corner_radius);

    let arrow_dir = arrow_params.x;
    let arrow_size = arrow_params.y;
    let arrow_off = arrow_params.z;

    if (arrow_dir > 0.5 && arrow_size > 0.0) {
        // Compute triangle vertices based on direction.
        var t0: vec2<f32>;  // apex
        var t1: vec2<f32>;  // base corner 1
        var t2: vec2<f32>;  // base corner 2

        if (arrow_dir < 1.5) {
            // Top: arrow points upward from the top edge
            t0 = vec2<f32>(arrow_off, -half_size.y - arrow_size);
            t1 = vec2<f32>(arrow_off - arrow_size, -half_size.y);
            t2 = vec2<f32>(arrow_off + arrow_size, -half_size.y);
        } else if (arrow_dir < 2.5) {
            // Bottom: arrow points downward from the bottom edge
            t0 = vec2<f32>(arrow_off, half_size.y + arrow_size);
            t1 = vec2<f32>(arrow_off + arrow_size, half_size.y);
            t2 = vec2<f32>(arrow_off - arrow_size, half_size.y);
        } else if (arrow_dir < 3.5) {
            // Left: arrow points left from the left edge
            t0 = vec2<f32>(-half_size.x - arrow_size, arrow_off);
            t1 = vec2<f32>(-half_size.x, arrow_off + arrow_size);
            t2 = vec2<f32>(-half_size.x, arrow_off - arrow_size);
        } else {
            // Right: arrow points right from the right edge
            t0 = vec2<f32>(half_size.x + arrow_size, arrow_off);
            t1 = vec2<f32>(half_size.x, arrow_off - arrow_size);
            t2 = vec2<f32>(half_size.x, arrow_off + arrow_size);
        }

        let d_arrow = sdf_triangle(p, t0, t1, t2);
        d = min(d, d_arrow);
    }

    return d;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let corner_radius = in.params.x;
    let border_width = in.params.y;
    let opacity = in.params.z;
    let shadow_radius = in.params.w;

    let half_size = in.rect_size * 0.5;
    let centre = half_size;
    let p = in.local_pos - centre;

    // Compute the combined tooltip SDF (rect + optional arrow).
    let d = sdf_tooltip(p, half_size, corner_radius, in.arrow_params);

    // ── Shadow ──────────────────────────────────────────────────────────
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (shadow_radius > 0.0 && in.shadow_color.a > 0.0) {
        // Gaussian-ish falloff over shadow_radius pixels
        let shadow_alpha = 1.0 - smoothstep(0.0, shadow_radius, d);
        color = vec4<f32>(in.shadow_color.rgb, in.shadow_color.a * shadow_alpha * opacity);
    }

    // ── Body + border ───────────────────────────────────────────────────
    let aa = fwidth(d) * 0.5; // half-pixel anti-aliasing

    if (border_width > 0.0) {
        // Inner boundary: inset the combined SDF by border_width.
        // For an exact distance field, d + offset equals the SDF of the
        // shape shrunk by that offset.
        let d_inner = d + border_width;

        let border_alpha = (1.0 - smoothstep(-aa, aa, d)) * smoothstep(-aa, aa, d_inner);
        let fill_alpha = 1.0 - smoothstep(-aa, aa, d_inner);

        let border_c = vec4<f32>(in.border_color.rgb, in.border_color.a * border_alpha * opacity);
        let fill_c = vec4<f32>(in.bg_color.rgb, in.bg_color.a * fill_alpha * opacity);

        // Composite fill over shadow, then border over that
        color = alpha_over(fill_c, color);
        color = alpha_over(border_c, color);
    } else {
        let fill_alpha = 1.0 - smoothstep(-aa, aa, d);
        let fill_c = vec4<f32>(in.bg_color.rgb, in.bg_color.a * fill_alpha * opacity);
        color = alpha_over(fill_c, color);
    }

    if (color.a < 0.001) {
        discard;
    }

    return color;
}

// Standard premultiplied-alpha "over" compositing.
fn alpha_over(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let out_a = src.a + dst.a * (1.0 - src.a);
    if (out_a < 0.001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let out_rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}
