# GUP-379: Area Chart Builder prepare_render_bound

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-20 **Completed**: 2025-07-21

## Context

The `AreaChartBuilder::build_with_data()` computes `NdcBounds` and calls
`apply_accessors_to_selection()` to map data values to NDC chart-area
coordinates. However, like the bar chart builder before GUP-289, it does not
call `prepare_render_bound()`, so the Selection is never marked render-ready. As
a result, `render_to_png()` on an area chart may produce axes and grid but no
visible filled-area segments.

This is the same class of bug fixed in GUP-289 for the bar chart builder.

## User Story

> "As a Gup developer, I want `AreaChartBuilder` charts to render visible filled
> areas via `render_to_png()` without manually calling
> `prepare_render_bound()`."

## Acceptance Criteria

- [x] `AreaChartBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time so the Selection is render-ready.
- [x] `render_to_png()` on an area chart shows visible filled segments in the
      data area.
- [x] At least one test validates visible area pixels in the data region.

## Technical Tasks

- [x] Clone `context` before passing to `Selection::new` in
      `AreaChartBuilder::build_with_data()`.
- [x] Call `prepare_render_bound()` at the end of
      `AreaChartBuilder::build_with_data()` (after
      `apply_accessors_to_selection`).
- [x] Add a visual regression test for area chart PNG export.

## Dependencies

### Prerequisite Stories

- GUP-287 ✅ (Area Chart Data Mark Rendering)
- GUP-289 ✅ (Bar Chart Builder prepare_render_bound — establishes the pattern)

## Testing Strategy

- Visual regression test: `render_to_png` for an area chart produces non-white
  pixels in the data region.

## Success Metrics

- Area chart `render_to_png()` produces visible filled-area marks.
- All existing area chart tests continue to pass.

## Risk Assessment

- **Very Low**: The fix is a single `prepare_render_bound()` call following the
  established pattern from GUP-289.

## Definition of Done

- [x] Area chart builder produces visible data marks via `render_to_png()`.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### Changes Made

- **`src/chart_builder/builders/area.rs`**: Added `prepare_render_bound()` call
  at the end of both `build_with_data()` (line-segment based area chart) and
  `build_filled()` (tessellated polygon area chart). Cloned `context` before
  passing to `Selection::new` so `device()`/`queue()` remain accessible for the
  `prepare_render_bound()` call.
- Added `test_area_chart_render_to_png_produces_visible_area` visual regression
  test that validates non-white pixels appear in the data region when rendering
  an area chart via `render_to_rgba()`.

### Test Results

- 3061 library tests pass, 0 failures
- 38 area chart tests pass including the new visual regression test
- All examples compile
