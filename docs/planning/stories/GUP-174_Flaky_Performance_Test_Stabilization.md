# GUP-174: Flaky Performance Test Stabilization

**Priority**: Low **Complexity**: Low **Created**: 2025-08-06 **Status**: ✅
Complete **Completed**: 2025-08-07

## Overview

Review and stabilize timing-sensitive performance tests across the codebase that
fail intermittently in debug builds due to tight thresholds or system load
variability.

## Context

During GUP-077 implementation, the pre-existing
`label::positioner::tests::test_performance_500_labels` test was observed
failing with 12ms vs 10ms threshold — a marginal timing issue in debug builds.
Similar flaky patterns may exist in other performance tests.

## User Story

As a developer, I want all tests to pass reliably so that test failures indicate
real bugs rather than environmental noise.

## Acceptance Criteria

- [x] Audit all performance-threshold tests in the codebase
- [x] Adjust thresholds to account for debug-build overhead
- [x] Add documentation noting which tests have timing sensitivity
- [x] Ensure `cargo test -- --test-threads=1` passes reliably in debug mode

## Technical Tasks

- [x] Grep for timing assertions across all test files
- [x] Categorize by current reliability (always pass, sometimes fail, often
      fail)
- [x] Adjust thresholds or add `#[ignore]` annotations with comments
- [x] Consider environment-detection for adaptive thresholds

## Dependencies

- **Related**: GUP-077 (established performance testing patterns)

## Testing Strategy

- Run full test suite 10 times in debug mode to verify reliability
- Confirm no regressions in test coverage

## Risk Assessment

- **Low**: Simple threshold adjustments

## Definition of Done

- [x] All performance tests pass reliably in debug builds
- [x] Thresholds documented with rationale

## Implementation Summary

### What Was Implemented

Applied `#[cfg(debug_assertions)]` / `#[cfg(not(debug_assertions))]`
profile-aware threshold differentiation to all 30+ timing-sensitive test
assertions across 15 files, following the pattern established by GUP-187.

### Files Changed

**Source files (11):**

- `src/text/layout.rs` — 3 tests: glyph positioning, collision detection, grid
  efficiency
- `src/text/atlas.rs` — 2 tests: SDF generation, glyph cache efficiency
- `src/text/renderer.rs` — 1 test: vertex creation performance
- `src/render.rs` — 2 tests: blend mode switching, RAII guard performance
- `src/tick_generator.rs` — 1 test: tick generation (μs-level)
- `src/mark.rs` — 1 test: shader generation performance
- `src/buffer.rs` — 1 test: GPU buffer download performance
- `src/selection.rs` — 1 test: uniform update performance
- `src/context.rs` — 1 test: surface resize performance
- `src/pipeline_cache.rs` — 1 test: cached vs uncached comparison (tolerance
  factor)
- `src/shader_ast/benchmarks.rs` — 4 benchmarks + 5 tests: AST pipeline
  thresholds

**Test files (4):**

- `tests/interaction_performance_tests.rs` — 6 tests: point/region/batch/stream
  queries
- `tests/axis_integration_tests.rs` — 1 test: tick position calculation
- `tests/shader_function_integration.rs` — 1 test: shader function composition
- `tests/color_gradient_performance_tests.rs` — 4 tests: builder, large
  gradient, presets, WGSL gen

### Test Results

- 1,227 library/integration tests pass (0 failures, 4 ignored)
- All 15 modified test files compile and pass
- Tests verified stable across 3 consecutive runs
