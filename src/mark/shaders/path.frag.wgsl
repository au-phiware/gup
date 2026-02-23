// Fragment shader for standard path mark rendering
// Simple fill rendering - future versions will implement SDF-based rendering

struct FragmentInput {
    @location(0) world_position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width: f32,
}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {
    // Simple fill rendering - in future, implement proper SDF-based path rendering
    return input.fill_color;
}
