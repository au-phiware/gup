# GUP-108: Correct SDF Font Atlas Generation

**Status**: Open  
**Priority**: High  
**Component**: Font Atlas / Text Rendering  
**Depends On**: None  
**Blocks**: GUP-107 (may be related to character positioning issues)

## Summary

The current font atlas generation incorrectly uses fontdue's coverage data
directly as signed distance field (SDF) values instead of computing proper
distance fields. This results in poor text rendering quality and may contribute
to character positioning bugs.

## Problem Statement

The current SDF generation in `FontAtlas::generate_sdf()` uses a simplified
approach that treats fontdue's coverage bitmap as distance field data:

```rust
// Current incorrect approach in src/text/atlas.rs
let sample_coverage = bitmap[check_y as usize * width + check_x as usize];
let sample_inside = sample_coverage > 128;
```

This approach does not generate true signed distance fields, which are essential
for high-quality SDF text rendering with proper antialiasing, scalability, and
visual fidelity.

## Background

Signed Distance Fields (SDFs) store the distance from each texel to the nearest
edge of the glyph outline. Proper SDF generation requires:

1. **Distance calculation**: Each texel stores the distance to the nearest
   opposite-state texel
2. **Sign information**: Inside vs outside the glyph boundary
3. **High precision**: Accurate distance measurements for smooth antialiasing
4. **Edge detection**: Proper boundary identification for distance calculations

Coverage data from font rasterization is fundamentally different from distance
field data and cannot be used directly.

## Proposed Solutions

### Option 1: TTF Parser Outline-Based SDF Generation

Use glyph outline data from the underlying `ttf_parser` crate to calculate exact
distances:

- **Approach**: Extract vector outline data from font files
- **Method**: Calculate distance from each texel to nearest curve segment
- **Precision**: Mathematically exact distance calculations
- **Complexity**: Requires bezier curve distance algorithms
- **Performance**: More computationally expensive but higher quality

### Option 2: High-Resolution Brute-Force Method

Implement the approach described in Valve's SIGGRAPH 2007 paper "Improved
Alpha-Tested Magnification for Vector Textures and Special Effects":

- **Approach**: Rasterize glyphs at high resolution, then downsample with
  distance calculation
- **Method**: For each output texel, search local neighborhood for nearest
  opposite-state texel
- **Precision**: Quality depends on oversampling ratio
- **Complexity**: Simpler algorithm, well-documented approach
- **Performance**: "Negligible execution time" according to paper due to limited
  search radius

## Detailed Implementation: Option 2 (Recommended)

Based on the Valve paper's brute-force approach:

```rust
// Pseudocode for proper SDF generation
fn generate_proper_sdf(glyph_char: char, target_size: u32) -> Vec<u8> {
    let oversample_factor = 8; // Render at 8x resolution
    let high_res_size = target_size * oversample_factor;

    // 1. Rasterize glyph at high resolution
    let (metrics, high_res_bitmap) = font.rasterize(glyph_char, font_size * oversample_factor);

    // 2. For each output texel, compute distance to nearest edge
    let mut sdf_bitmap = vec![0u8; (target_size * target_size) as usize];

    for out_y in 0..target_size {
        for out_x in 0..target_size {
            // Map output texel to high-res coordinates
            let center_x = (out_x as f32 + 0.5) * oversample_factor as f32;
            let center_y = (out_y as f32 + 0.5) * oversample_factor as f32;

            // Determine if center point is inside or outside
            let is_inside = sample_high_res_bitmap(center_x, center_y, &high_res_bitmap);

            // Search neighborhood for nearest opposite-state texel
            let distance = find_nearest_edge_distance(
                center_x, center_y,
                &high_res_bitmap,
                is_inside,
                max_search_radius
            );

            // Convert to 8-bit SDF value
            let normalized_distance = (distance / max_distance).clamp(0.0, 1.0);
            let sdf_value = if is_inside {
                128.0 + normalized_distance * 127.0  // 128-255: inside
            } else {
                128.0 - normalized_distance * 128.0  // 0-127: outside
            };

            sdf_bitmap[(out_y * target_size + out_x) as usize] = sdf_value as u8;
        }
    }

    sdf_bitmap
}
```

## Success Criteria

- [ ] **Proper SDF Generation**: Distance fields accurately represent glyph
      boundaries
- [ ] **Visual Quality**: Text rendering shows smooth antialiasing at all scales
- [ ] **Performance**: Atlas generation time remains acceptable (<100ms for
      ASCII charset)
- [ ] **Compatibility**: Existing SDF shader continues to work with new atlas
      data
- [ ] **Validation**: SDF values can be verified against known test glyphs
- [ ] **Memory Efficiency**: Atlas size and memory usage remain reasonable

## Technical Requirements

### Implementation Details

1. **Replace existing `generate_sdf()` method** in `src/text/atlas.rs`
2. **Add proper distance field algorithms** (brute-force neighborhood search)
3. **Implement high-resolution rasterization** with configurable oversample
   factor
4. **Add SDF validation tools** for debugging and quality assurance
5. **Update SDF parameters** (range, scale) based on new generation method

### Configuration Options

```rust
pub struct SdfConfig {
    pub oversample_factor: u32,    // Default: 8x
    pub max_distance: f32,         // SDF range in pixels
    pub search_radius: u32,        // Brute-force search limit
    pub edge_threshold: u8,        // Coverage threshold for edge detection
}
```

### Testing Strategy

- **Unit tests**: Verify SDF values for simple geometric shapes (circle, square)
- **Visual tests**: Compare rendering quality before/after implementation
- **Performance tests**: Ensure atlas generation time meets requirements
- **Integration tests**: Verify compatibility with existing text rendering
  pipeline

## Impact Assessment

### Benefits

- **Improved text quality**: Proper antialiasing and scalability
- **Better visual fidelity**: Smooth text rendering at all sizes
- **Standard compliance**: Correct SDF implementation following established
  methods
- **Potential bug fixes**: May resolve character positioning issues in GUP-107

### Risks

- **Performance impact**: More expensive atlas generation
- **Implementation complexity**: Proper distance field algorithms required
- **Compatibility**: May require shader parameter adjustments
- **Testing effort**: Extensive validation needed for quality assurance

## References

- [Valve SIGGRAPH 2007: Improved Alpha-Tested Magnification for Vector Textures](https://steamcdn-a.akamaihd.net/apps/valve/2007/SIGGRAPH2007_AlphaTestedMagnification.pdf)
- [fontdue crate documentation](https://docs.rs/fontdue/)
- [ttf_parser crate documentation](https://docs.rs/ttf-parser/)

## Implementation Priority

**Priority**: High - Fundamental text rendering quality issue that affects all
text display

**Effort Estimate**: 2-3 days implementation + 1 day testing and validation

**Dependencies**: None - can be implemented independently
