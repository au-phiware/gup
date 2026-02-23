// Box Plot Vertex Shader
// Handles rendering of box plot components: box, median, whiskers, and outliers

struct BoxPlotInstance {
    position: vec2<f32>,
    min: f32,
    q1: f32,
    median: f32,
    q3: f32,
    max: f32,
    width: f32,
    orientation: u32,
    box_fill_color: vec4<f32>,
    box_stroke_color: vec4<f32>,
    median_color: vec4<f32>,
    whisker_color: vec4<f32>,
    stroke_width: f32,
    notched: u32,
    notch_width: f32,
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
    @location(1) local_position: vec2<f32>,
    @location(2) box_fill_color: vec4<f32>,
    @location(3) box_stroke_color: vec4<f32>,
    @location(4) median_color: vec4<f32>,
    @location(5) whisker_color: vec4<f32>,
    @location(6) stats: vec4<f32>, // min, q1, median, q3
    @location(7) max_width: vec2<f32>, // max, width
    @location(8) stroke_width: f32,
    @location(9) orientation: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    
    var output: VertexOutput;
    
    // For box plot, we render the box (Q1-Q3)
    // The vertex shader positions vertices for the box
    let box_height = instance.q3 - instance.q1;
    let box_center_y = (instance.q1 + instance.q3) * 0.5;
    
    var world_pos: vec2<f32>;
    
    if (instance.orientation == 0u) {
        // Vertical orientation
        world_pos = vec2<f32>(
            instance.position.x + input.position.x * instance.width,
            instance.position.y + input.position.y * box_height + box_center_y
        );
    } else {
        // Horizontal orientation
        world_pos = vec2<f32>(
            instance.position.x + input.position.y * box_height + box_center_y,
            instance.position.y + input.position.x * instance.width
        );
    }
    
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.box_fill_color = instance.box_fill_color;
    output.box_stroke_color = instance.box_stroke_color;
    output.median_color = instance.median_color;
    output.whisker_color = instance.whisker_color;
    output.stats = vec4<f32>(instance.min, instance.q1, instance.median, instance.q3);
    output.max_width = vec2<f32>(instance.max, instance.width);
    output.stroke_width = instance.stroke_width;
    output.orientation = f32(instance.orientation);
    
    return output;
}
