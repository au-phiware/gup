# GUP-363: Fix AccessorFunction::clone() to Preserve Closures

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-27
**Completed**: 2025-07-27

## Context

`AccessorFunction::clone()` silently loses the closure, returning a field-based
accessor that always produces `AccessorValue::Float(0.0)`. This was discovered
during GUP-362 when the composite builder's override-by-append pattern was found
to be using cloned accessors that returned zero for every data point. While
GUP-362 removed the affected overrides, the broken `Clone` impl remains a
landmine for any code that clones `AccessorFunction`.

## User Story

> "As a library developer, I want `AccessorFunction::clone()` to either
> faithfully reproduce the closure or fail at compile time, so that I never
> silently get wrong data from a cloned accessor."

## Acceptance Criteria

- [x] `AccessorFunction` either preserves the closure through `Clone` (via
      `Arc`-based sharing) or is made non-Clone.
- [x] All existing code that clones `AccessorFunction` is updated to use the new
      pattern.
- [x] No silent data corruption from accessor cloning.
- [x] All tests pass.

## Technical Tasks

- [x] Audit all uses of `AccessorFunction::clone()` in the codebase.
- [x] Choose approach: `Arc`-based closure sharing or remove `Clone`.
- [x] Implement the chosen approach.
- [x] Update downstream code that depends on `Clone`.

## Dependencies

### Prerequisite Stories

- GUP-362: Accessor-to-GPU Position Pipeline ✅

## Testing Strategy

- Unit tests verifying that cloned accessors produce the same output as
  originals.
- Integration tests for any code paths that previously cloned accessors.

## Risk Assessment

- **Medium**: Removing `Clone` may require API changes for downstream code. The
  `Arc` approach is more backward-compatible but adds indirection.

## Definition of Done

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### Approach

Chose the `Arc`-based closure sharing approach over removing `Clone`. This is
fully backward-compatible — all existing code continues to work with zero
changes.

### Key Changes

- **`src/chart_builder/builders.rs`**: Changed `AccessorFunction<T>` internal
  storage from `Box<dyn Fn(&T) -> AccessorValue>` to
  `Arc<dyn Fn(&T) -> AccessorValue>`. Updated `new()`, `from_field()`, and
  `Clone` impl to use `Arc::new` and `Arc::clone`.
- **`src/chart_builder/builders/area.rs`**: Added `Clone` impl for
  `Baseline<T>` enum. Fixed `AreaChartBuilder::clone()` to properly clone the
  baseline instead of resetting to `Baseline::default()`.

### Clone Sites Audited

| Location | Status |
|---|---|
| `BarChartBuilder::clone()` (5 accessor fields) | ✅ Works via Arc |
| `AreaChartBuilder::clone()` (4 accessor fields + baseline) | ✅ Fixed |
| `examples/basic/03_line_chart.rs` (x/y accessor clones) | ✅ Works via Arc |
| `examples/intermediate/categorical_bar.rs` (x/y accessor clones) | ✅ Works via Arc |

### Tests Added

5 new unit tests:
- `test_accessor_function_clone_preserves_field_name`
- `test_accessor_function_clone_preserves_closure`
- `test_accessor_function_clone_preserves_string_closure`
- `test_accessor_function_clone_none_field_name`
- `test_option_accessor_function_clone`

### Test Results

- 3079 lib tests passed, 0 failed
- All examples compile
