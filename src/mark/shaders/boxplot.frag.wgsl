// Box Plot Fragment Shader
// Renders the box with anti-aliased edges

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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance to edge for anti-aliasing
    let edge_distance = min(
        min(0.5 - abs(input.local_position.x), 0.5 - abs(input.local_position.y)),
        0.5
    );
    
    // Anti-aliasing
    let edge_width = 0.01;
    let alpha = smoothstep(0.0, edge_width, edge_distance);
    
    // Determine if we're on the stroke or fill
    let is_stroke = edge_distance < input.stroke_width / input.max_width.y;
    
    var color: vec4<f32>;
    if (is_stroke) {
        color = input.box_stroke_color;
    } else {
        color = input.box_fill_color;
    }
    
    return vec4<f32>(color.rgb, color.a * alpha);
}
