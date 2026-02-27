# GUP-224: Migrate Chart Builder to Instanced Tick Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: ✅ Complete
(2025-07-21)

## Overview

Update `ComposedChart::generate_axis_geometry()` and
`generate_axis_geometry_resolved()` to use the `TickPipeline` and
`generate_tick_instances()` for tick marks instead of the current vertex-pair
approach. The axis line itself continues to use `LineList` vertex pairs.

## Context

GUP-204 introduced GPU-instanced tick rendering as an alternative API. The main
consumer of axis vertices — `ComposedChart` in `chart_builder.rs` — still uses
the vertex-pair path (`generate_axis_vertices()`) for everything. Migrating the
chart builder to instanced ticks would realise the 31% data reduction per tick
and single-draw-call benefit in production rendering.

## User Story

> "As a chart builder user, I want my charts to automatically use the most
> efficient rendering path for tick marks without changing my application code."

## Acceptance Criteria

- [x] `generate_axis_geometry()` returns tick instance data alongside vertices
- [x] `ComposedChart` rendering uses `TickPipeline` for tick marks
- [x] Axis lines continue to use `LineList` vertex pairs
- [x] All existing chart builder tests pass
- [x] Examples render identically before and after

## Technical Tasks

1. Modify `generate_axis_geometry()` to return a richer struct (vertices for
   axis lines, instances for ticks, labels)
2. Create or accept a `TickPipeline` in chart rendering code
3. Update render pass recording to use instanced draw for ticks
4. Ensure backward compatibility for callers that only use `Vec<Vertex>`

## Dependencies

- **GUP-204**: GPU Instance Rendering for Axis Ticks ✅

## Testing Strategy

- Visual regression: rendered charts identical before and after
- Unit: chart builder generates correct instance counts
- Integration: example screenshots match

## Definition of Done

- [x] Chart builder uses instanced tick rendering
- [x] All existing tests pass
- [x] No API breakage for downstream callers

## Implementation Summary

### What Was Implemented

1. **`AxisGeometry` struct** (`src/chart_builder.rs`) — A rich return type that
   separates axis line vertices (`line_vertices: Vec<Vertex>`) from tick
   instance data (`tick_instances: Vec<TickInstance>`) and labels. Includes
   `into_legacy()` to flatten back into the old `(Vec<Vertex>, Vec<AxisLabel>)`
   format.

2. **`generate_axis_geometry_instanced()`** — New method on `ComposedChart` that
   uses `AxisRenderer::generate_line_vertices()` for axis lines and
   `AxisRenderer::generate_tick_instances()` for ticks, producing separated
   data.

3. **`generate_axis_geometry_instanced_resolved()`** — Instanced variant with
   label collision resolution via `LabelPositioner`.

4. **Backward-compatible delegation** — The existing `generate_axis_geometry()`
   now delegates to `generate_axis_geometry_instanced().into_legacy()`, and
   `generate_axis_geometry_resolved()` delegates to the instanced resolved
   variant. No caller needs to change.

5. **`TickPipeline` integration in `ComposedChart`** — `render()` lazily creates
   a `TickPipeline` and uploads tick instance data. `draw_ticks()` records
   instanced draw commands into a render pass. `has_tick_data()` queries
   readiness.

6. **`TickBuffers` struct** — Internal helper holding uploaded `wgpu::Buffer`
   references for the base geometry and instance data.

7. **Debug impl for `TickPipeline`** — Added manual `Debug` implementation so
   `TickPipeline` can be stored in `ComposedChart` (which derives `Debug`).

8. **Example migration** — `multi_font_chart_demo` updated to use the instanced
   path: axis lines drawn with LineList pipeline, ticks drawn via
   `TickPipeline`.

### Key Files Changed

| File                                | Change                                                              |
| ----------------------------------- | ------------------------------------------------------------------- |
| `src/chart_builder.rs`              | AxisGeometry, instanced methods, TickPipeline integration, 12 tests |
| `src/axis.rs`                       | Debug impl for TickPipeline                                         |
| `examples/multi_font_chart_demo.rs` | Migrated to instanced tick rendering                                |

### Test Counts

- **12 new tests** (AxisGeometry unit tests, instanced geometry generation,
  legacy equivalence, data reduction verification)
- **107 existing chart_builder tests** — all pass unchanged
- **1789 total library tests** — all pass
