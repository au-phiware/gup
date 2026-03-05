# GUP-289: Bar Chart Builder prepare_render_bound

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-20 **Completed**: 2025-07-20

## Context

The `BarChartBuilder::build_with_data()` correctly computes `NdcBounds` and
calls `apply_accessors_to_selection()` to map data values to NDC chart-area
coordinates. However, it does not call `prepare_render_bound()`, so the
Selection is never marked render-ready. As a result, `render_to_png()` on a bar
chart produces axes and grid but no visible data bars.

## User Story

> "As a Gup developer, I want `BarChartBuilder` charts to render visible bars
> via `render_to_png()` without manually calling `prepare_render_bound()`."

## Acceptance Criteria

- [x] `BarChartBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time so the Selection is render-ready.
- [x] `render_to_png()` on a bar chart shows visible rectangles in the data
      area.
- [x] At least one test validates visible bar pixels in the data region.

## Technical Tasks

- [x] Call `prepare_render_bound()` at the end of
      `BarChartBuilder::build_with_data()` (after
      `apply_accessors_to_selection`).
- [x] Add a visual regression test for bar chart PNG export.

## Dependencies

### Prerequisite Stories

- GUP-284 ✅ (Unify Chart Builder Data Layer)

## Testing Strategy

- Visual regression test: `render_to_png` for a bar chart produces non-white
  pixels in the data region.

## Risk Assessment

- **Very Low**: The only change needed is adding a single
  `prepare_render_bound()` call. The NdcBounds mapping is already in place.

## Definition of Done

- [x] Bar chart builder produces visible data marks via `render_to_png()`.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### Key Changes

| File                              | Change                                                                                                                                                                                |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/chart_builder/builders/bar.rs` | Added `prepare_render_bound()` call after `apply_accessors_to_selection()`. Cloned `context` before passing to `Selection::new` so it remains available for the pipeline preparation. Added 2 new tests. |

### Test Summary

- 22 bar chart tests pass (20 pre-existing + 2 new)
- `test_bar_chart_is_render_ready_after_build`: verifies `is_render_ready()` and
  `has_data_mark_data()` after build
- `test_bar_chart_render_to_png_produces_visible_bars`: renders to RGBA and
  checks for non-white pixels in the data region
