# GUP-268A: Data Mark Rendering in PNG Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-07-18 **Completed**: 2025-07-19

## Context

GUP-268 delivered PNG export with off-screen GPU rendering, staging-buffer
readback, and PNG encoding. The current implementation renders the chart frame —
axes, tick marks, and grid lines — but does not yet render data marks (circles,
rectangles, lines) produced by the `Selection` GPU pipeline. This is because
`ComposedChart::render_to_png` uses `prepare_draw_commands` which only uploads
axis/tick/grid geometry, while the data visualization lives in the `Selection`'s
separate GPU buffer pipeline.

This story wires the `Selection::prepare_render` and mark draw pipeline through
the PNG export path so that exported images show the full chart including data
points.

## User Story

> "As a visualisation developer, I want PNG exports to include the data marks
> (scatter points, bars, lines) — not just the axes and grid — so that the
> exported image faithfully represents the complete chart."

## Acceptance Criteria

- [x] `render_to_png` output includes data marks rendered by the chart's
      `Selection` (circles, rectangles, lines, etc.).
- [x] Data marks are correctly positioned and coloured in the exported image.
- [x] The `export_png` example produces a PNG with visible data points.
- [x] No regression in existing PNG export tests.

## Technical Tasks

- [x] Wire `Selection::prepare_render` into `ComposedChart::render_to_png` so
      that mark GPU buffers are uploaded before the export render pass.
- [x] Issue the Selection's draw commands into the off-screen render pass
      alongside the existing axis/tick/grid draw calls.
- [x] Handle pipeline format compatibility between the Selection's render
      pipeline and the off-screen texture format (`Bgra8UnormSrgb`).
- [x] Update the `export_png` example to verify data marks appear.
- [x] Add a visual integration test comparing exported pixel data against
      expected mark positions.

## Dependencies

### Prerequisite Stories

- GUP-268: PNG Export ✅ — provides the off-screen rendering and PNG encoding
  infrastructure.

## Testing Strategy

- **Integration test**: Export a chart with known data points, decode the PNG,
  and assert that pixels at expected mark positions are non-white.
- **Visual validation**: Run the export example and inspect the output PNG.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

Wired the `Selection` GPU render pipeline through the PNG export path so that
data marks (circles, rectangles, lines, etc.) appear in exported images
alongside axes, ticks, and grid lines.

### Key Changes

| File | Change |
| --- | --- |
| `src/chart_builder.rs` | Added `visualization.render()` calls to `render_to_rgba` and `render_to_texture_view` render passes |
| `examples/export_png.rs` | Added `prepare_render` with clip-space mapping so data marks are visible |
| `tests/png_export_integration.rs` | Added 2 integration tests: marks-appear and graceful-no-marks |

### Pipeline Compatibility

The Selection's render pipeline already targets `Bgra8UnormSrgb` (hardcoded in
`MarkInfoImpl::create_render_pipeline_impl`), which matches the
`OffscreenTarget` texture format — no format conversion was needed.

### Test Results

- 10 PNG export integration tests pass (8 existing + 2 new)
- All examples compile
- Visual inspection confirms data marks at correct positions in 1x, 2x, and
  large exports
