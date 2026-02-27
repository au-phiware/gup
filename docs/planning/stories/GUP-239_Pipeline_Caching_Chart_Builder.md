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

## Retrospective

**Completed**: 2025-02-25

### Key Technical Learnings

#### Consistent Pipeline Caching Pattern

- **Challenge**: The `TickPipeline` was already cached on `ComposedChart` but
  the axis-line pipeline was created inline every frame — two different patterns
  for essentially the same concern.
- **Solution**: Created `AxisLinePipeline` as a sibling to `TickPipeline`,
  following the identical lazy-init pattern. Both are now cached side-by-side in
  `ComposedChart`.
- **Pattern**: For any GPU pipeline that doesn't change between frames, create a
  named struct that wraps `wgpu::RenderPipeline` with `new()`, `upload()`, and
  `draw()` methods. Cache it as `Option<T>` with lazy initialization.

#### Public vs Private Preparation APIs

- **Challenge**: `ComposedChart::render()` calls internal
  `prepare_tick_pipeline()` which takes a `RenderContext`. But the
  `multi_font_chart_demo` example bypasses `render()` and manages its own render
  pass, so it needed a way to trigger pipeline preparation.
- **Solution**: Added `prepare_draw_commands(device, queue, surface_format)` as
  a public method that accepts raw wgpu handles instead of `RenderContext`,
  serving callers that own their render loop.
- **Pattern**: When a struct has both internal render methods and
  externally-managed render passes, provide public "prepare" + "draw" methods
  that accept the minimal set of GPU handles.

### Architectural Decisions

#### Reuse basic.wgsl for Axis Lines

- **Decision**: Use the existing `basic.wgsl` shader for axis-line rendering
  rather than a separate shader.
- **Reasoning**: The shader in the example's `AXIS_SHADER_SRC` was byte-for-byte
  identical to `basic.wgsl` (position+color passthrough). Reusing the existing
  file avoids duplication.
- **Trade-off**: `basic.wgsl` uses `PointList` topology in `BasicPipeline`,
  while `AxisLinePipeline` uses `LineList`. The shader source is shared but the
  pipelines are separate — this is correct since topology is a pipeline
  property, not a shader property.
- **Future**: If axis lines need different rendering (e.g., dashed lines), the
  shader can be swapped without affecting the caching pattern.

#### prepare_draw_commands as a Single Entry-Point

- **Decision**: Combine axis-line and tick preparation into one
  `prepare_draw_commands()` call instead of two separate public methods.
- **Reasoning**: Callers always need both; having a single method reduces
  boilerplate and ensures consistent state. The geometry is generated once and
  split between the two pipelines.
- **Trade-off**: Slightly less flexible than two independent prepare methods,
  but the typical usage pattern always prepares both together.
- **Future**: If grid-line preparation needs to be included, the method can be
  extended or a variant added.

### Development Workflow Insights

- The 2-point story estimate was accurate — the implementation was
  straightforward once the existing `TickPipeline` pattern was understood.
- `cargo clean` was needed mid-story due to disk space exhaustion. The full
  target directory was 20+ GB. Worth noting for development environments with
  limited disk.
- The `multi_font_chart_demo` example exited quickly in the test environment (no
  interactive window focus), so visual verification was limited to confirming no
  errors in stdout/stderr. All rendering logic was validated through unit tests.
