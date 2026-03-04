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

## Retrospective

**Completed**: 2025-03-05

### Key Technical Learnings

#### Rust macro_rules! Scoping in Module Hierarchies

- **Challenge**: After moving code from `mod.rs` into submodules, the `vec2!`,
  `vec3!`, `vec4!`, `mat2!`, `mat3!`, `mat4!` macros (defined via
  `macro_rules!`) were not visible in sibling submodules. Compilation failed
  with "cannot find macro `vec4` in this scope" across all submodules.
- **Solution**: Keep `macro_rules!` macros in `mod.rs` (the parent module),
  defined before the `pub mod` declarations. In Rust, `macro_rules!` macros are
  scoped to the module they're defined in and its child modules. They are NOT
  automatically available through `pub use` re-exports.
- **Pattern**: When splitting a module that contains `macro_rules!` definitions,
  always keep the macros in the parent module file. Alternatively, use
  `#[macro_export]` for crate-wide visibility, but that changes the macro's
  canonical path.

#### Non-Contiguous Code Extraction

- **Challenge**: The original file had categories interleaved. For example, the
  "basic functions" section around line 3534 contained LinearScale (math),
  ColorMap (color), and PositionTransform (geometric) all adjacent to each
  other.
- **Solution**: Used a scripted extraction approach (Node.js) to precisely
  select non-contiguous line ranges for each submodule, rather than trying to
  do sequential cut-and-paste.
- **Pattern**: For large file splits where sections interleave, write a script
  that takes explicit line ranges rather than attempting manual extraction.

#### Test Module Placement Strategy

- **Challenge**: Three test modules (`tests`, `compatibility_tests`,
  `color_scale_tests`) all used `use super::*` and tested across categories.
  Moving them to individual submodules would require modifying imports.
- **Solution**: Kept all tests in `mod.rs`. Since `mod.rs` re-exports
  everything via `pub use`, `use super::*` in tests continues to bring all
  symbols into scope. Zero test modifications needed.
- **Pattern**: For pure refactoring splits, keeping tests in the parent module
  with glob re-exports is the lowest-risk approach that satisfies "no test
  modification" requirements.

### Architectural Decisions

#### Glob Re-exports for Backward Compatibility

- **Decision**: Use `pub use self::core::*;` etc. in mod.rs for all submodules.
- **Reasoning**: The story explicitly requires "all public API types remain
  accessible from `shader_function::` path" with no breaking changes. Glob
  re-exports achieve this with minimal maintenance overhead.
- **Trade-off**: Glob re-exports can cause name conflicts if two submodules
  export the same name. Currently no conflicts exist.
- **Future**: If submodule-specific imports are desired (e.g.,
  `shader_function::math::LinearScale`), the current structure already supports
  this since submodules are `pub mod`.

#### Keeping `core` as Module Name

- **Decision**: Named the core traits module `core.rs` as specified in the
  story, despite `core` being a Rust standard library crate name.
- **Reasoning**: The module is accessed as `self::core` or
  `super::core` within the shader_function module tree, which doesn't conflict
  with `::core` (the standard library). No code in this module references
  `::core` directly.
- **Trade-off**: Could cause confusion for developers who expect `core` to
  refer to the standard library. However, the shader_function module is
  internal and well-documented.

### Development Workflow Insights

- **Disk space**: The ZFS home partition ran out of space during development
  (0 bytes available due to snapshots). Using `CARGO_TARGET_DIR=/tmp/gup-target`
  (an existing target dir on a separate filesystem) was essential to continue
  building and testing.
- **Scripted extraction**: Using Node.js to read the file and extract precise
  line ranges into new files was much more reliable than manual editing of a
  12,000+ line file. The script approach allowed precise control over which
  lines went where.
- **Incremental verification**: Running `cargo check` after each submodule
  creation caught import issues immediately, making them easy to fix one at a
  time rather than debugging a large batch of errors.

### Follow-up Stories

1. **Move Tests to Submodules** — Move the three test modules from `mod.rs`
   into their respective submodules (e.g., color tests to `color.rs`,
   statistical tests to `statistical.rs`). This would further reduce `mod.rs`
   from 3,359 lines and co-locate tests with the code they verify. Lower
   priority since current approach works.
