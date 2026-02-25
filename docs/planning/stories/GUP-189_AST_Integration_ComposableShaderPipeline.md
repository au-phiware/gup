# GUP-189: AST Integration with ComposableShaderPipeline

**Story ID**: GUP-189 **Title**: AST Integration with ComposableShaderPipeline
**Status**: ✅ Complete **Completed**: 2025-08-08 **Priority**: Medium **Effort**: 5 story points
**Created**: 2025-08-07 **Dependencies**: GUP-073 (Advanced Shader Composition)

## Overview

Wire the AST-based optimizer from `shader_ast` into the existing
`ComposableShaderPipeline.optimize_shader()` method so that current pipeline
users get AST-based dead code elimination, constant folding, and function
inlining transparently, without changing their API usage.

## Context

GUP-073 introduced a full AST-based WGSL system (`shader_ast` module) but it
lives alongside the existing string-based `ComposableShaderPipeline`. The two
systems are not yet integrated — the old pipeline still uses string-based
optimizations (regex-like replacements). This story bridges the gap.

## User Story

As a developer using the shader pipeline, I want the existing optimization
methods to use AST-based passes automatically, so that I get better
optimizations without changing my code.

## Acceptance Criteria

- [x] `ComposableShaderPipeline::optimize_shader()` delegates to AST-based
      optimizer when `OptimizationConfig.use_ast_analysis` is true
- [x] Backward-compatible: existing tests still pass
- [x] AST parsing errors fall back to string-based optimization gracefully
- [x] Generated WGSL from the pipeline is at least as small as before
- [x] Performance: no regression in pipeline compilation benchmarks

## Technical Tasks

1. Add AST parsing step inside `optimize_shader()`.
2. Implement fallback logic if AST parsing fails.
3. Update `OptimizationConfig` to expose AST control.
4. Add integration tests comparing old and new optimizer output.

## Dependencies

- GUP-073: Advanced Shader Composition (provides `shader_ast` module)

## Testing Strategy

- Unit tests for the integration path.
- Regression tests comparing output before/after.
- Performance benchmarks to detect regressions.

## Success Metrics

- All existing `shader_pipeline` tests continue to pass.
- AST optimizer produces smaller or equal WGSL output.
- No measurable performance regression.

## Risk Assessment

- **Low risk**: The AST system is new and additive. String-based fallback
  ensures no breakage.

## Definition of Done

- [x] Implementation complete with tests passing
- [x] `mask all-fix` clean
- [x] All examples compile
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

- Added `use_ast_analysis` field to `OptimizationConfig` (default: `false` for
  backward compatibility)
- Refactored `optimize_shader()` to delegate to AST-based optimizer
  (`parse_wgsl` → `optimize` → `generate_wgsl_minimal`) when
  `use_ast_analysis` is true
- Implemented graceful fallback: if AST parsing fails, the string-based
  optimization path is used automatically
- Improved the `optimize()` function in `shader_ast::optimizer` to re-run
  dead-code elimination after function inlining, removing functions that became
  dead after their call sites were inlined

### Key Files Changed

| File                                        | Change                                           |
| ------------------------------------------- | ------------------------------------------------ |
| `src/shader_pipeline.rs`                    | Added `use_ast_analysis`, AST delegation, 7 new tests |
| `src/shader_ast/optimizer.rs`               | Re-run DCE after inlining                        |
| `tests/shader_pipeline_performance_tests.rs` | 4 new integration tests, updated struct literals |

### Test Counts

- **7 new unit tests** in `shader_pipeline::tests` (AST integration)
- **4 new integration tests** in `shader_pipeline_performance_tests`
- **All 23 shader_pipeline tests pass**
- **All 58 shader_ast tests pass**
- **All 14 performance tests pass**
