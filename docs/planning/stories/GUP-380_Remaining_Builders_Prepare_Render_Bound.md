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

## Retrospective

**Completed**: 2025-07-22

### Key Technical Learnings

#### Two Render Preparation Patterns

- **Challenge**: `prepare_render_bound()` requires attribute bindings set via
  `.attr()`, which works for builders that use `apply_accessors_to_selection()`
  (bar, scatter, line). But boxplot and violin builders pre-compute
  `BoxPlotAttributes` directly — they don't use accessor bindings.
- **Solution**: Used `prepare_render()` with a mapper closure for boxplot and
  violin builders, and `apply_accessors_to_selection()` +
  `prepare_render_bound()` for the density builder.
- **Pattern**: There are two distinct paths to render-readiness: (1) accessor
  bindings → `prepare_render_bound()` for builders that use
  `apply_accessors_to_selection`, and (2) mapper closure → `prepare_render()`
  for builders that pre-compute typed attribute data. Choose based on whether
  the selection's data type is the user's generic `T` or a pre-computed mark
  attribute type.

#### NDC Coordinate Transformation for Pre-Computed Marks

- **Challenge**: BoxPlot and Violin builders store statistical values (whisker
  min/max, quartiles, positions, outliers) in data space. The BoxPlot shader
  passes these through an identity viewport transform by default, so values
  outside [-1, 1] render off-screen. Simply calling `prepare_render()` with
  `BoxPlotInstance::from()` produced 0 visible pixels.
- **Solution**: Compute the chart area NDC bounds and data domain, then
  transform all BoxPlotAttributes values (position, statistical values, width,
  outliers) from data space to NDC space in the mapper closure before creating
  BoxPlotInstance.
- **Pattern**: Any builder that pre-computes mark data must also transform
  that data into NDC coordinates. The transformation requires: (1) chart area
  NDC bounds from `calculate_chart_area()`, (2) data domain computed from the
  mark attributes, (3) per-field linear mapping from data range to NDC range.

#### Density Builder Ownership Restructuring

- **Challenge**: The density builder borrowed `x_accessor` and `y_accessor`
  via `as_ref()` for sample extraction, then needed to move them into
  `apply_accessors_to_selection()`. The original code used separate local
  references that blocked the move.
- **Solution**: Restructured to validate accessor presence upfront with early
  returns, then use `as_ref().unwrap()` for sample extraction (safe after
  validation), leaving the `Option` values intact for the later move into
  `apply_accessors_to_selection()`.
- **Pattern**: When a consumed `self` method needs to both borrow and later
  move a field, validate first, borrow temporarily, then move.

### Architectural Decisions

#### Mapper vs Binding Approach per Builder Type

- **Decision**: Used `prepare_render()` with a mapper for boxplot/violin, and
  `apply_accessors_to_selection()` + `prepare_render_bound()` for density.
- **Reasoning**: Boxplot and violin builders create `Selection<BoxPlotAttributes,
  BoxPlot>` where the data type IS the mark attributes. A mapper that converts
  `BoxPlotAttributes → BoxPlotInstance` with NDC transformation is the natural
  fit. The density builder uses `Selection<T, Rectangle>` where `T` is the
  user's generic type — accessor bindings are the established pattern for this.
- **Trade-off**: Two different patterns across builders adds complexity, but
  each pattern matches its data model naturally.
- **Future**: A shared `finalize_boxplot_chart()` helper could extract the
  common NDC transformation + `prepare_render` pattern used by both boxplot
  and violin builders.

### Development Workflow Insights

- The story was labelled "Very Low" risk and "mechanical application", but the
  boxplot/violin builders required significantly more work than expected due to
  the NDC transformation requirement. The density builder WAS mechanical (same
  pattern as bar), but boxplot/violin needed a different approach entirely.
- The visual regression test pattern (render to RGBA, count non-white pixels
  in center region) caught the NDC issue immediately — the initial
  implementation passed `is_render_ready()` but produced 0 visible pixels.
- Running `--test-threads=1` is essential; the GPU tests reliably pass with
  serial execution.

### Follow-up Stories

1. **GUP-381: Extract shared NDC transformation helper for BoxPlot builders** —
   The boxplot and violin builders share ~60 lines of identical NDC
   transformation logic (domain computation, coordinate mapping, outlier
   transformation). This should be extracted into a shared helper function to
   reduce duplication and simplify future BoxPlot-based builders.
