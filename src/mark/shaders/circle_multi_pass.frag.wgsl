// Multi-pass fragment shader for circle mark rendering
// Provides separate entry points for fill-only and outline-only rendering,
// plus a shadow entry point.
//
// Entry points:
//   - fs_main:    Full circle with fill and stroke (same as standard circle)
//   - fs_fill:    Fill only – no stroke ring
//   - fs_outline: Outline (stroke) ring only – no interior fill
//   - fs_shadow:  Soft shadow disc with Gaussian-like falloff

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) radius: f32,
    @location(5) stroke_width: f32,
}

// ── Full circle (fill + stroke) ────────────────────────────────────────────
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance_from_center = length(input.local_position);

    let outer_radius = 1.0;
    let stroke_thickness = input.stroke_width / input.radius;
    let inner_radius = max(0.0, outer_radius - stroke_thickness);

    let edge_width = 0.02;

    let outer_alpha = 1.0 - smoothstep(outer_radius - edge_width, outer_radius + edge_width, distance_from_center);

    if (stroke_thickness > 0.0) {
        let inner_alpha = smoothstep(inner_radius - edge_width, inner_radius + edge_width, distance_from_center);
        let stroke_alpha = outer_alpha * inner_alpha;
        let fill_alpha = outer_alpha * (1.0 - inner_alpha);

        let final_color = input.stroke_color * stroke_alpha + input.fill_color * fill_alpha;
        return vec4<f32>(final_color.rgb, max(stroke_alpha, fill_alpha));
    } else {
        return vec4<f32>(input.fill_color.rgb, input.fill_color.a * outer_alpha);
    }
}

// ── Fill only (no stroke ring) ─────────────────────────────────────────────
@fragment
fn fs_fill(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance_from_center = length(input.local_position);

    let outer_radius = 1.0;
    let edge_width = 0.02;

    let alpha = 1.0 - smoothstep(outer_radius - edge_width, outer_radius + edge_width, distance_from_center);

    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(input.fill_color.rgb, input.fill_color.a * alpha);
}

// ── Outline only (stroke ring, transparent centre) ─────────────────────────
@fragment
fn fs_outline(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance_from_center = length(input.local_position);

    let outer_radius = 1.0;
    let stroke_thickness = input.stroke_width / input.radius;
    let inner_radius = max(0.0, outer_radius - stroke_thickness);

    let edge_width = 0.02;

    // Alpha at the outer edge
    let outer_alpha = 1.0 - smoothstep(outer_radius - edge_width, outer_radius + edge_width, distance_from_center);
    // Alpha transitioning into the stroke band
    let inner_alpha = smoothstep(inner_radius - edge_width, inner_radius + edge_width, distance_from_center);

    let ring_alpha = outer_alpha * inner_alpha;

    if (ring_alpha < 0.01) {
        discard;
    }
    return vec4<f32>(input.stroke_color.rgb, input.stroke_color.a * ring_alpha);
}

// ── Shadow disc (soft Gaussian-like falloff) ───────────────────────────────
@fragment
fn fs_shadow(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance_from_center = length(input.local_position);

    // Soft falloff – a wider smoothstep gives the shadow a blurry edge
    let alpha = 1.0 - smoothstep(0.5, 1.0, distance_from_center);

    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(input.fill_color.rgb, input.fill_color.a * alpha);
}
