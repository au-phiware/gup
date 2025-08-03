// Optimized fragment shader for rectangle mark rendering
// Uses distance field calculations for anti-aliased rectangles with rounded corners

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) size: vec2<f32>,
    @location(5) stroke_width: f32,
    @location(6) corner_radius: f32,
}

// Signed distance function for rounded rectangle
fn sdf_rounded_rectangle(pos: vec2<f32>, size: vec2<f32>, corner_radius: f32) -> f32 {
    let half_size = size * 0.5;
    let corner_offset = max(abs(pos) - (half_size - corner_radius), vec2<f32>(0.0));
    let distance_to_corner = length(corner_offset);
    let distance_to_edge = max(
        max(abs(pos).x - half_size.x, abs(pos).y - half_size.y),
        distance_to_corner - corner_radius
    );
    return distance_to_edge;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate position in rectangle coordinate system
    let pos = input.local_position * input.size;
    
    // Calculate distance to rectangle edge using SDF
    let distance_to_edge = sdf_rounded_rectangle(pos, input.size, input.corner_radius);
    
    // Calculate stroke boundaries
    let outer_distance = distance_to_edge;
    let inner_distance = distance_to_edge + input.stroke_width;
    
    // Anti-aliasing edge width (resolution-independent)
    let edge_width = fwidth(distance_to_edge) * 0.5;
    
    // Calculate alpha values with smooth transitions
    let outer_alpha = 1.0 - smoothstep(-edge_width, edge_width, outer_distance);
    
    // Handle stroke rendering
    if (input.stroke_width > 0.0) {
        let inner_alpha = smoothstep(-edge_width, edge_width, inner_distance);
        let stroke_alpha = outer_alpha * (1.0 - inner_alpha);
        let fill_alpha = outer_alpha * inner_alpha;
        
        // Blend stroke and fill colors
        let final_color = input.stroke_color * stroke_alpha + input.fill_color * fill_alpha;
        return vec4<f32>(final_color.rgb, max(stroke_alpha, fill_alpha));
    } else {
        // No stroke - just fill
        return vec4<f32>(input.fill_color.rgb, input.fill_color.a * outer_alpha);
    }
}