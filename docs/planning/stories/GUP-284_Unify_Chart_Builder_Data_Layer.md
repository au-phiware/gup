# GUP-284: Unify Chart Builder Data-Layer Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 📋 Planned **Created**:
2025-07-18

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

- [ ] `ComposedChart::render_to_png()` produces a PNG with visible data marks
      (circles for scatter, lines for line charts) in addition to axes and grid.
- [ ] `ComposedChart::render()` records draw commands for data marks into the
      render pass.
- [ ] The `ScatterPlotBuilder` produces a chart where data points are visible
      without additional manual rendering.
- [ ] Existing examples that use the chart builder (`01_hello_chart`,
      `styled_scatter`, etc.) show data marks.
- [ ] Performance: rendering 1000 data points via the builder adds < 2 ms to the
      frame time.

## Technical Tasks

- [ ] Wire the `Selection<T, M>`'s GPU buffers (vertex, instance, index) into
      `ComposedChart`'s render pass.
- [ ] Add a `draw_data_marks()` method to `ComposedChart` analogous to
      `draw_grid_lines()` and `draw_ticks()`.
- [ ] Call `draw_data_marks()` in both `render()` and `render_to_png()`.
- [ ] Update examples to verify visible data marks.

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

- [ ] `render_to_png` produces visible data marks.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
- [ ] At least one example visually confirmed with data marks.
