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

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Box vs Arc for Cloneable Closures

- **Challenge**: `Box<dyn Fn>` cannot be cloned because closures don't implement
  `Clone`. The original workaround was to re-create a field-based accessor on
  clone, silently losing the closure.
- **Solution**: Replace `Box<dyn Fn>` with `Arc<dyn Fn>`. Arc is reference-
  counted and cloneable, so `Arc::clone` gives a new handle to the same
  closure at negligible cost.
- **Pattern**: Whenever a type needs to be Clone but contains a trait object
  closure (`dyn Fn`), `Arc` is the standard Rust solution. This is a common
  pattern in async runtimes, callbacks, and event handlers.

#### Cascading Clone Fixes

- **Challenge**: `AreaChartBuilder::clone()` was already working around the
  broken `AccessorFunction::clone()` by resetting `y0_baseline` to
  `Baseline::default()` instead of cloning it (since `Baseline::Accessor`
  contains an `AccessorFunction`).
- **Solution**: With `AccessorFunction` now properly cloneable, added a `Clone`
  impl for `Baseline<T>` and fixed `AreaChartBuilder::clone()` to clone the
  baseline faithfully.
- **Pattern**: When fixing a fundamental type's Clone impl, audit all containing
  types for workarounds that can be cleaned up.

### Architectural Decisions

#### Arc Over Removing Clone

- **Decision**: Used `Arc<dyn Fn>` rather than removing `Clone` from
  `AccessorFunction`.
- **Reasoning**: Removing `Clone` would have broken `BarChartBuilder::clone()`,
  `AreaChartBuilder::clone()`, and multiple examples. The `Arc` approach is
  fully backward-compatible with zero API changes required.
- **Trade-off**: Slight overhead from atomic reference counting on clone/drop,
  but closures are cloned infrequently and the cost is negligible compared to
  GPU operations.
- **Future**: This enables any future code to freely clone accessors without
  concern. It also aligns with how `AccessorFunction` is used (shared
  read-only reference to a function).

### Development Workflow Insights

- The fix was minimal and surgical: changing 4 lines in the struct definition
  and constructor (Box → Arc), plus rewriting the Clone impl (3 lines). Total
  code change was small but high-impact.
- The `cargo clean` was needed due to disk space constraints before running
  `mask all-fix`, which compiles with all features and all targets. This added
  significant wall-clock time to what was otherwise a 15-minute story.
- Having good test infrastructure made validation straightforward — the 3079
  existing tests all passing confirms no regressions.
