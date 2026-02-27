# GUP-225: Instanced Grid Line Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: ✅ Complete (2026-02-27)

## Overview

Apply the GPU instancing pattern from GUP-204 (tick marks) to grid lines. Grid
lines use the same vertex-pair approach as ticks but can have significantly more
lines (one per major/minor tick across the full chart area). Instanced rendering
would reduce vertex upload cost and unify the rendering pipeline.

## Context

The grid system currently generates vertex pairs for each grid line, similar to
axis ticks. For dense grids (minor subdivisions enabled on both axes), this can
produce hundreds of line segments. The `TickInstance` pattern — a base line
segment instanced with per-line position and length — applies directly to grid
lines with a slightly different parameterisation (full chart width/height
instead of tick length).

## User Story

> "As a developer rendering dense grids with minor subdivisions, I want grid
> lines to render efficiently so that enabling minor grids doesn't impact
> performance."

## Acceptance Criteria

- [x] Grid lines rendered via GPU instancing
- [x] Per-instance data: position along perpendicular axis, line length, color
- [x] Performance improvement measured for dense grids
- [x] Visual output identical to current vertex-pair approach
- [x] Backward compatible with existing grid API

## Technical Tasks

1. Define `GridLineInstance` struct (or reuse `TickInstance`)
2. Add instance generation to the grid renderer
3. Create or reuse an instanced pipeline for grid lines
4. Benchmark dense grids (e.g. 10×10 minor subdivisions)
5. Update grid geometry cache

## Dependencies

- **GUP-204**: GPU Instance Rendering for Axis Ticks ✅ (provides the instancing
  pattern)
- **GUP-095**: GPU-Accelerated Grid Lines ✅ (provides grid rendering
  infrastructure)

## Testing Strategy

- Visual regression: instanced grid lines match vertex-pair grid lines
- Performance benchmark: measure vertex count reduction for dense grids
- Integration: works with existing grid configuration options

## Definition of Done

- [x] GPU instancing implemented for grid lines
- [x] Benchmark shows measurable improvement for dense grids
- [x] All existing grid tests pass
- [x] Visual output unchanged

## Implementation Summary

### What Was Implemented

Reused the existing `TickInstance` struct and `TickPipeline` from GUP-204 to
render grid lines via GPU instancing. Each grid line is represented by a single
32-byte `TickInstance` (position, direction+length vector, colour) instead of two
full `Vertex` structs (56+ bytes).

### Key Files Changed

| File                   | Changes                                                                 |
| ---------------------- | ----------------------------------------------------------------------- |
| `src/grid.rs`          | Added instance generation methods, caching, benchmark tests (12 new tests) |
| `src/chart_builder.rs` | Added `prepare_grid_pipeline()`, `draw_grid_lines()`, `has_grid_data()` |

### New Public API

- `GridRenderer::generate_horizontal_instances_static()` — horizontal grid line
  instances
- `GridRenderer::generate_vertical_instances_static()` — vertical grid line
  instances
- `GridRenderer::generate_grid_instances()` — all grid instances with caching
- `GridRenderer::grid_instance_count()` — cached instance count
- `GridSystem::generate_grid_instances()` — delegate to renderer
- `ComposedChart::draw_grid_lines()` — record instanced draw for grids
- `ComposedChart::has_grid_data()` — check if grid buffers are ready

### Performance Results

- Dense grid (10×10 minor subdivisions, 222+ lines): **>30% data reduction**
- Instance data: 32 bytes per line vs 56+ bytes per vertex pair
- Shared `TickPipeline` — no new shader or pipeline allocation

### Test Count

- 12 new tests (10 in `grid.rs`, 2 benchmark/integration)
- All 53 grid tests pass (43 existing + 10 new)
- All 40 chart_builder tests pass
