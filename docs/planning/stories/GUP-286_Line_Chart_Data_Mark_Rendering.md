# GUP-286: Line Chart Builder Data-Mark Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-19 **Completed**: 2025-07-20

## Context

GUP-284 unified the scatter chart builder's data-mark rendering so that
`ComposedChart::render_to_png()` produces complete charts with visible circles.
However, the `LineChartBuilder` needs analogous treatment. The `Line` mark uses
a different vertex layout (line strips / connected segments) and requires a
different mapping strategy than the instanced-quad approach used for circles.

## User Story

> "As a Gup developer, I want `LineChartBuilder` charts to render visible line
> segments via `render_to_png()` without manually wiring up the Line mark
> pipeline."

## Acceptance Criteria

- [x] `LineChartBuilder::build_with_data()` produces a render-ready
      `ComposedChart` whose Selection draws line segments.
- [x] `render_to_png()` on a line chart shows connected line segments in the
      data region.
- [x] Line segments respect the chart area NDC bounds (same axis alignment as
      scatter charts).
- [x] At least one test validates visible line pixels in the data region.

## Technical Tasks

- [x] Update `LineChartBuilder::build_with_data()` to apply accessor-driven attr
      bindings using the `NdcBounds` approach from GUP-284.
- [x] Call `prepare_render_bound()` at build time for Line marks.
- [x] Add a visual regression test for line chart PNG export.

## Dependencies

### Prerequisite Stories

- GUP-284 ✅ (Unify Chart Builder Data Layer)

## Testing Strategy

- Visual regression test: `render_to_png` for a line chart produces non-white
  pixels in the data region forming connected segments.

## Risk Assessment

- **Low**: The `Line` mark already implements `MarkInstanceBuilder`. The main
  work is adapting the accessor-to-NDC mapping for line segment endpoints rather
  than circle centres.

## Definition of Done

- [x] Line chart builder produces visible data marks via `render_to_png()`.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### What Was Implemented

The `LineChartBuilder::build_with_data()` method was updated to produce
render-ready line charts by mapping data-space segment coordinates to NDC
chart-area coordinates and preparing the GPU pipeline at build time.

### Key Changes

| File                              | Change                                                                                                                                                                               |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/chart_builder/builders/line.rs` | Rewrote `build_with_data()`: compute data domain with 5% padding, create ComposedChart with axes first, compute NdcBounds, bind start/end/color/width attrs with NDC mapping, call `prepare_render_bound()`. Added `auto_domain_from_iter()` helper. Added 2 new tests. |

### Test Summary

- 2 new tests added (`test_line_chart_has_data_mark_data`,
  `test_line_chart_render_to_png_produces_visible_lines`)
- All 24 line chart tests pass
- All 3058 library tests pass
- All examples compile and produce correct output
- Visual verification: `03_line_chart` and `multi_series_line` examples export
  PNGs with visible line segments
