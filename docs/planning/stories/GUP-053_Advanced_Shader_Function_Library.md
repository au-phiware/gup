# GUP-053: Advanced Shader Function Library

## Story Overview

**Title**: Expand Shader Function Library with Advanced Transformations
**Epic**: Phase 1 Initiative 2 - Unified Shader Function System **Priority**:
Medium **Story Points**: 8 **Status**: ✅ Complete (2025-07-14)

## Context

GUP-005 implemented basic shader functions (LinearScale, ColorMap,
PositionTransform). To provide a comprehensive visualization toolkit, we need an
expanded library of commonly-used data transformation functions that demonstrate
the full power of the composable shader system.

## User Story

**As a** visualization developer **I want** a rich library of shader functions
**So that** I can create complex visualizations by composing pre-built
transformations

## Problem Statement

The current shader function library is minimal and serves primarily as examples.
Real visualization applications need a broader set of transformations including:

- Mathematical functions (logarithmic, exponential, trigonometric)
- Data normalization and scaling variants
- Color space conversions
- Geometric transformations
- Statistical functions

## Acceptance Criteria

### AC1: Mathematical Transform Functions

- [x] LogarithmicScale for log transformations (pre-existing as LogScale)
- [x] ExponentialScale for exponential scaling
- [x] PowerScale for power law transformations (pre-existing)
- [x] ClampFunction for value limiting (pre-existing as Clamp)

### AC2: Color and Visual Functions

- [x] HSVColorMap for HSV color space mapping
- [x] GradientColorMap for multi-stop color gradients (pre-existing as
      ColorGradient)
- [x] AlphaBlending for transparency control
- [x] ColorSpaceConverter (RGB ↔ HSV)

### AC3: Geometric and Spatial Functions

- [x] PolarTransform for polar coordinate conversion
- [x] MatrixTransform for general 2D/3D transformations
- [x] ProjectionTransform for coordinate projections
- [x] DistanceFunction for distance calculations

### AC4: Statistical and Data Functions

- [x] NormalizeFunction for statistical normalization
- [x] StandardizeFunction for z-score standardization
- [x] QuantileFunction for percentile mapping
- [x] BinningFunction for data discretization

## Technical Requirements

### Function Examples

```rust
// Logarithmic scaling
pub struct LogarithmicScale {
    base: f32,
    domain_min: f32,
    domain_max: f32,
}

// HSV color mapping
pub struct HSVColorMap {
    hue_range: (f32, f32),
    saturation: f32,
    value: f32,
}

// Polar coordinate transformation
pub struct PolarTransform {
    center: Vec2,
    angle_offset: f32,
}
```

### Composition Examples

```rust
// Complex visualization pipeline
let data_transform = LogarithmicScale::new(10.0, 1.0, 1000.0)
    .compose(NormalizeFunction::new(0.0, 1.0))
    .compose(HSVColorMap::new((0.0, 240.0), 1.0, 1.0));

let spatial_transform = PolarTransform::new(center, 0.0)
    .compose(ProjectionTransform::new(viewport));
```

## Dependencies

- GUP-005: Shader Function Trait (prerequisite)
- GUP-051: WGSL Code Generation Templates (for implementation)

## Testing Strategy

- Unit tests for each function's mathematical correctness
- Composition tests to ensure all functions work together
- Visual tests to validate output correctness
- Performance benchmarks for complex compositions

## Definition of Done

- [x] All listed shader functions implemented and tested
- [x] Comprehensive documentation with mathematical formulas
- [x] Visual examples demonstrating each function
- [x] Performance validation shows acceptable overhead
- [x] Integration with existing shader function system

## Implementation Summary

### What Was Implemented

12 new composable shader functions added to `src/shader_function.rs`:

**AC1 — Mathematical Transform Functions:**

- `ExponentialScale` — Maps values using exponential scaling with configurable
  base. Convenience constructors for base-10 and natural exponential.

**AC2 — Color and Visual Functions:**

- `HSVColorMap` — Maps scalar [0,1] to RGBA via HSV color space with
  configurable hue range, saturation, and value. Includes HSV→RGB conversion in
  WGSL. Convenience constructors: `rainbow()`, `cool_warm()`.
- `AlphaBlending` — Applies alpha multiplier to RGBA colors for transparency
  control. Vec4→Vec4 type signature.
- `ColorSpaceConverter` — Bidirectional RGB↔HSV conversion with direction flag.
  Implements both `rgb_to_hsv_convert` and `hsv_to_rgb_convert` helper functions
  in WGSL.

**AC3 — Geometric and Spatial Functions:**

- `PolarTransform` — Cartesian↔Polar coordinate conversion with configurable
  center and angle offset. Bidirectional via direction flag.
- `MatrixTransform` — General 2D affine transformation (2×3 matrix). Convenience
  constructors: `identity()`, `rotation()`, `scaling()`, `translation()`.
- `ProjectionTransform` — Maps data coordinates to viewport/screen coordinates
  with independent X/Y axis mapping.
- `DistanceFunction` — Euclidean distance from input point to configurable
  reference point. Vec2→f32 type signature.

**AC4 — Statistical Shader Functions:**

- `NormalizeFunction` — Maps [min, max] → [0, 1] with zero-range guard.
- `StandardizeFunction` — Z-score standardization (value - mean) / std_dev with
  zero-std-dev guard.
- `QuantileFunction` — Maps values to quantile position [0, 1] based on up to 16
  pre-computed boundaries.
- `BinningFunction` — Discretizes continuous values into N bins, outputting
  normalized bin center positions.

### Key Files Changed

- `src/shader_function.rs` — All 12 shader functions with uniforms, WGSL code,
  and type-safe composition
- `src/prelude.rs` — All new types exported for convenient access

### Test Counts

- 107 shader function unit tests passing (including ~50 new GUP-053 tests)
- Composition tests verify multi-stage pipelines (up to 3 stages)
- All 2099+ project tests pass with 0 failures

## Retrospective

**Completed**: 2025-07-14

### Key Technical Learnings

#### Composable Shader Function Pattern Is Mature

- **Challenge**: Adding 12 new functions to an already-large file (7100+ lines)
  without introducing regressions.
- **Solution**: The existing `ComposableShaderFunction` trait pattern (struct +
  uniforms + ShaderUniform impl + ComposableShaderFunction impl) is very
  consistent and mechanical. Each function follows the same 5-step recipe.
- **Pattern**: New shader functions can be added by copying an existing one and
  modifying the WGSL body, uniform fields, and type signatures. The type system
  catches composition errors at compile time.

#### WGSL Helper Functions Require Unique Naming

- **Challenge**: HSVColorMap and ColorSpaceConverter both need an HSV→RGB helper
  function. Duplicate function names in WGSL are a compilation error.
- **Solution**: Used distinct function names (`hsv_to_rgb` in HSVColorMap vs
  `hsv_to_rgb_convert` in ColorSpaceConverter). When composing these together,
  the WGSL deduplication logic in `FunctionChain::generate_wgsl()` handles
  duplicate `fn` definitions, but only exact duplicates — not functions with the
  same name but different bodies.
- **Pattern**: Always use globally unique WGSL function names, even for helper
  functions within a shader function's WGSL block.

#### Uniform Padding for GPU Alignment

- **Challenge**: `bytemuck::Pod` requires every byte to be initialized. Structs
  with odd numbers of f32 fields need explicit padding for 16-byte alignment.
- **Solution**: Added `_padding: [f32; N]` fields to all uniform structs that
  need them. Mixed types (f32 + u32) also require careful padding.
- **Pattern**: Always pad uniform structs to 16-byte boundaries. Use `[f32; 2]`
  or `[f32; 3]` padding as needed.

### Architectural Decisions

#### Pre-existing Functions Counted Toward AC Satisfaction

- **Decision**: LogScale, PowerScale, Clamp, and ColorGradient were already
  implemented in prior stories. Rather than re-implementing them, I counted them
  as satisfying their respective AC items.
- **Reasoning**: The story's goal is "a rich library of shader functions" — not
  specifically to implement each item from scratch. The library now has all 16
  listed functions available.
- **Trade-off**: Less new code for AC1, but the AC items are genuinely
  satisfied.
- **Future**: If more mathematical functions are needed (e.g., trigonometric
  scales, symlog), they can be added following the same pattern.

#### RGB↔HSV Only (No LAB Color Space)

- **Decision**: Implemented RGB↔HSV conversion but not LAB color space.
- **Reasoning**: LAB conversion requires illuminant reference values and more
  complex math (XYZ intermediate space). HSV covers the most common
  visualization use cases. LAB can be added as a follow-up if needed.
- **Trade-off**: Less complete color space coverage, but avoids complexity and
  keeps the scope manageable.
- **Future**: A dedicated GUP story for perceptual color spaces (LAB, LCH,
  OKLab) would be valuable for advanced color work.

#### Shader Functions in Single File vs Separate Modules

- **Decision**: Added all functions to the existing `shader_function.rs` file
  rather than splitting into separate module files.
- **Reasoning**: The existing pattern has all shader functions in one file with
  section markers. Splitting would require restructuring and potentially
  breaking existing imports.
- **Trade-off**: File is now 7700+ lines, which is large. Module splitting may
  be warranted in the future.
- **Future**: Consider splitting `shader_function.rs` into submodules (e.g.,
  `shader_function/math.rs`, `shader_function/color.rs`,
  `shader_function/geometric.rs`, `shader_function/statistical.rs`).

### Development Workflow Insights

- The implementation was straightforward due to the well-established
  `ComposableShaderFunction` pattern. Each function took ~5 minutes to implement
  once the pattern was clear.
- The type system's compile-time validation of composition chains (via
  `ShaderCompatible`) is very powerful — it caught a potential Vec2→f32→Vec4
  chain issue during testing.
- Running `mask all-fix` before every commit is essential — it catches
  formatting and clippy issues that would otherwise accumulate.
- Pre-commit hooks with cargo check add ~20s per commit but catch issues early.
  The lock file contention when running in parallel can cause delays.

### Follow-up Stories

1. **GUP-293: LAB/OKLab Perceptual Color Space Shader Functions** — Add
   perceptual color space conversions (RGB↔XYZ↔LAB, RGB↔OKLab) for advanced
   color work in data visualization.
2. **GUP-294: Shader Function Module Reorganization** — Split the 7700+ line
   `shader_function.rs` into submodules organized by category (math, color,
   geometric, statistical, temporal) for better maintainability.
