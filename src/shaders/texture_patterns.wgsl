// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// Texture-based pattern sampling for performance comparison with procedural patterns.
// This shader samples pre-rendered pattern textures instead of generating patterns procedurally.

/// Texture pattern parameters uniform buffer
struct TexturePatternUniforms {
    /// Foreground color (pattern color)
    foreground_color: vec4<f32>,
    /// Background color (base color)
    background_color: vec4<f32>,
    /// Pattern scaling factor
    scale: f32,
    /// Padding for alignment
    _padding: vec3<f32>,
}

/// Sample texture-based pattern
/// This function samples a pre-rendered pattern texture and applies colors
fn sample_texture_pattern(
    position: vec2<f32>,
    texture: texture_2d<f32>,
    pattern_sampler: sampler,
    params: TexturePatternUniforms
) -> vec4<f32> {
    // Calculate texture coordinates with scaling
    let tex_coords = (position * params.scale) / 512.0; // Normalize to texture size
    
    // Sample the pattern texture
    let pattern_alpha = textureSample(texture, pattern_sampler, tex_coords);
    
    // Mix foreground and background colors based on texture alpha
    return mix(params.background_color, params.foreground_color, pattern_alpha.a);
}

/// Sample texture-based pattern with tiling
/// This version ensures seamless tiling by using fract() on texture coordinates
fn sample_texture_pattern_tiled(
    position: vec2<f32>,
    texture: texture_2d<f32>,
    pattern_sampler: sampler,
    params: TexturePatternUniforms
) -> vec4<f32> {
    // Calculate texture coordinates with scaling and tiling
    let tex_coords = fract((position * params.scale) / 512.0); // Ensure tiling
    
    // Sample the pattern texture
    let pattern_alpha = textureSample(texture, pattern_sampler, tex_coords);
    
    // Mix foreground and background colors based on texture alpha
    return mix(params.background_color, params.foreground_color, pattern_alpha.a);
}
