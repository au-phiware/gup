# GUP-108: Correct SDF Font Atlas Generation

**Status**: Completed **Priority**: High **Component**: Font Atlas / Text
Rendering **Depends On**: None **Blocks**: GUP-107 (may be related to character
positioning issues) **Completed**: 2025-12-31

## Summary

The current font atlas generation uses fontdue's coverage data incorrectly as
signed distance field (SDF) values instead of computing proper multi-channel
signed distance fields (MSDF). This results in poor text rendering quality, lack
of sharp corner preservation, and suboptimal scalability. We will replace the
existing approach with a new implementation that uses `ttf_parser` crate
(v0.21.1) to extract glyph vector outlines and generate true 3-channel MSDF
atlas textures.

## Problem Statement

The current SDF generation in `FontAtlas::generate_sdf()` uses a simplified
approach that treats fontdue's coverage bitmap as distance field data:

```rust
// Current incorrect approach in src/text/atlas.rs
let sample_coverage = bitmap[check_y as usize * width + check_x as usize];
let sample_inside = sample_coverage > 128;
```

This approach does not generate proper MSDF data, which are essential for:

- Sharp corner preservation at all scales
- High-quality antialiasing
- True distance-based rendering effects (glows, outlines, shadows)
- Optimal memory usage with 3-channel representation

## Background

Multi-channel Signed Distance Fields (MSDFs) store the distance to the nearest
edge in three color channels, each representing a different direction. Proper
MSDF generation requires:

1. **Vector outline extraction**: Using `ttf_parser` to get precise glyph
   contours
2. **Distance calculation**: Computing signed distances to glyph boundaries
3. **Channel separation**: Distributing distance information across RGB channels
4. **High precision**: Accurate geometric calculations for smooth rendering
5. **Edge detection**: Proper boundary identification and contour following

Traditional coverage-based approaches cannot achieve the same quality and
efficiency as true MSDF generation.

## Proposed Solution

### MSDF Generation using ttf_parser

Implement proper MSDF generation by extracting glyph vector data from font files
using the `ttf_parser` crate:

- **Vector extraction**: Use `ttf_parser` v0.21.1 to parse glyph outlines from
  TTF/OTF files
- **Geometric processing**: Convert Bézier curves to distance field
  representation
- **3-channel output**: Generate RGB MSDF textures with distance information
- **Precision**: Mathematically exact distance calculations from vector data
- **Performance**: Optimized algorithms for real-time atlas generation

### Implementation Architecture

```rust
// New MSDF generation pipeline
struct MsdfGenerator {
    font_data: Vec<u8>,
    font: ttf_parser::Font<'static>,
    config: MsdfConfig,
}

impl MsdfGenerator {
    fn generate_msdf(&self, glyph_id: GlyphId, size: f32) -> Result<MsdfBitmap, Error> {
        // 1. Extract glyph outline using ttf_parser
        let outline = self.extract_glyph_outline(glyph_id)?;

        // 2. Generate distance field for each channel
        let red_channel = self.compute_distance_field(&outline, Direction::Right)?;
        let green_channel = self.compute_distance_field(&outline, Direction::Up)?;
        let blue_channel = self.compute_distance_field(&outline, Direction::Diagonal)?;

        // 3. Combine channels into MSDF bitmap
        Ok(MsdfBitmap::combine_channels(red_channel, green_channel, blue_channel))
    }
}
```

## Detailed Implementation

### Vector Data Extraction with ttf_parser

```rust
use ttf_parser::{Font, GlyphId, OutlineBuilder};

struct GlyphOutlineBuilder {
    contours: Vec<Contour>,
}

impl OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(Contour::new());
        self.contours.last_mut().unwrap().push_point(Point::new(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.contours.last_mut().unwrap().push_point(Point::new(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // Convert quadratic Bézier to distance field representation
        self.contours.last_mut().unwrap().push_quad_curve(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        // Convert cubic Bézier to distance field representation
        self.contours.last_mut().unwrap().push_cubic_curve(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.contours.last_mut().unwrap().close();
    }
}
```

### MSDF Distance Field Computation

```rust
fn compute_distance_field(&self, outline: &GlyphOutline, direction: Direction) -> DistanceField {
    let mut distance_field = DistanceField::new(self.config.atlas_size);

    for y in 0..self.config.atlas_size {
        for x in 0..self.config.atlas_size {
            let point = Vec2::new(x as f32, y as f32);
            let distance = self.compute_signed_distance(&outline, point, direction);
            let normalized_distance = self.normalize_distance(distance);

            distance_field.set(x, y, normalized_distance);
        }
    }

    distance_field
}

fn compute_signed_distance(&self, outline: &GlyphOutline, point: Vec2, direction: Direction) -> f32 {
    let mut min_distance = f32::INFINITY;
    let mut sign = 1.0;

    // Check distance to all contours
    for contour in &outline.contours {
        for edge in &contour.edges {
            let distance = edge.distance_to_point(point);
            if distance.abs() < min_distance.abs() {
                min_distance = distance;
                sign = if contour.contains_point(point) { 1.0 } else { -1.0 };
            }
        }
    }

    min_distance * sign
}
```

## Success Criteria

- [ ] **Proper MSDF Generation**: 3-channel distance fields accurately represent
      glyph boundaries
- [ ] **Sharp Corner Preservation**: Corners remain sharp at all rendering
      scales
- [ ] **High-Quality Antialiasing**: Smooth text rendering without artifacts
- [ ] **Performance**: Atlas generation time remains acceptable (<200ms for
      ASCII charset)
- [ ] **Memory Efficiency**: 3-channel MSDF uses RGB textures optimally
- [ ] **Compatibility**: Existing text rendering pipeline works with new MSDF
      data
- [ ] **Validation**: MSDF values can be verified against known geometric shapes

## Technical Requirements

### Implementation Details

1. **Replace existing `generate_sdf()` method** in `src/text/atlas.rs`
2. **Add ttf_parser integration** for glyph outline extraction
3. **Implement MSDF algorithms** (distance field computation, channel
   separation)
4. **Add Bézier curve processing** for accurate outline representation
5. **Update texture format** from R8Unorm to Rgba8Unorm for 3-channel MSDF
6. **Create MSDF validation tools** for debugging and quality assurance
7. **Update shader code** to handle 3-channel MSDF rendering

### Configuration Options

```rust
pub struct MsdfConfig {
    pub atlas_size: u32,                    // Size of atlas texture
    pub glyph_size: f32,                    // Size of individual glyphs in pixels
    pub distance_range: f32,                // MSDF distance range in pixels
    pub angle_threshold: f32,               // Threshold for sharp corner detection
    pub edge_coloring_angle_threshold: f32, // Threshold for edge coloring
    pub padding: u32,                       // Padding around glyphs
}
```

### Testing Strategy

- **Unit tests**: Verify MSDF values for simple geometric shapes (circle,
  square, triangle)
- **Visual tests**: Compare rendering quality before/after implementation
- **Performance tests**: Ensure atlas generation time meets requirements
- **Integration tests**: Verify compatibility with existing text rendering
  pipeline
- **Geometric validation**: Test distance field accuracy against known shapes

## Impact Assessment

### Benefits

- **Superior text quality**: Perfect sharp corners and smooth antialiasing at
  all scales
- **Better visual fidelity**: True distance-based rendering effects
- **Standard compliance**: Correct MSDF implementation following established
  methods
- **Memory efficiency**: 3-channel representation more efficient than
  single-channel alternatives
- **Future-proof**: Foundation for advanced text effects and rendering
  techniques

### Risks

- **Implementation complexity**: MSDF algorithms and Bézier processing required
- **Performance impact**: More computationally expensive atlas generation
- **Shader updates**: Text rendering shader needs MSDF support
- **Dependency addition**: New dependency on ttf_parser crate
- **Testing effort**: Extensive validation needed for geometric accuracy

### Dependencies

- `ttf_parser` crate v0.21.1 for font parsing and glyph outline extraction
- Updated shader code for 3-channel MSDF rendering
- Potential updates to texture format and GPU resource management

## Implementation Priority

**Priority**: High - Fundamental text rendering quality improvement that affects
all text display

**Effort Estimate**: 3-4 days implementation + 2 days testing and validation

**Dependencies**: ttf_parser v0.21.1, shader updates, texture format changes

## References

- [MSDF paper: Multi-channel signed distance fields](https://github.com/Chlumsky/msdfgen/files/3050967/thesis.pdf)
- [ttf_parser crate documentation](https://docs.rs/ttf_parser/)
- [MSDF-Atlas-Gen repository](https://github.com/Chlumsky/msdf-atlas-gen)
- [MSDF-Gen repository](https://github.com/Chlumsky/msdfgen)
- [Valve SDF paper](https://steamcdn-a.akamaihd.net/apps/valve/2007/SIGGRAPH2007_AlphaTestedMagnification.pdf)

## Implementation Notes (Completed 2025-12-31)

### What Was Implemented

The MSDF implementation followed the algorithm described in Viktor Chlumsky's
thesis "Shape Decomposition for Multi-channel Distance Fields".

#### Core Components

1. **Proper Bezier Curve Distance Calculations** (`src/text/msdf.rs`)
   - Line segment distance: Analytical solution with perpendicular projection
   - Quadratic Bezier distance: Solved using cubic equation roots
   - Cubic Bezier distance: Iterative subdivision + Newton-Raphson refinement

2. **Edge Coloring Algorithm**
   - Implemented cycling between Yellow (RG), Cyan (GB), and Magenta (RB) colors
   - Adjacent edges at sharp corners receive different colors
   - Corner detection based on direction change angle threshold

3. **True MSDF Generation**
   - Each color channel stores pseudo-distance to the nearest edge of that color
   - Pseudo-distance extends beyond endpoints along tangent directions
   - Proper signed distance with inside/outside determination using cross
     product

4. **Shader Update** (`src/shaders/text.wgsl`)
   - Changed from single-channel (`.r`) to three-channel sampling (`.rgb`)
   - Implemented median-of-three reconstruction for sharp corner preservation
   - Formula: `median(r, g, b) = max(min(a, b), min(max(a, b), c))`

5. **Atlas Integration** (`src/text/atlas.rs`)
   - Updated texture format from R8Unorm to Rgba8Unorm for 3-channel MSDF
   - Integrated MsdfGenerator with glyph caching system
   - Proper coordinate transformation between glyph space and pixel space

### Key Technical Insights

- **Edge Coloring Limitation**: For closed contours with n corners and only 3
  colors, perfect coloring is impossible when n is not divisible by 3. A square
  (4 corners) will have one pair of adjacent edges with the same color.

- **Pseudo-distance vs True Distance**: MSDF uses pseudo-distance (perpendicular
  projection along edge tangent) at endpoints rather than true Euclidean
  distance. This prevents artifacts at sharp corners.

- **Median Reconstruction**: The key insight is that at corners where edge
  colors differ, taking the median of the three channels selects the correct
  distance value, preserving sharpness.

### Files Modified

- `src/text/msdf.rs` - Complete rewrite with proper MSDF algorithm
- `src/shaders/text.wgsl` - Updated to use median-of-three sampling
- `src/text/atlas.rs` - Updated to use RGBA format and new MsdfGenerator

### Tests Added

- `test_contour_edge_coloring` - Verifies triangle edge coloring (3 corners, 3
  colors)
- `test_contour_edge_coloring_square` - Verifies square edge coloring handles
  color wrap-around
- Existing tests updated and passing: 67 text-related tests

### Bug Fix: Orthogonality-Based Edge Comparison (2026-02-06)

A visual artifact bug was discovered where unwanted pixels extended beyond glyph
corners (e.g., 'l' and 'v' glyphs had visible artifacts at their sharp corners).

**Root Cause**: The `msdf_at()` function compared edges using only
`abs(distance)` to determine which edge was "closest" for each color channel.
However, according to Chlumsky's thesis (Algorithm 7, Section 2.4), edge
comparison must also use **orthogonality** as a tie-breaker when distances are
equal.

At corner points, two adjacent edges have equal distances to points along the
corner bisector. Without orthogonality, the wrong edge could be selected for a
color channel, causing the median operation to produce incorrect inside/outside
determinations.

**Fix**: Added `orthogonality` field to `SignedDistance` struct and implemented
`is_closer_than()` method that:

1. Primarily compares by absolute distance
2. Uses orthogonality as tie-breaker when distances are within 1e-6

Orthogonality is computed as the cross product of the normalized tangent
direction and the normalized direction to the query point. Higher orthogonality
means the point is more "directly facing" the edge, making it the preferred edge
when distances are equal.

**Files Modified**:

- `src/text/msdf.rs` - Added orthogonality to SignedDistance and all distance
  functions
- `examples/msdf_debug.rs` - Added visual debugging tool for MSDF investigation
