# GUP-380: Remaining Builders prepare_render_bound

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 🚧 In Progress **Created**:
2025-07-21

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

- [ ] `BoxPlotBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time.
- [ ] `DensityPlotBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time.
- [ ] `ViolinPlotBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time.
- [ ] Each builder has a visual regression test validating non-white pixels in
      the data region via `render_to_rgba()`.

## Technical Tasks

- [ ] Clone `context` before passing to `Selection::new` in each builder.
- [ ] Call `prepare_render_bound()` after attribute bindings in each builder.
- [ ] Add visual regression tests for boxplot, density, and violin chart types.

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

- [ ] All three builders produce visible data marks via `render_to_png()`.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
