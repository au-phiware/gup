# GUP-294: Shader Function Module Reorganization

## Story Overview

**Title**: Shader Function Module Reorganization **Epic**: Phase 1 Initiative
2 - Unified Shader Function System **Priority**: Low **Story Points**: 3
**Status**: ✅ Complete (2025-03-05)

## Context

The `src/shader_function.rs` file has grown to 7700+ lines after GUP-053 added
12 new shader functions. While the code is well-organized with section markers,
the single-file approach makes navigation and maintenance increasingly
difficult. A module split would improve developer experience without changing
any public APIs.

## User Story

**As a** library maintainer **I want** shader functions organized into
submodules **So that** I can navigate, maintain, and extend the function library
more easily

## Acceptance Criteria

- [x] Split `shader_function.rs` into category-based submodules
- [x] Submodules: `math.rs`, `color.rs`, `geometric.rs`, `statistical.rs`,
      `temporal.rs`, `core.rs`
- [x] All public API types remain accessible from `shader_function::` path
- [x] All existing tests continue to pass without modification
- [x] No breaking changes to downstream code or examples
- [x] Re-exports in `shader_function/mod.rs` maintain backward compatibility

## Technical Tasks

1. Create `src/shader_function/` directory with `mod.rs`
2. Move core traits (ShaderType, ComposableShaderFunction, FunctionChain) to
   `core.rs`
3. Move math functions (LinearScale, LogScale, PowerScale, ExponentialScale,
   Clamp, Threshold, SmoothStep) to `math.rs`
4. Move color functions (ColorMap, ColorGradient, HSVColorMap, AlphaBlending,
   ColorSpaceConverter) to `color.rs`
5. Move geometric functions (PositionTransform, PolarTransform, MatrixTransform,
   ProjectionTransform, DistanceFunction) to `geometric.rs`
6. Move statistical functions (NormalizeFunction, StandardizeFunction,
   QuantileFunction, BinningFunction, StatisticsCompute, etc.) to
   `statistical.rs`
7. Move temporal functions (TemporalInterpolation, Easing, KeyframeAnimation,
   etc.) to `temporal.rs`
8. Update re-exports to maintain backward compatibility
9. Verify all tests and examples compile

## Dependencies

- GUP-053: Advanced Shader Function Library ✅

## Testing Strategy

- All existing tests must pass without modification
- All examples must compile without changes
- Module path tests to verify re-exports work correctly

## Risk Assessment

- **Low**: Pure refactoring with no behavior changes
- **Low**: Re-exports ensure backward compatibility

## Definition of Done

- [x] All code migrated to submodules
- [x] All tests pass
- [x] All examples compile
- [x] No breaking API changes

## Implementation Summary

Split the monolithic `shader_function.rs` (12,141 lines) into a module
directory with six category-based submodules:

| Submodule        | Lines | Contents                                                                   |
| ---------------- | ----- | -------------------------------------------------------------------------- |
| `core.rs`        | 1,711 | ShaderType, Vec3/Vec4/Mat types, ShaderUniform, ComposableShaderFunction,  |
|                  |       | FunctionChain, ParallelComposition, ConditionalFunction, UniformBuffer     |
| `temporal.rs`    | 1,493 | TemporalInterpolation, Easing, KeyframeAnimation, AnimationTimeline,       |
|                  |       | CubicBezierTiming, AnimationTimelineWithEvents                             |
| `math.rs`        | 1,297 | LinearScale, LogScale, PowerScale, ExponentialScale, BandScale,            |
|                  |       | PointScale, OrdinalScale, Clamp, Threshold, SmoothStep                     |
| `color.rs`       | 1,832 | ColorMap, ColorGradient, ColorScale, HSVColorMap, AlphaBlending,           |
|                  |       | ColorSpaceConverter, PerceptualColorSpaceConverter, PerceptualInterpolation |
| `geometric.rs`   | 441   | PositionTransform, PolarTransform, MatrixTransform,                        |
|                  |       | ProjectionTransform, DistanceFunction                                      |
| `statistical.rs` | 2,071 | NormalizeFunction, StandardizeFunction, QuantileFunction, BinningFunction, |
|                  |       | StatisticsCompute, HistogramCompute, StreamingStatistics, KernelDensity    |
| `mod.rs`         | 3,359 | Module declarations, re-exports, vec/mat macros, all test modules          |

### Key Design Decisions

- **Macros stay in `mod.rs`**: `macro_rules!` macros (vec2!, vec3!, vec4!,
  mat2!, mat3!, mat4!) must be defined in the parent module to be visible in
  submodules, per Rust's macro scoping rules.
- **Tests stay in `mod.rs`**: All test modules remain in mod.rs where
  `use super::*` gives them access to everything through the re-exports. This
  means zero test modifications were needed.
- **Glob re-exports**: `pub use self::core::*;` etc. in mod.rs ensures all
  existing `use crate::shader_function::Foo` paths continue to work.

### Test Results

- 2,813 lib tests pass (0 failures, 4 ignored)
- All examples compile without changes
- No breaking API changes
