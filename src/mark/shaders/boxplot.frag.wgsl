// Box Plot Fragment Shader — unified SDF renderer
// Reads instance data from the storage buffer to render all box plot
// components: box (IQR), median line, whisker lines, whisker caps, and
// outlier circles.  Anti-aliasing is achieved via smoothstep on signed
// distance values.

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

@group(0) @binding(0)
var<storage, read> instances: array<BoxPlotInstance>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) @interpolate(flat) instance_index: u32,
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let inst = instances[input.instance_index];

    // Map coordinates to value/category based on orientation.
    var val: f32;   // value-axis coordinate
    var cat: f32;   // category-axis coordinate
    var cat0: f32;  // category-axis centre of this box plot
    if (inst.orientation == 0u) {
        val = input.world_position.y;
        cat = input.world_position.x;
        cat0 = inst.position.x;
    } else {
        val = input.world_position.x;
        cat = input.world_position.y;
        cat0 = inst.position.y;
    }

    let hw  = inst.width * 0.5;   // half box width
    let dc  = abs(cat - cat0);    // distance from centre on category axis
    let sw  = inst.stroke_width;
    let aa  = 0.002;              // anti-aliasing half-width

    // ── Compute effective half-width (notch narrows the box at the median) ──
    var effective_hw = hw;
    if (inst.notched == 1u && val >= inst.q1 && val <= inst.q3) {
        // t = 0 at median, 1 at Q1/Q3 (distance from median as fraction)
        var t: f32;
        if (val <= inst.median) {
            t = (inst.median - val) / max(inst.median - inst.q1, 0.0001);
        } else {
            t = (val - inst.median) / max(inst.q3 - inst.median, 0.0001);
        }
        t = clamp(t, 0.0, 1.0);
        // At median (t=0): hw * (1 - notch_width), at Q1/Q3 (t=1): hw
        effective_hw = hw * (1.0 - inst.notch_width * (1.0 - t));
    }

    // ── Outlier circles (highest priority) ──────────────────────────
    for (var i = 0u; i < inst.outlier_count; i++) {
        let ov = inst.outliers[i / 4u][i % 4u];
        let d  = length(vec2<f32>(cat - cat0, val - ov));
        let r  = inst.outlier_radius;
        if (d < r + aa) {
            let outer_alpha = 1.0 - smoothstep(r - aa, r + aa, d);
            // Thin stroke ring around each outlier circle
            let inner = r - sw * 0.6;
            if (d > inner) {
                return vec4<f32>(inst.box_stroke_color.rgb,
                                inst.box_stroke_color.a * outer_alpha);
            }
            return vec4<f32>(inst.outlier_color.rgb,
                            inst.outlier_color.a * outer_alpha);
        }
    }

    // ── Box (IQR: Q1 → Q3) ─────────────────────────────────────────
    let in_box_x = dc <= effective_hw + aa;
    let in_box_y = val >= inst.q1 - aa && val <= inst.q3 + aa;
    if (in_box_x && in_box_y) {
        let edge_x = effective_hw - dc;
        let edge_y = min(val - inst.q1, inst.q3 - val);
        let edge   = min(edge_x, edge_y);
        let alpha  = smoothstep(-aa, aa, edge);

        // Stroke region (outer band)
        if (edge < sw) {
            return vec4<f32>(inst.box_stroke_color.rgb,
                            inst.box_stroke_color.a * alpha);
        }

        // Median line inside the box
        let md = abs(val - inst.median);
        if (md < sw * 0.8) {
            let m_alpha = 1.0 - smoothstep(sw * 0.4, sw * 0.8, md);
            let color   = mix(inst.box_fill_color, inst.median_color, m_alpha);
            return vec4<f32>(color.rgb, color.a * alpha);
        }

        return vec4<f32>(inst.box_fill_color.rgb,
                        inst.box_fill_color.a * alpha);
    }

    // ── Whisker lines (thin vertical/horizontal from box to extremes) ──
    let wlw = sw * 0.5;  // whisker line half-width

    // Lower whisker
    if (dc <= wlw + aa && val >= inst.whisker_min - aa && val < inst.q1) {
        let a = 1.0 - smoothstep(wlw - aa, wlw + aa, dc);
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * a);
    }
    // Upper whisker
    if (dc <= wlw + aa && val > inst.q3 && val <= inst.whisker_max + aa) {
        let a = 1.0 - smoothstep(wlw - aa, wlw + aa, dc);
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * a);
    }

    // ── Whisker caps (short horizontal/vertical bars at extremes) ───
    let cap_hw = hw * 0.5;  // cap half-width in category axis
    let cap_hh = sw * 0.5;  // cap half-height in value axis

    // Lower cap
    if (dc <= cap_hw + aa && abs(val - inst.whisker_min) <= cap_hh + aa) {
        let ax = 1.0 - smoothstep(cap_hw - aa, cap_hw + aa, dc);
        let ay = 1.0 - smoothstep(cap_hh - aa, cap_hh + aa, abs(val - inst.whisker_min));
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * ax * ay);
    }
    // Upper cap
    if (dc <= cap_hw + aa && abs(val - inst.whisker_max) <= cap_hh + aa) {
        let ax = 1.0 - smoothstep(cap_hw - aa, cap_hw + aa, dc);
        let ay = 1.0 - smoothstep(cap_hh - aa, cap_hh + aa, abs(val - inst.whisker_max));
        return vec4<f32>(inst.whisker_color.rgb, inst.whisker_color.a * ax * ay);
    }

    discard;
}
