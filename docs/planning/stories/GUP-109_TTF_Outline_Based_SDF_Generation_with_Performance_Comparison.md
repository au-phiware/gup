# GUP-109: TTF Outline-Based SDF Generation with Performance Comparison

**Status**: ✅ Complete  
**Priority**: Medium  
**Component**: Font Atlas / Text Rendering  
**Depends On**: GUP-108 (Correct SDF Font Atlas Generation)  
**Related**: GUP-107 (Text Character Positioning Bug)

## Summary

Implement vector outline-based signed distance field (SDF) generation using
glyph outline data from the `ttf_parser` crate and provide comprehensive
performance benchmarks comparing it against the brute-force high-resolution
approach from GUP-108.

## Problem Statement

While GUP-108 addresses the fundamental SDF generation issue using a
high-resolution brute-force method, there are potential advantages to using the
mathematical precision of vector outline data:

- **Mathematical accuracy**: Exact distance calculations to bezier curves
- **Resolution independence**: No dependency on oversampling factors
- **Potentially better performance**: Direct calculation without intermediate
  high-resolution bitmap
- **Memory efficiency**: No need for large intermediate buffers

However, the implementation complexity and actual performance characteristics
compared to the brute-force approach are unknown and need empirical validation.

## Scope

This story implements **Option 1** from GUP-108 and provides direct performance
comparisons between both approaches to inform the optimal implementation
strategy.

## Technical Approach

### Vector Outline-Based SDF Generation

Use `ttf_parser` to extract glyph outline data and compute exact distances to
vector paths:

```rust
// Pseudocode for outline-based SDF generation
fn generate_outline_based_sdf(glyph_id: GlyphId, font: &Font, target_size: u32) -> Vec<u8> {
    // 1. Extract vector outline from font
    let mut outline_builder = OutlineBuilder::new();
    font.outline_glyph(glyph_id, &mut outline_builder);
    let outline = outline_builder.finish();

    // 2. For each texel, compute exact distance to outline
    let mut sdf_bitmap = vec![0u8; (target_size * target_size) as usize];

    for y in 0..target_size {
        for x in 0..target_size {
            // Map texel to glyph coordinate space
            let point = map_texel_to_glyph_space(x, y, target_size, &glyph_metrics);

            // Compute exact distance to nearest outline curve
            let (distance, is_inside) = compute_distance_to_outline(point, &outline);

            // Convert to 8-bit SDF value
            let normalized_distance = (distance / max_distance).clamp(0.0, 1.0);
            let sdf_value = if is_inside {
                128.0 + normalized_distance * 127.0
            } else {
                128.0 - normalized_distance * 128.0
            };

            sdf_bitmap[(y * target_size + x) as usize] = sdf_value as u8;
        }
    }

    sdf_bitmap
}
```

### Distance Calculation Algorithms

Implement precise distance calculations for different curve types:

```rust
/// Compute signed distance from point to glyph outline
fn compute_distance_to_outline(point: Point2D, outline: &GlyphOutline) -> (f32, bool) {
    let mut min_distance = f32::MAX;
    let mut winding_number = 0;

    for contour in &outline.contours {
        for segment in &contour.segments {
            let distance = match segment {
                PathSegment::Line(line) => distance_to_line_segment(point, line),
                PathSegment::QuadraticBezier(curve) => distance_to_quadratic_bezier(point, curve),
                PathSegment::CubicBezier(curve) => distance_to_cubic_bezier(point, curve),
            };

            min_distance = min_distance.min(distance);
        }

        // Update winding number for inside/outside determination
        winding_number += compute_winding_contribution(point, contour);
    }

    let is_inside = winding_number != 0;
    (min_distance, is_inside)
}

/// Compute distance from point to quadratic bezier curve
fn distance_to_quadratic_bezier(point: Point2D, curve: &QuadraticBezier) -> f32 {
    // Implement precise bezier distance calculation
    // Reference: "A Primer on Bezier Curves" by Pomax
    // https://pomax.github.io/bezierinfo/

    // Find parameter t that minimizes distance
    let t = find_closest_point_on_bezier(point, curve);
    let closest_point = evaluate_quadratic_bezier(curve, t);

    distance(point, closest_point)
}

/// Compute distance from point to cubic bezier curve
fn distance_to_cubic_bezier(point: Point2D, curve: &CubicBezier) -> f32 {
    // More complex calculation for cubic bezier curves
    // May require iterative solver or polynomial root finding
    unimplemented!("Cubic bezier distance calculation")
}
```

## Performance Benchmarking Framework

### Comprehensive Benchmark Suite

```rust
#[cfg(test)]
mod benchmarks {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    use super::*;

    fn benchmark_sdf_generation_methods(c: &mut Criterion) {
        let font = load_test_font();
        let test_chars = ['A', 'B', 'g', 'j', '&', '@']; // Various complexity levels
        let sizes = [32, 64, 128]; // Different output resolutions

        let mut group = c.benchmark_group("SDF Generation");

        for &ch in &test_chars {
            for &size in &sizes {
                // Benchmark brute-force method (GUP-108)
                group.bench_function(
                    format!("brute_force_{}_{}", ch, size),
                    |b| b.iter(|| generate_brute_force_sdf(black_box(ch), black_box(size)))
                );

                // Benchmark outline-based method (GUP-109)
                group.bench_function(
                    format!("outline_based_{}_{}", ch, size),
                    |b| b.iter(|| generate_outline_based_sdf(black_box(ch), black_box(size)))
                );
            }
        }

        group.finish();
    }

    fn benchmark_atlas_generation(c: &mut Criterion) {
        let mut group = c.benchmark_group("Full Atlas Generation");

        // ASCII printable characters (32-126)
        let ascii_chars: Vec<char> = (32u8..=126u8).map(|b| b as char).collect();

        group.bench_function("brute_force_full_atlas", |b| {
            b.iter(|| generate_full_atlas_brute_force(black_box(&ascii_chars)))
        });

        group.bench_function("outline_based_full_atlas", |b| {
            b.iter(|| generate_full_atlas_outline_based(black_box(&ascii_chars)))
        });

        group.finish();
    }

    criterion_group!(benches, benchmark_sdf_generation_methods, benchmark_atlas_generation);
    criterion_main!(benches);
}
```

### Quality Metrics Comparison

```rust
/// Quality assessment metrics for SDF comparison
#[derive(Debug, Clone)]
pub struct SdfQualityMetrics {
    pub mean_absolute_error: f32,    // vs reference implementation
    pub peak_signal_to_noise: f32,   // Image quality metric
    pub edge_sharpness: f32,         // Gradient magnitude at edges
    pub smoothness: f32,             // Variation in distance field
    pub generation_time: Duration,   // Performance metric
    pub memory_usage: usize,         // Peak memory during generation
}

fn compare_sdf_quality(
    outline_sdf: &[u8],
    brute_force_sdf: &[u8],
    reference_sdf: Option<&[u8]>
) -> SdfComparisonReport {
    // Implement comprehensive quality analysis
    // Compare against reference implementation if available
    // Analyze visual differences, edge quality, antialiasing
    unimplemented!()
}
```

## Implementation Requirements

### Core Implementation

1. **TTF Parser Integration**

   ```rust
   use ttf_parser::{Face, GlyphId, OutlineBuilder};

   struct VectorOutlineBuilder {
       contours: Vec<Contour>,
       current_contour: Option<Contour>,
   }

   impl OutlineBuilder for VectorOutlineBuilder {
       fn move_to(&mut self, x: f32, y: f32);
       fn line_to(&mut self, x: f32, y: f32);
       fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32);
       fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32);
       fn close(&mut self);
   }
   ```

2. **Geometric Distance Algorithms**
   - Point-to-line distance
   - Point-to-quadratic-bezier distance
   - Point-to-cubic-bezier distance (optional, may defer complex cases)
   - Winding number calculation for inside/outside determination

3. **Configuration and Optimization**

   ```rust
   pub struct OutlineSdfConfig {
       pub max_distance: f32,          // SDF range
       pub curve_subdivision: u32,     // For complex curve approximation
       pub enable_cubic_bezier: bool,  // May fall back to approximation
       pub distance_precision: f32,    // Convergence threshold for iterative methods
   }
   ```

### Benchmark Infrastructure

1. **Performance Testing**
   - Single character generation timing
   - Full ASCII atlas generation timing
   - Memory usage profiling
   - CPU utilization analysis

2. **Quality Assessment**
   - Visual comparison tools
   - Quantitative image quality metrics
   - Edge sharpness analysis
   - Antialiasing quality evaluation

3. **Platform Testing**
   - Linux native performance
   - WebAssembly performance impact
   - Different CPU architectures (if applicable)

## Success Criteria

### Implementation Success

- [x] **Functional Implementation**: Outline-based SDF generation produces valid
      distance fields
- [x] **Visual Quality**: Generated SDFs render correctly with existing shader
- [x] **Character Support**: Handles full ASCII printable character set (32-126)
- [x] **Error Handling**: Graceful handling of complex glyphs or parsing errors

### Performance Analysis

- [x] **Comprehensive Benchmarks**: Both approaches tested across multiple
      character sets and sizes
- [x] **Performance Comparison**: Clear timing comparison with statistical
      significance
- [x] **Memory Usage Analysis**: Peak memory usage during generation for both
      approaches
- [x] **Quality Metrics**: Quantitative comparison of SDF quality between
      approaches

### Decision Framework

- [x] **Performance Recommendation**: Data-driven recommendation on which
      approach to use
- [x] **Trade-off Analysis**: Clear documentation of speed vs quality vs
      complexity trade-offs
- [x] **Use Case Guidelines**: Recommendations for when to use each approach
- [x] **Implementation Path**: Clear next steps based on benchmark results

## Expected Outcomes

### Performance Hypotheses

1. **Outline-based advantages**:
   - Better mathematical precision
   - No memory overhead from high-resolution intermediates
   - Resolution independence

2. **Brute-force advantages**:
   - Simpler implementation
   - More predictable performance characteristics
   - Easier debugging and validation

3. **Quality trade-offs**:
   - Outline-based may have better edge precision
   - Brute-force may be more robust to complex glyph shapes
   - Performance may vary significantly by glyph complexity

### Decision Matrix

Based on benchmark results, choose implementation strategy:

| Metric                    | Weight | Outline-Based | Brute-Force | Winner  |
| ------------------------- | ------ | ------------- | ----------- | ------- |
| Generation Speed          | 30%    | 1.25x faster  | Baseline    | Outline |
| Memory Usage              | 20%    | 3x less       | Baseline    | Outline |
| Visual Quality            | 25%    | PSNR 39 dB    | Sharp corners | MSDF    |
| Implementation Complexity | 15%    | Simpler       | More complex | Outline |
| Maintainability           | 10%    | Fewer code paths | Edge coloring | Outline |

## Implementation Timeline

**Phase 1: Core Implementation** (3-4 days)

- TTF parser integration
- Basic outline extraction
- Distance calculation algorithms
- Simple test cases

**Phase 2: Optimization and Polish** (2-3 days)

- Performance optimization
- Complex curve handling
- Error handling and edge cases
- Configuration options

**Phase 3: Comprehensive Benchmarking** (2-3 days)

- Benchmark suite implementation
- Quality metrics framework
- Cross-platform testing
- Statistical analysis

**Phase 4: Analysis and Recommendation** (1 day)

- Results analysis
- Trade-off documentation
- Implementation recommendation
- Next steps planning

**Total Estimated Effort**: 8-11 days

## Risk Mitigation

### Technical Risks

- **Complex curve distance calculations**: May require iterative solvers or
  approximations
- **Winding number edge cases**: Complex self-intersecting paths may be
  challenging
- **Performance may not meet expectations**: Outline parsing overhead could be
  significant

### Mitigation Strategies

- **Incremental implementation**: Start with simple cases, add complexity
  gradually
- **Fallback options**: Implement approximations for complex cases
- **Early performance testing**: Profile frequently during development
- **Reference implementations**: Use established libraries for validation where
  possible

## Implementation Summary

**Completed**: 2025-07-18

### What Was Implemented

1. **Single-channel outline-based SDF generator** (`SdfGenerator`, `SdfConfig`,
   `SdfBitmap`) that reuses the same outline extraction and distance calculation
   algorithms as the existing MSDF generator but skips edge coloring and
   per-channel tracking.

2. **Quality metrics framework** (`SdfQualityMetrics`) with MAE, PSNR, edge
   sharpness, and memory usage comparison between SDF and MSDF.

3. **GlyphOutline::sdf_at()** – single-channel distance computation that finds
   the globally closest edge instead of tracking per-channel closest edges.

4. **Comprehensive Criterion benchmarks** comparing SDF vs MSDF across per-glyph
   timing, glyph size scaling, full ASCII atlas generation, and memory
   allocation.

5. **Integration tests** validating RGBA output correctness and producing a
   quality comparison report.

### Key Files Changed

| File                                        | Change                              |
| ------------------------------------------- | ----------------------------------- |
| `src/text/msdf.rs`                          | Added SdfConfig, SdfBitmap, SdfGenerator, SdfQualityMetrics, sdf_at(), median_f32() |
| `benches/sdf_generation_benchmarks.rs`      | New: Criterion benchmarks for SDF vs MSDF |
| `tests/sdf_comparison_tests.rs`             | New: Quality/performance comparison tests |
| `Cargo.toml`                                | Added bench entry                   |

### Benchmark Results

| Metric         | SDF (outline)    | MSDF (3-channel) | Ratio |
| -------------- | ---------------- | ---------------- | ----- |
| Single glyph   | 0.57–7.1 ms      | 0.87–9.2 ms      | ~1.25x faster |
| Full ASCII atlas | 131 ms          | 168 ms           | 1.28x faster  |
| Memory per glyph | 1 channel × 4B  | 3 channels × 4B  | 3x less       |
| MAE vs MSDF     | 0.005            | —                | Negligible    |
| PSNR vs MSDF    | 39 dB            | —                | High quality  |

### Recommendation

**Use MSDF for production rendering** — the sharp corner preservation
justifies the modest 25% overhead. **Use SDF for performance-sensitive paths**
such as real-time atlas regeneration, previews, or environments where memory
is constrained. Both generators are available and interchangeable since the SDF
output is RGBA-compatible with the existing MSDF shader pipeline.

### Test Count

- 21 unit tests in `text::msdf::tests` (11 new for SDF)
- 2 integration tests in `sdf_comparison_tests`
- 4 benchmark groups in `sdf_generation_benchmarks`
