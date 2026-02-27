# GUP-239: Pipeline Caching for Chart Builder

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 2 **Status**: ✅ Complete
(2025-02-25)

## Overview

The `multi_font_chart_demo` (and likely other chart rendering code) recreates
the axis-line `wgpu::RenderPipeline` on every frame because it is built inline
inside the render loop. The `TickPipeline` introduced in GUP-204 is already
cached on the `ComposedChart`, but the `LineList` pipeline for axis lines is
not. Caching the axis-line pipeline would eliminate redundant pipeline creation
and improve frame-to-frame performance.

## Context

GUP-224 migrated tick marks to `TickPipeline`, which is lazily created and
stored on `ComposedChart`. The axis-line drawing still requires callers to
create their own `RenderPipeline`, which in the example code happens every
frame. This is wasteful because the pipeline configuration never changes between
frames.

## User Story

> "As a chart builder user, I want the chart rendering path to cache all GPU
> pipelines so that per-frame overhead is minimised."

## Acceptance Criteria

- [x] Axis-line pipeline is created once and reused across frames
- [x] `ComposedChart` provides a method to draw axis lines into a render pass
- [x] No pipeline recreation on window resize (surface format doesn't change)
- [x] All existing tests pass

## Technical Tasks

1. Store an axis-line `wgpu::RenderPipeline` alongside `TickPipeline` in
   `ComposedChart`
2. Add `draw_axis_lines()` method to record line draw commands
3. Update examples to use the cached pipeline
4. Benchmark before/after frame time

## Dependencies

- **GUP-224**: Migrate Chart Builder to Instanced Ticks ✅

## Testing Strategy

- Unit: pipeline is created once (not per-frame)
- Integration: example renders correctly with cached pipeline
- Performance: no regression in frame time

## Definition of Done

- [x] Axis-line pipeline cached on `ComposedChart`
- [x] `draw_axis_lines()` method available
- [x] Example updated to use cached pipeline
- [x] All existing tests pass

## Implementation Summary

### What was implemented

- **`AxisLinePipeline`** struct in `src/axis.rs` — a cached
  `wgpu::RenderPipeline` using `basic.wgsl` with `LineList` topology for
  axis-line drawing. Mirrors the `TickPipeline` pattern with `new()`,
  `upload()`, and `draw()` methods.
- **`draw_axis_lines()`** and **`has_axis_line_data()`** public methods on
  `ComposedChart` — parallel to the existing `draw_ticks()` / `has_tick_data()`.
- **`prepare_draw_commands()`** public method on `ComposedChart` — a single
  entry-point for callers that manage their own render pass to prepare both
  axis-line and tick pipelines with lazy creation.
- **`multi_font_chart_demo`** updated to use `ComposedChart` cached pipelines,
  removing ~60 lines of per-frame inline pipeline construction and the
  `AXIS_SHADER_SRC` constant.

### Key files changed

| File                                | Change                                                                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `src/axis.rs`                       | Added `AxisLinePipeline` struct with `new()`, `upload()`, `draw()`, `pipeline()`                                                |
| `src/chart_builder.rs`              | Added `axis_line_pipeline` / `axis_line_buffers` fields, `prepare_draw_commands()`, `draw_axis_lines()`, `has_axis_line_data()` |
| `examples/multi_font_chart_demo.rs` | Replaced inline pipeline creation with cached pipeline calls                                                                    |

### Tests added

- `test_axis_line_pipeline_creation` — GPU pipeline creation sanity
- `test_axis_line_pipeline_upload_and_draw_zero` — zero-vertex edge case
- `test_has_axis_line_data_initially_false` — initial state verification
- `test_prepare_draw_commands_creates_axis_line_data` — pipeline + buffer
  creation
- `test_prepare_draw_commands_no_axes_no_data` — no-axis configuration
- `test_prepare_draw_commands_pipeline_reused_across_calls` — pipeline reuse
  across frames

### Test results

- 1861 lib tests passed (0 failed, 4 ignored)
- All examples compile clean
- All 6 new tests pass
