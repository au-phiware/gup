// Optimized fragment shader for circle mark rendering
// Uses distance field calculations for anti-aliased circles

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) radius: f32,
    @location(5) stroke_width: f32,
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from center in local coordinates
    let distance_from_center = length(input.local_position);
    
    // Define circle boundaries
    let outer_radius = 1.0;
    let stroke_thickness = input.stroke_width / input.radius;
    let inner_radius = max(0.0, outer_radius - stroke_thickness);
    
    // Anti-aliasing edge width
    let edge_width = 0.02;
    
    // Calculate alpha values with smooth transitions
    let outer_alpha = 1.0 - smoothstep(outer_radius - edge_width, outer_radius + edge_width, distance_from_center);
    
    // Handle stroke rendering
    if (stroke_thickness > 0.0) {
        let inner_alpha = smoothstep(inner_radius - edge_width, inner_radius + edge_width, distance_from_center);
        let stroke_alpha = outer_alpha * inner_alpha;
        let fill_alpha = outer_alpha * (1.0 - inner_alpha);
        
        // Blend stroke and fill colors
        let final_color = input.stroke_color * stroke_alpha + input.fill_color * fill_alpha;
        return vec4<f32>(final_color.rgb, max(stroke_alpha, fill_alpha));
    } else {
        // No stroke - just fill
        return vec4<f32>(input.fill_color.rgb, input.fill_color.a * outer_alpha);
    }
}