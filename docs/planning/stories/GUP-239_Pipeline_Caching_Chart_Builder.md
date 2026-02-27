# GUP-239: Pipeline Caching for Chart Builder

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 2 **Status**: 📋 Planned

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

- [ ] Axis-line pipeline is created once and reused across frames
- [ ] `ComposedChart` provides a method to draw axis lines into a render pass
- [ ] No pipeline recreation on window resize (surface format doesn't change)
- [ ] All existing tests pass

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

- [ ] Axis-line pipeline cached on `ComposedChart`
- [ ] `draw_axis_lines()` method available
- [ ] Example updated to use cached pipeline
- [ ] All existing tests pass
