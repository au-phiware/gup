# GUP-224: Migrate Chart Builder to Instanced Tick Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: 🚧 In Progress

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

- [ ] `generate_axis_geometry()` returns tick instance data alongside vertices
- [ ] `ComposedChart` rendering uses `TickPipeline` for tick marks
- [ ] Axis lines continue to use `LineList` vertex pairs
- [ ] All existing chart builder tests pass
- [ ] Examples render identically before and after

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

- [ ] Chart builder uses instanced tick rendering
- [ ] All existing tests pass
- [ ] No API breakage for downstream callers
