// Box Plot Vertex Shader — unified SDF renderer
// Positions a quad that covers the full box plot extent (whiskers + outliers).
// The fragment shader uses the instance data (read from the storage buffer
// via flat instance_index) to render all components: box, median, whiskers,
// caps, and outlier circles.

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

struct VertexInput {
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) @interpolate(flat) instance_index: u32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let inst = instances[input.instance_index];

    // Compute the full extent including whiskers and outliers.
    var val_min = inst.whisker_min;
    var val_max = inst.whisker_max;
    for (var i = 0u; i < inst.outlier_count; i++) {
        let v = inst.outliers[i / 4u][i % 4u];
        val_min = min(val_min, v);
        val_max = max(val_max, v);
    }

    // Add margin so outlier circles and strokes are not clipped.
    let margin = max(inst.outlier_radius, inst.stroke_width) + 0.005;
    val_min -= margin;
    val_max += margin;
    let half_w = inst.width * 0.5 + margin;

    // Map the unit quad [-0.5, 0.5] to cover the full extent.
    var world_pos: vec2<f32>;
    if (inst.orientation == 0u) {
        // Vertical: category = x, value = y
        world_pos = vec2<f32>(
            inst.position.x + input.position.x * half_w * 2.0,
            val_min + (input.position.y + 0.5) * (val_max - val_min),
        );
    } else {
        // Horizontal: category = y, value = x
        world_pos = vec2<f32>(
            val_min + (input.position.x + 0.5) * (val_max - val_min),
            inst.position.y + input.position.y * half_w * 2.0,
        );
    }

    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.instance_index = input.instance_index;
    return output;
}
