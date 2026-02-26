// Box Plot Fragment Shader with Pattern Support
// Renders the box with anti-aliased edges and pattern support for accessibility.
//
// Stroke width and outlier radius are specified in pixels.  The viewport
// uniform converts them to clip-space units so that visual appearance
// is consistent regardless of window size.

struct BoxPlotInstance {
    position: vec2<f32>,
    whisker_min: f32,
    q1: f32,
    median: f32,
    q3: f32,
    whisker_max: f32,
    width: f32,
    box_fill_color: vec4<f32>,
    box_stroke_color: vec4<f32>,
    median_color: vec4<f32>,
    whisker_color: vec4<f32>,
    outlier_color: vec4<f32>,
    stroke_width: f32,
    outlier_radius: f32,
    orientation: u32,
    outlier_count: u32,
    notched: u32,
    notch_width: f32,
    _pad_notch: vec2<f32>,
    outliers: array<vec4<f32>, 8>,
}

struct ViewportUniforms {
    width: f32,
    height: f32,
}

@group(0) @binding(0)
var<storage, read> instances: array<BoxPlotInstance>;

@group(0) @binding(1)
var<uniform> viewport: ViewportUniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) @interpolate(flat) instance_index: u32,
}

struct PatternUniforms {
    pattern_type: u32,
    spacing: f32,
    angle: f32,
    foreground_color: vec4<f32>,
    background_color: vec4<f32>,
    thickness: f32,
    _padding: vec2<f32>,
}

@group(1) @binding(0)
var<uniform> pattern: PatternUniforms;

// Pattern type constants
const PATTERN_SOLID: u32 = 0u;
const PATTERN_DOTS: u32 = 1u;
const PATTERN_LINES: u32 = 2u;
const PATTERN_CROSSHATCH: u32 = 3u;

// Pattern generation functions
fn pattern_dots(position: vec2<f32>, spacing: f32) -> f32 {
    let grid_pos = position / spacing;
    let cell_offset = fract(grid_pos) - 0.5;
    let dist = length(cell_offset * spacing);
    let dot_radius = spacing * 0.2;
    let edge_width = 1.0;
    return 1.0 - smoothstep(dot_radius - edge_width, dot_radius + edge_width, dist);
}

fn pattern_lines(position: vec2<f32>, spacing: f32, angle: f32, thickness: f32) -> f32 {
    let cos_angle = cos(angle);
    let sin_angle = sin(angle);
    let rotated_x = position.x * cos_angle - position.y * sin_angle;
    let pattern_pos = fract(rotated_x / spacing);
    let dist_from_line = abs(pattern_pos - 0.5) * spacing;
    let half_thickness = thickness * 0.5;
    let edge_width = 1.0;
    return 1.0 - smoothstep(half_thickness - edge_width, half_thickness + edge_width, dist_from_line);
}

fn pattern_crosshatch(position: vec2<f32>, spacing: f32, thickness: f32) -> f32 {
    let horizontal_pos = fract(position.y / spacing);
    let h_dist = abs(horizontal_pos - 0.5) * spacing;
    let h_alpha = 1.0 - smoothstep(thickness * 0.25, thickness * 0.25 + 1.0, h_dist);

    let vertical_pos = fract(position.x / spacing);
    let v_dist = abs(vertical_pos - 0.5) * spacing;
    let v_alpha = 1.0 - smoothstep(thickness * 0.25, thickness * 0.25 + 1.0, v_dist);

    return max(h_alpha, v_alpha);
}

fn apply_pattern(position: vec2<f32>) -> vec4<f32> {
    var pattern_alpha = 0.0;

    switch pattern.pattern_type {
        case PATTERN_DOTS: {
            pattern_alpha = pattern_dots(position, pattern.spacing);
        }
        case PATTERN_LINES: {
            pattern_alpha = pattern_lines(position, pattern.spacing, pattern.angle, pattern.thickness);
        }
        case PATTERN_CROSSHATCH: {
            pattern_alpha = pattern_crosshatch(position, pattern.spacing, pattern.thickness);
        }
        default: {
            // PATTERN_SOLID
            return pattern.foreground_color;
        }
    }

    return mix(pattern.background_color, pattern.foreground_color, pattern_alpha);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let inst = instances[input.instance_index];

    var val: f32;
    var cat: f32;
    var cat0: f32;
    if (inst.orientation == 0u) {
        val = input.world_position.y;
        cat = input.world_position.x;
        cat0 = inst.position.x;
    } else {
        val = input.world_position.x;
        cat = input.world_position.y;
        cat0 = inst.position.y;
    }

    let hw  = inst.width * 0.5;
    let dc  = abs(cat - cat0);

    // Convert pixel-space stroke width and outlier radius to clip-space.
    let px2clip = 2.0 / vec2<f32>(viewport.width, viewport.height);
    let px2clip_iso = sqrt(px2clip.x * px2clip.y);
    let sw  = inst.stroke_width * px2clip_iso;
    let aa  = px2clip_iso;
    let r   = inst.outlier_radius * px2clip_iso;

    // ── Compute effective half-width (notch narrows the box at the median) ──
    var effective_hw = hw;
    if (inst.notched == 1u && val >= inst.q1 && val <= inst.q3) {
        var t: f32;
        if (val <= inst.median) {
            t = (inst.median - val) / max(inst.median - inst.q1, 0.0001);
        } else {
            t = (val - inst.median) / max(inst.q3 - inst.median, 0.0001);
        }
        t = clamp(t, 0.0, 1.0);
        effective_hw = hw * (1.0 - inst.notch_width * (1.0 - t));
    }

    // ── Outlier circles ─────────────────────────────────────────────
    for (var i = 0u; i < inst.outlier_count; i++) {
        let ov = inst.outliers[i / 4u][i % 4u];
        let d  = length(vec2<f32>(cat - cat0, val - ov));
        if (d < r + aa) {
            let outer_alpha = 1.0 - smoothstep(r - aa, r + aa, d);
            let inner = r - sw * 0.6;
            if (d > inner) {
                return vec4<f32>(inst.box_stroke_color.rgb,
                                inst.box_stroke_color.a * outer_alpha);
            }
            if pattern.pattern_type != PATTERN_SOLID {
                let pc = apply_pattern(input.world_position);
                return vec4<f32>(pc.rgb, pc.a * outer_alpha);
            }
            return vec4<f32>(inst.outlier_color.rgb,
                            inst.outlier_color.a * outer_alpha);
        }
    }

    // ── Box (IQR) ───────────────────────────────────────────────────
    let in_box_x = dc <= effective_hw + aa;
    let in_box_y = val >= inst.q1 - aa && val <= inst.q3 + aa;
    if (in_box_x && in_box_y) {
        let edge_x = effective_hw - dc;
        let edge_y = min(val - inst.q1, inst.q3 - val);
        let edge   = min(edge_x, edge_y);
        let alpha  = smoothstep(-aa, aa, edge);

        if (edge < sw) {
            return vec4<f32>(inst.box_stroke_color.rgb,
                            inst.box_stroke_color.a * alpha);
        }

        let md = abs(val - inst.median);
        if (md < sw * 0.8) {
            let m_alpha = 1.0 - smoothstep(sw * 0.4, sw * 0.8, md);
            let color   = mix(inst.box_fill_color, inst.median_color, m_alpha);
            return vec4<f32>(color.rgb, color.a * alpha);
        }

        if pattern.pattern_type != PATTERN_SOLID {
            let pc = apply_pattern(input.world_position);
            return vec4<f32>(pc.rgb, pc.a * alpha);
        }
        return vec4<f32>(inst.box_fill_color.rgb,
                        inst.box_fill_color.a * alpha);
    }

    // ── Whisker lines ───────────────────────────────────────────────
    let wlw = sw * 0.5;
    if (dc <= wlw + aa && val >= inst.whisker_min - aa && val < inst.q1) {
        let a = 1.0 - smoothstep(wlw - aa, wlw + aa, dc);
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * a);
    }
    if (dc <= wlw + aa && val > inst.q3 && val <= inst.whisker_max + aa) {
        let a = 1.0 - smoothstep(wlw - aa, wlw + aa, dc);
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * a);
    }

    // ── Whisker caps ────────────────────────────────────────────────
    let cap_hw = hw * 0.5;
    let cap_hh = sw * 0.5;
    if (dc <= cap_hw + aa && abs(val - inst.whisker_min) <= cap_hh + aa) {
        let ax = 1.0 - smoothstep(cap_hw - aa, cap_hw + aa, dc);
        let ay = 1.0 - smoothstep(cap_hh - aa, cap_hh + aa, abs(val - inst.whisker_min));
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * ax * ay);
    }
    if (dc <= cap_hw + aa && abs(val - inst.whisker_max) <= cap_hh + aa) {
        let ax = 1.0 - smoothstep(cap_hw - aa, cap_hw + aa, dc);
        let ay = 1.0 - smoothstep(cap_hh - aa, cap_hh + aa, abs(val - inst.whisker_max));
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * ax * ay);
    }

    discard;
}
