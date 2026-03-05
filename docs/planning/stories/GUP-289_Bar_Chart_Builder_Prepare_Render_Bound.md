# GUP-289: Bar Chart Builder prepare_render_bound

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 🚧 In Progress **Created**:
2025-07-20

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

- [ ] `BarChartBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time so the Selection is render-ready.
- [ ] `render_to_png()` on a bar chart shows visible rectangles in the data
      area.
- [ ] At least one test validates visible bar pixels in the data region.

## Technical Tasks

- [ ] Call `prepare_render_bound()` at the end of
      `BarChartBuilder::build_with_data()` (after `apply_accessors_to_selection`).
- [ ] Add a visual regression test for bar chart PNG export.

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

- [ ] Bar chart builder produces visible data marks via `render_to_png()`.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
