# GUP-284: Unify Chart Builder Data-Layer Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-18 **Completed**: 2025-07-19

## Context

The `ComposedChart::render()` method currently renders only axes, grid lines,
and ticks. The actual data marks (circles for scatter, lines for line charts,
rectangles for bar charts) are not wired into the chart builder's render pass.
The method contains a placeholder comment: "In a complete implementation, this
would use the Mixable render system."

This gap was surfaced by GUP-264 (Tauri Integration), where the WASM API had to
bypass the chart builder entirely and render circles using the low-level Mark
system directly. Without this story, all chart builder consumers (examples,
export pipelines, integration crates) must manually render data marks.

## User Story

> "As a Gup developer, I want `ComposedChart::render()` and `render_to_png()` to
> produce complete charts (data marks + axes + grid) so that I don't have to
> manually wire up mark rendering for every chart type."

## Acceptance Criteria

- [x] `ComposedChart::render_to_png()` produces a PNG with visible data marks
      (circles for scatter, lines for line charts) in addition to axes and grid.
- [x] `ComposedChart::render()` records draw commands for data marks into the
      render pass.
- [x] The `ScatterPlotBuilder` produces a chart where data points are visible
      without additional manual rendering.
- [x] Existing examples that use the chart builder (`01_hello_chart`,
      `styled_scatter`, etc.) show data marks.
- [x] Performance: rendering 1000 data points via the builder adds < 2 ms to the
      frame time.

## Technical Tasks

- [x] Wire the `Selection<T, M>`'s GPU buffers (vertex, instance, index) into
      `ComposedChart`'s render pass.
- [x] Add a `draw_data_marks()` method to `ComposedChart` analogous to
      `draw_grid_lines()` and `draw_ticks()`.
- [x] Call `draw_data_marks()` in both `render()` and `render_to_png()`.
- [x] Update examples to verify visible data marks.

## Dependencies

### Prerequisite Stories

- GUP-018 ✅ (Chart Builders)
- GUP-011 ✅ (Mark-Shader Integration)

## Testing Strategy

- Visual regression test: `render_to_png` for a scatter chart produces non-white
  pixels in the data region.
- Unit test: `draw_data_marks()` returns a non-zero draw count.

## Risk Assessment

- **Medium**: The Selection/Mark rendering pipeline uses a different shader and
  buffer layout than the axis/tick pipeline. Integrating them into a single
  render pass requires careful pipeline management.

## Definition of Done

- [x] `render_to_png` produces visible data marks.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.
- [x] At least one example visually confirmed with data marks.

## Implementation Summary

### What Was Implemented

The chart builder's data-mark rendering pipeline was connected end-to-end so
that `ComposedChart::render()`, `render_to_png()`, and
`render_to_texture_view()` produce complete charts with visible data marks
(circles, rectangles, etc.) alongside axes, ticks, and grid lines.

### Key Changes

| File | Change |
| --- | --- |
| `src/chart_builder/builders.rs` | Rewrote `apply_accessors_to_selection()` to evaluate accessor functions, auto-compute data domain, and map data values to NDC chart-area coordinates. Added `NdcBounds` struct and `auto_domain()` helper. |
| `src/chart_builder/builders/scatter.rs` | Updated `build_with_data()` to compute exact chart area (with axis margins) before applying accessor bindings, then call `prepare_render_bound()` at build time. |
| `src/chart_builder/builders/bar.rs` | Updated for new `apply_accessors_to_selection` signature. |
| `src/chart_builder/builders/heatmap/mod.rs` | Updated for new `apply_accessors_to_selection` signature. |
| `src/chart_builder.rs` | Added `prepare_data_pipeline()`, `draw_data_marks()`, `has_data_mark_data()` methods. Updated `render()` to prepare data marks. Made `ChartArea` and `calculate_chart_area()` `pub(crate)`. |
| `src/chart_builder/labels.rs` | Added `M: MarkInstanceBuilder` bound to `LabeledChart::render()`. |

### Test Summary

- 3 new tests added (render-ready selection, draw count, visual PNG regression)
- All 3056 library tests pass
- All examples compile and run correctly
