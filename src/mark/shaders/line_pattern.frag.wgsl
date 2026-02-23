// Fragment shader for line mark rendering with pattern support
// Integrates patterns for accessibility and color-independent encoding

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) width: f32,
    @location(4) style: u32,
    @location(5) line_length: f32,
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
    // Calculate distance from line center
    let distance_from_center = abs(input.local_position.y);
    
    // Calculate base alpha for line width with anti-aliasing
    let half_width = 0.5;
    let edge_width = fwidth(distance_from_center) * 0.5;
    let base_alpha = 1.0 - smoothstep(half_width - edge_width, half_width + edge_width, distance_from_center);
    
    // Apply line style patterns (dashed, dotted, etc)
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
    
    // Apply pattern if pattern mode is enabled
    var final_color: vec4<f32>;
    
    if pattern.pattern_type != PATTERN_SOLID {
        // Use world position for pattern to ensure consistency
        let pattern_color = apply_pattern(input.world_position);
        
        // Combine alpha values
        let final_alpha = base_alpha * style_alpha;
        
        final_color = vec4<f32>(pattern_color.rgb, pattern_color.a * final_alpha);
    } else {
        // Standard rendering without accessibility pattern
        let final_alpha = base_alpha * style_alpha;
        final_color = vec4<f32>(input.color.rgb, input.color.a * final_alpha);
    }
    
    return final_color;
}
