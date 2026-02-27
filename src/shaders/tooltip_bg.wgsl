// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tooltip background shader — renders solid-colour rounded rectangles with
// optional border and drop shadow.

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
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) rect_size: vec2<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) shadow_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let rect_size = vertex.rect_max - vertex.rect_min;
    let shadow_radius = vertex.params.w;
    let border_width = vertex.params.y;

    // Expand the quad to cover shadow and border
    let expand = max(shadow_radius + max(abs(vertex.shadow_offset.x), abs(vertex.shadow_offset.y)), border_width);
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

    return out;
}

// Signed distance to a rounded rectangle centred at the origin.
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius, radius);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
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

    // ── Shadow ──────────────────────────────────────────────────────────
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (shadow_radius > 0.0 && in.shadow_color.a > 0.0) {
        let d_shadow = sdf_rounded_rect(p, half_size, corner_radius);
        // Gaussian-ish falloff over shadow_radius pixels
        let shadow_alpha = 1.0 - smoothstep(0.0, shadow_radius, d_shadow);
        color = vec4<f32>(in.shadow_color.rgb, in.shadow_color.a * shadow_alpha * opacity);
    }

    // ── Body + border ───────────────────────────────────────────────────
    let d = sdf_rounded_rect(p, half_size, corner_radius);
    let aa = fwidth(d) * 0.5; // half-pixel anti-aliasing

    if (border_width > 0.0) {
        // Border is an annulus: outside the inner rect, inside the outer rect
        let inner_half = half_size - vec2<f32>(border_width, border_width);
        let inner_radius = max(corner_radius - border_width, 0.0);
        let d_inner = sdf_rounded_rect(p, inner_half, inner_radius);

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
