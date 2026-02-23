// Fragment shader for rectangle mark rendering with pattern support
// Integrates patterns for accessibility and color-independent encoding

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
    
    // Apply pattern if pattern mode is enabled (pattern_type != 0)
    var final_color: vec4<f32>;
    
    if pattern.pattern_type != PATTERN_SOLID {
        // Use world position for pattern to ensure consistency across instances
        let pattern_color = apply_pattern(input.world_position);
        
        // Handle stroke rendering with pattern
        if (input.stroke_width > 0.0) {
            let inner_alpha = smoothstep(-edge_width, edge_width, inner_distance);
            let stroke_alpha = outer_alpha * (1.0 - inner_alpha);
            let fill_alpha = outer_alpha * inner_alpha;
            
            // Apply pattern to both fill and stroke
            let stroke_with_pattern = pattern_color * stroke_alpha;
            let fill_with_pattern = pattern_color * fill_alpha;
            
            final_color = stroke_with_pattern + fill_with_pattern;
            final_color.a = max(stroke_alpha, fill_alpha);
        } else {
            // No stroke - just fill with pattern
            final_color = vec4<f32>(pattern_color.rgb, pattern_color.a * outer_alpha);
        }
    } else {
        // Standard rendering without pattern
        if (input.stroke_width > 0.0) {
            let inner_alpha = smoothstep(-edge_width, edge_width, inner_distance);
            let stroke_alpha = outer_alpha * (1.0 - inner_alpha);
            let fill_alpha = outer_alpha * inner_alpha;
            
            let stroke_part = input.stroke_color * stroke_alpha;
            let fill_part = input.fill_color * fill_alpha;
            
            final_color = stroke_part + fill_part;
            final_color.a = max(stroke_alpha, fill_alpha);
        } else {
            final_color = vec4<f32>(input.fill_color.rgb, input.fill_color.a * outer_alpha);
        }
    }
    
    return final_color;
}
