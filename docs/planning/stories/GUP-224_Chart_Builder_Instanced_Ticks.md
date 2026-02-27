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

## Retrospective

**Completed**: 2025-07-21

### Key Technical Learnings

#### Separation Strategy: Additive Not Replacement

- **Challenge**: The story required migrating from vertex-pair tick rendering to
  instanced rendering without breaking the existing API that returns
  `(Vec<Vertex>, Vec<AxisLabel>)`.
- **Solution**: Introduced `AxisGeometry` as a richer return type with an
  `into_legacy()` method that reconstructs the old tuple format. The existing
  `generate_axis_geometry()` delegates to the new instanced path and flattens
  the result. This is zero-cost for callers who don't opt in.
- **Pattern**: When migrating from a simple API to a richer one, create the new
  struct first, add a conversion method to the old format, then have the old
  method delegate through the new one. This guarantees behavioral equivalence.

#### TickPipeline Lifecycle in ComposedChart

- **Challenge**: `ComposedChart` derives `Debug`, but `wgpu::RenderPipeline`
  does not implement `Debug`. Also, the pipeline requires a `wgpu::Device` and
  `TextureFormat` which aren't available at construction time.
- **Solution**: Added a manual `Debug` impl for `TickPipeline` and used lazy
  initialization in `render()` via `Option<TickPipeline>`. The two-phase pattern
  (prepare in `render()`, draw in `draw_ticks()`) matches the existing
  architecture where render passes are created externally.
- **Pattern**: For GPU pipeline objects that require device context, use
  `Option<Pipeline>` with lazy initialization. For Debug constraints, prefer
  manual impls with opaque field descriptions.

#### Vertex Position Equivalence Testing

- **Challenge**: Ensuring the instanced path produces exactly the same visual
  output as the vertex-pair path. The two code paths compute endpoints
  differently (instanced stores `position + tick_vector`, vertex pairs compute
  both endpoints directly).
- **Solution**: The `test_instanced_matches_legacy_vertex_positions` test
  generates both formats and compares every vertex position with `1e-6`
  tolerance. This catches any floating-point divergence between the paths.
- **Pattern**: When introducing an alternative rendering path, always have a
  "round-trip" test that converts the new format back to the old and compares.

### Architectural Decisions

#### AxisGeometry as Public API

- **Decision**: Made `AxisGeometry` a public struct in `chart_builder` with
  public fields, rather than hiding it behind methods.
- **Reasoning**: Downstream callers (like examples) need direct access to
  `line_vertices` and `tick_instances` to feed them into their own render
  passes. Getter methods would add boilerplate without benefit.
- **Trade-off**: Public fields are harder to evolve. If the struct needs to
  change, it's a breaking API change.
- **Future**: If additional geometry types are needed (e.g., grid lines),
  `AxisGeometry` could be extended or replaced with a more general
  `ChartGeometry`.

#### Two-Phase Draw Pattern (render + draw_ticks)

- **Decision**: `render()` prepares GPU buffers, `draw_ticks()` records draw
  commands into a caller-provided render pass.
- **Reasoning**: The existing render architecture follows the "single render
  pass" principle from CLAUDE.md. The caller creates the render pass and
  controls draw order. The chart builder shouldn't own the render pass.
- **Trade-off**: Requires the caller to call two methods instead of one.
- **Future**: A higher-level `render_all()` method could combine both phases
  when the caller doesn't need fine-grained control.

### Development Workflow Insights

- The `generate_line_vertices()` method already existed in `AxisRenderer` (added
  by GUP-204 as part of the separation), which made the migration
  straightforward. Good API design in GUP-204 paid forward.
- The commented-out test block in `chart_builder.rs` (from the original
  Selection integration work) provided useful patterns for test structure, even
  though those tests can't be enabled yet.
- The `multi_font_chart_demo` example was the only caller that needed manual
  migration. All other callers use `generate_axis_geometry()` which is backward
  compatible through delegation.
- The pre-existing flaky `test_registry_scalability` test (GUP-233) caused a
  false alarm during full-suite validation. Important to document known flakes.

### Follow-up Stories

1. **GUP-225: Instanced Grid Line Rendering** — Apply the same instancing
   pattern to grid lines (already planned in INDEX.md). Grid lines have even
   more lines per chart than ticks, so the data reduction benefit is larger.

2. **GUP-239: Pipeline Caching for Chart Builder** — The `multi_font_chart_demo`
   currently recreates the axis line pipeline every frame. The `TickPipeline` is
   cached but the `LineList` pipeline is not. A `PipelineCache` integration
   would avoid this overhead.
