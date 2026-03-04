# GUP-294: Shader Function Module Reorganization

## Story Overview

**Title**: Shader Function Module Reorganization **Epic**: Phase 1 Initiative
2 - Unified Shader Function System **Priority**: Low **Story Points**: 3
**Status**: 🚧 In Progress

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

- [ ] Split `shader_function.rs` into category-based submodules
- [ ] Submodules: `math.rs`, `color.rs`, `geometric.rs`, `statistical.rs`,
      `temporal.rs`, `core.rs`
- [ ] All public API types remain accessible from `shader_function::` path
- [ ] All existing tests continue to pass without modification
- [ ] No breaking changes to downstream code or examples
- [ ] Re-exports in `shader_function/mod.rs` maintain backward compatibility

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

- [ ] All code migrated to submodules
- [ ] All tests pass
- [ ] All examples compile
- [ ] No breaking API changes
