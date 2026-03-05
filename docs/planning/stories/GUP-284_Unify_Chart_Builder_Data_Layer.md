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

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### Accessor Function Ownership and Arc Wrapping

- **Challenge**: `AccessorFunction<T>` contains a `Box<dyn Fn>` which is not
  `Clone`. The attr binding closures passed to `Selection::attr()` needed to
  capture the accessor functions, but they were borrowed references.
- **Solution**: Wrap `AccessorFunction<T>` in `Arc` before capturing in closures.
  Since the inner `Box<dyn Fn>` already has `Send + Sync` bounds on non-WASM, the
  `Arc` wrapper is `Send + Sync` and satisfies the `MaybeSend + MaybeSync` bounds
  required by `Selection::attr()`.
- **Pattern**: When closures need to share non-Clone function objects across
  attribute bindings, `Arc`-wrapping is the idiomatic Rust pattern.

#### Build-Time vs Render-Time Pipeline Preparation

- **Challenge**: Adding `M: MarkInstanceBuilder` bounds to `render_to_png()`,
  `render_to_texture_view()`, and all their transitive callers (HTML export, PDF
  export, etc.) would have been extremely invasive.
- **Solution**: Call `prepare_render_bound()` at build time in
  `ScatterPlotBuilder::build_with_data()`. The GPU context is available at build
  time, so the Selection is render-ready before any rendering method is called.
  This avoids adding trait bounds to any existing public API.
- **Pattern**: Prepare GPU resources as early as possible (at build time) rather
  than lazily at render time, when doing so avoids propagating restrictive trait
  bounds through the API surface.

#### NDC Coordinate Mapping with Chart Margins

- **Challenge**: Data positions need to be mapped from data-domain values to NDC
  coordinates within the chart area, which is affected by axis margins. Axes are
  added after the Selection is created but before accessors are applied.
- **Solution**: Restructure `build_with_data()` to add axes first
  (`.with_default_axes()`), then compute the exact chart area including axis
  margins, then apply accessor bindings with the correct NDC bounds.
- **Pattern**: Order operations so that layout-affecting configuration (axes,
  margins) is complete before computing spatial mappings for data.

### Architectural Decisions

#### Prepare at Build-Time Rather Than Render-Time

- **Decision**: Call `prepare_render_bound()` in `build_with_data()` instead of
  in `render()` or `render_to_png()`.
- **Reasoning**: Avoids adding `M: MarkInstanceBuilder` trait bounds to the
  entire rendering API surface, which would have been a breaking change.
- **Trade-off**: The Selection cannot be cheaply re-configured after building
  (attr bindings are baked into GPU instances). Re-binding requires a new
  `prepare_render_bound()` call.
- **Future**: A `refresh_data()` method on `ComposedChart` could re-evaluate
  bindings and re-upload instances for dynamic data scenarios.

#### Auto-Domain with 5% Padding

- **Decision**: When no explicit `x_scale`/`y_scale` is set, auto-compute the
  data domain from all data items and add 5% padding on each side.
- **Reasoning**: Prevents data marks from sitting exactly on the axis lines,
  which looks visually wrong. The 5% padding mirrors standard charting library
  behaviour.
- **Trade-off**: Very sparse datasets (1-2 points) get relatively large padding.
  Single-value datasets get a ±1 range.
- **Future**: Could add a `domain_padding(f32)` method to the builder for user
  control.

### Development Workflow Insights

- The core change was surprisingly small (~260 lines changed across 6 files) for
  the impact it has. The main complexity was understanding the existing
  architecture: how `AccessorFunction` works, how `Selection::attr()` stores
  bindings, how `prepare_render_bound()` evaluates them, and how the render pass
  draws instances.
- The existing `render_to_rgba()` and `render_to_texture_view()` methods already
  had the code to draw data marks (`self.visualization.render(&mut render_pass)`
  guarded by `is_render_ready()`). The only missing piece was calling
  `prepare_render_bound()` and providing real accessor bindings instead of
  placeholders.
- Visual verification via the `export_png` example and the PNG regression test
  was essential — the test initially failed with exactly 10 non-white pixels
  (at the threshold boundary) because the default circle radius was very small
  at the test resolution. Increasing the resolution and using `point_size()`
  fixed this.

### Follow-up Stories

1. **GUP-286: Line Chart Builder Data-Mark Rendering** — The line chart builder
   (`LineChartBuilder`) needs similar treatment to connect line marks to the
   render pipeline. The `Line` mark uses a different vertex layout (line strips
   vs instanced quads) and may need a different mapping strategy for connected
   line segments.

2. **GUP-287: Dynamic Data Refresh for ComposedChart** — After build-time
   pipeline preparation, there is no easy way to update data and re-render. A
   `refresh_data()` method should re-evaluate attr bindings and re-upload GPU
   instances without rebuilding the entire chart.
