# GUP-381: Extract shared NDC transformation helper for BoxPlot builders

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-22 **Completed**: 2025-07-22

## Context

GUP-380 added NDC coordinate transformation to both `BoxPlotBuilder` and
`ViolinPlotBuilder`. The two builders share ~60 lines of identical logic:
computing data domain from `BoxPlotAttributes`, mapping positions, statistical
values, width, and outliers from data space to NDC coordinates. This duplication
should be extracted into a shared helper.

## User Story

> "As a Gup developer, I want a shared helper for transforming BoxPlotAttributes
> into NDC-space BoxPlotInstance values so that new BoxPlot-based builders don't
> need to duplicate the coordinate mapping logic."

## Acceptance Criteria

- [x] A shared helper function exists that transforms `BoxPlotAttributes` into
      NDC-space `BoxPlotInstance` values given NDC bounds.
- [x] `BoxPlotBuilder::build_with_data()` uses the shared helper.
- [x] `ViolinPlotBuilder::build_with_data()` uses the shared helper.
- [x] All existing tests continue to pass.

## Technical Tasks

- [x] Extract the NDC domain computation (x/y min/max from BoxPlotAttributes
      slice) into a shared function.
- [x] Extract the BoxPlotAttributes → BoxPlotInstance NDC mapper into a shared
      function or closure factory.
- [x] Update both builders to use the shared functions.

## Dependencies

### Prerequisite Stories

- GUP-380 ✅ (Remaining Builders prepare_render_bound)

## Testing Strategy

- All existing boxplot and violin tests should continue to pass unchanged.
- No new tests needed — this is a pure refactor.

## Success Metrics

- Duplication between boxplot and violin builders reduced to a single call site.
- All existing tests pass.

## Risk Assessment

- **Very Low**: Pure refactoring with no behaviour change.

## Definition of Done

- [x] Shared helper function extracts common logic.
- [x] Both builders use the helper.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### What Was Implemented

A single `boxplot_ndc_mapper()` function was added to
`src/chart_builder/builders.rs` that encapsulates the full NDC transformation
pipeline for BoxPlot-based marks:

1. Computes data domain (x/y min/max) from a `&[BoxPlotAttributes]` slice,
   including outlier values and 5% padding.
2. Returns a closure (`impl Fn(&BoxPlotAttributes) -> BoxPlotInstance`) that
   maps positions, statistical values (whisker_min…whisker_max), width, and up
   to 32 outliers from data space to NDC coordinates.

Both `BoxPlotBuilder::build_with_data()` and
`ViolinPlotBuilder::build_with_data()` now construct an `NdcBounds` and delegate
to `boxplot_ndc_mapper()`, replacing ~60 duplicated lines each.

### Key Files Changed

| File                                    | Change                                                         |
| --------------------------------------- | -------------------------------------------------------------- |
| `src/chart_builder/builders.rs`         | Added `boxplot_ndc_mapper()` function and BoxPlot type imports |
| `src/chart_builder/builders/boxplot.rs` | Replaced inline NDC mapping with call to shared helper         |
| `src/chart_builder/builders/violin.rs`  | Replaced inline NDC mapping with call to shared helper         |

### Test Results

- 3,067 lib tests passed, 0 failed
- All integration tests passed
- All examples compile

## Retrospective

**Completed**: 2025-07-22

### Key Technical Learnings

#### Rust 2024 Precise Capturing (`use<>`)

- **Challenge**: The `boxplot_ndc_mapper()` function takes
  `&[BoxPlotAttributes]` and returns `impl Fn(…) -> BoxPlotInstance`. Under Rust
  2024 edition rules, `impl Trait` return types capture all input lifetimes, so
  the compiler considered the returned closure to borrow the input slice. This
  caused a borrow-checker error when the caller needed to pass the closure to
  `prepare_render()` which takes `&mut self` on the same struct.
- **Solution**: Added `+ use<>` to the return type:
  `impl Fn(&BoxPlotAttributes) -> BoxPlotInstance + use<>`. This tells the
  compiler the closure captures no external lifetimes — only owned values (the
  computed domain floats and `NdcBounds` copy).
- **Pattern**: When writing factory functions that compute state from a borrowed
  slice and return closures over that computed state, use `+ use<>` (or the
  appropriate capture set) in Rust 2024 to avoid spurious lifetime captures.

### Architectural Decisions

#### Helper Placement in `builders.rs`

- **Decision**: Placed `boxplot_ndc_mapper()` in `src/chart_builder/builders.rs`
  rather than in the `boxplot` submodule.
- **Reasoning**: The function sits next to `NdcBounds` (which it consumes) and
  `apply_accessors_to_selection()` (which follows the same shared-helper
  pattern). Both the boxplot and violin submodules reference it via `super::`.
- **Trade-off**: Slightly larger `builders.rs`; an alternative would have been a
  dedicated `ndc_helpers.rs` module, but for a single function this was
  unnecessary.
- **Future**: If more chart-type-specific NDC helpers emerge (e.g., for density
  or area charts), a dedicated submodule could be warranted.

### Development Workflow Insights

- The refactoring was straightforward — the duplicated code was truly identical
  between the two builders, with no subtle divergence.
- Disk space exhaustion on the build volume required `cargo clean` and use of
  `CARGO_TARGET_DIR=/tmp/gup-target` to run tests on a separate filesystem. This
  is a recurring environment constraint worth documenting.
- `mask all-fix` runs clippy with `-D warnings`, which flags many pre-existing
  dead-code warnings in `gup-macros` — these are unrelated to this story.

### Follow-up Stories

No follow-up stories identified. The refactoring is complete and self-contained.
