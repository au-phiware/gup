# GUP-380: Remaining Builders prepare_render_bound

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-21 **Completed**: 2025-07-22

## Context

GUP-289 and GUP-379 fixed `BarChartBuilder` and `AreaChartBuilder` respectively
to call `prepare_render_bound()` at build time. Three other chart builders that
implement `ChartBuilder::build_with_data()` still lack this call:

- `BoxPlotBuilder`
- `DensityPlotBuilder`
- `ViolinPlotBuilder`

Without the call, `render_to_png()` on these chart types may produce axes and
grid but no visible data marks.

## User Story

> "As a Gup developer, I want all chart builders that return a `ComposedChart`
> to call `prepare_render_bound()` at build time so that `render_to_png()` works
> without manual GPU pipeline setup."

## Acceptance Criteria

- [x] `BoxPlotBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time.
- [x] `DensityPlotBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time.
- [x] `ViolinPlotBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time.
- [x] Each builder has a visual regression test validating non-white pixels in
      the data region via `render_to_rgba()`.

## Technical Tasks

- [x] Clone `context` before passing to `Selection::new` in each builder.
- [x] Call `prepare_render_bound()` after attribute bindings in each builder.
- [x] Add visual regression tests for boxplot, density, and violin chart types.

## Dependencies

### Prerequisite Stories

- GUP-289 ✅ (Bar Chart Builder prepare_render_bound — establishes the pattern)
- GUP-379 ✅ (Area Chart Builder prepare_render_bound)

## Testing Strategy

- Visual regression tests: `render_to_rgba` for each chart type produces
  non-white pixels in the data region.

## Success Metrics

- All three chart builders produce visible data marks via `render_to_png()`.
- All existing tests continue to pass.

## Risk Assessment

- **Very Low**: Mechanical application of the established pattern from GUP-289
  and GUP-379.

## Definition of Done

- [x] All three builders produce visible data marks via `render_to_png()`.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### Changes Made

| File | Change |
| --- | --- |
| `src/chart_builder/builders/boxplot.rs` | Added NDC transformation mapper and `prepare_render()` call. Computes chart area NDC bounds and data domain, maps all BoxPlotAttributes (positions, statistical values, width, outliers) from data space to clip space. Cloned `context` before `Selection::new`. Added 2 new tests. |
| `src/chart_builder/builders/density.rs` | Added NDC bounds computation, `apply_accessors_to_selection()`, and `prepare_render_bound()` call. Imported `NdcBounds` and `apply_accessors_to_selection`. Restructured accessor validation for ownership compatibility. Added 2 new tests. |
| `src/chart_builder/builders/violin.rs` | Added NDC transformation mapper and `prepare_render()` call (same pattern as boxplot). Imported `BoxPlotInstance`. Added 2 new tests. |

### Test Results

- 3067 library tests pass, 0 failures
- 9 boxplot tests pass (7 pre-existing + 2 new)
- 48 density tests pass (46 pre-existing + 2 new)
- 22 violin tests pass (20 pre-existing + 2 new)
- All examples compile
