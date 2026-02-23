// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// Pattern generation functions for accessibility and visual differentiation.
// These patterns provide color-independent ways to distinguish data categories.

/// Pattern type enumeration
/// Must match the Pattern enum in Rust code
const PATTERN_SOLID: u32 = 0u;
const PATTERN_DOTS: u32 = 1u;
const PATTERN_LINES: u32 = 2u;
const PATTERN_CROSSHATCH: u32 = 3u;

/// Pattern parameters uniform buffer
struct PatternUniforms {
    /// Pattern type (PATTERN_SOLID, PATTERN_DOTS, etc.)
    pattern_type: u32,
    /// Pattern spacing in pixels
    spacing: f32,
    /// Pattern angle in radians (for lines)
    angle: f32,
    /// Foreground color (pattern color)
    foreground_color: vec4<f32>,
    /// Background color (base color)
    background_color: vec4<f32>,
    /// Line thickness for line patterns
    thickness: f32,
    /// Padding for alignment
    _padding: vec2<f32>,
}

/// Generate a solid pattern (no pattern, just solid color)
fn pattern_solid(
    position: vec2<f32>,
    params: PatternUniforms
) -> vec4<f32> {
    return params.foreground_color;
}

/// Generate a dot pattern
/// Creates evenly spaced dots with anti-aliasing
fn pattern_dots(
    position: vec2<f32>,
    params: PatternUniforms
) -> vec4<f32> {
    let spacing = params.spacing;
    
    // Calculate position within pattern grid
    let grid_pos = position / spacing;
    let cell = floor(grid_pos);
    let cell_offset = fract(grid_pos) - 0.5;
    
    // Distance from cell center
    let dist = length(cell_offset * spacing);
    
    // Dot radius (40% of spacing)
    let dot_radius = spacing * 0.2;
    
    // Anti-aliased edge
    let edge_width = 1.0;
    let alpha = 1.0 - smoothstep(dot_radius - edge_width, dot_radius + edge_width, dist);
    
    // Mix foreground and background colors based on alpha
    return mix(params.background_color, params.foreground_color, alpha);
}

/// Generate a line pattern
/// Creates parallel lines at the specified angle
fn pattern_lines(
    position: vec2<f32>,
    params: PatternUniforms
) -> vec4<f32> {
    let spacing = params.spacing;
    let angle = params.angle;
    let thickness = params.thickness;
    
    // Rotate position by angle
    let cos_angle = cos(angle);
    let sin_angle = sin(angle);
    let rotated_x = position.x * cos_angle - position.y * sin_angle;
    
    // Calculate position within pattern
    let pattern_pos = fract(rotated_x / spacing);
    
    // Distance from line center (0.5 is the center of each stripe)
    let dist_from_line = abs(pattern_pos - 0.5) * spacing;
    
    // Half thickness
    let half_thickness = thickness * 0.5;
    
    // Anti-aliased edge
    let edge_width = 1.0;
    let alpha = 1.0 - smoothstep(half_thickness - edge_width, half_thickness + edge_width, dist_from_line);
    
    // Mix foreground and background colors based on alpha
    return mix(params.background_color, params.foreground_color, alpha);
}

/// Generate a crosshatch pattern
/// Creates two perpendicular sets of lines
fn pattern_crosshatch(
    position: vec2<f32>,
    params: PatternUniforms
) -> vec4<f32> {
    let spacing = params.spacing;
    let thickness = params.thickness;
    
    // Horizontal lines
    let horizontal_pos = fract(position.y / spacing);
    let h_dist = abs(horizontal_pos - 0.5) * spacing;
    let h_alpha = 1.0 - smoothstep(thickness * 0.25, thickness * 0.25 + 1.0, h_dist);
    
    // Vertical lines
    let vertical_pos = fract(position.x / spacing);
    let v_dist = abs(vertical_pos - 0.5) * spacing;
    let v_alpha = 1.0 - smoothstep(thickness * 0.25, thickness * 0.25 + 1.0, v_dist);
    
    // Combine both directions
    let combined_alpha = max(h_alpha, v_alpha);
    
    // Mix foreground and background colors based on alpha
    return mix(params.background_color, params.foreground_color, combined_alpha);
}

/// Main pattern generation function
/// Dispatches to the appropriate pattern function based on pattern_type
fn apply_pattern(
    position: vec2<f32>,
    params: PatternUniforms
) -> vec4<f32> {
    switch params.pattern_type {
        case PATTERN_DOTS: {
            return pattern_dots(position, params);
        }
        case PATTERN_LINES: {
            return pattern_lines(position, params);
        }
        case PATTERN_CROSSHATCH: {
            return pattern_crosshatch(position, params);
        }
        default: {
            // PATTERN_SOLID or unknown
            return pattern_solid(position, params);
        }
    }
}
