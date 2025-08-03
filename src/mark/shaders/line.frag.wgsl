// Optimized fragment shader for line mark rendering
// Uses distance field calculations for anti-aliased lines with style support

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) width: f32,
    @location(4) style: u32,
    @location(5) line_length: f32,
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from line center
    let distance_from_center = abs(input.local_position.y);
    
    // Calculate base alpha for line width with anti-aliasing
    let half_width = 0.5;
    let edge_width = fwidth(distance_from_center) * 0.5;
    let base_alpha = 1.0 - smoothstep(half_width - edge_width, half_width + edge_width, distance_from_center);
    
    // Apply line style patterns
    var style_alpha = 1.0;
    let pattern_coord = input.local_position.x * input.line_length;
    
    if (input.style == 1u) { // Dashed
        let dash_size = 10.0;
        let gap_size = 5.0;
        let cycle = dash_size + gap_size;
        let position_in_cycle = fract(pattern_coord / cycle) * cycle;
        style_alpha = select(0.0, 1.0, position_in_cycle < dash_size);
        
        // Smooth dash transitions
        let transition_width = 1.0;
        if (position_in_cycle < transition_width) {
            style_alpha *= smoothstep(0.0, transition_width, position_in_cycle);
        } else if (position_in_cycle > dash_size - transition_width) {
            style_alpha *= smoothstep(dash_size, dash_size - transition_width, position_in_cycle);
        }
    } else if (input.style == 2u) { // Dotted
        let dot_spacing = 8.0;
        let dot_size = 3.0;
        let position_in_cycle = fract(pattern_coord / dot_spacing) * dot_spacing;
        let distance_from_dot_center = abs(position_in_cycle - dot_spacing * 0.5);
        style_alpha = 1.0 - smoothstep(dot_size * 0.5 - 0.5, dot_size * 0.5 + 0.5, distance_from_dot_center);
    }
    
    // Combine alpha values
    let final_alpha = base_alpha * style_alpha;
    
    return vec4<f32>(input.color.rgb, input.color.a * final_alpha);
}