// Fragment shader for path mark rendering with pattern support
// Integrates patterns for accessibility and color-independent encoding

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width: f32,
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
    // Apply pattern if pattern mode is enabled (pattern_type != 0)
    if pattern.pattern_type != PATTERN_SOLID {
        // Use world position for pattern to ensure consistency across instances
        let pattern_color = apply_pattern(input.world_position);
        
        // For paths, apply pattern to the fill
        // In future, when stroke rendering is implemented, handle stroke separately
        return pattern_color;
    } else {
        // Standard rendering without pattern - just return fill color
        return input.fill_color;
    }
}
