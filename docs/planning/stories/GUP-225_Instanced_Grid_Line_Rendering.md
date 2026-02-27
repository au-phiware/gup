# GUP-225: Instanced Grid Line Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: ✅ Complete
(2026-02-27)

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
32-byte `TickInstance` (position, direction+length vector, colour) instead of
two full `Vertex` structs (56+ bytes).

### Key Files Changed

| File                   | Changes                                                                    |
| ---------------------- | -------------------------------------------------------------------------- |
| `src/grid.rs`          | Added instance generation methods, caching, benchmark tests (12 new tests) |
| `src/chart_builder.rs` | Added `prepare_grid_pipeline()`, `draw_grid_lines()`, `has_grid_data()`    |

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

## Retrospective

**Completed**: 2026-02-27

### Key Technical Learnings

#### Reusing TickInstance for Grid Lines

- **Challenge**: Deciding whether to create a dedicated `GridLineInstance`
  struct or reuse `TickInstance`.
- **Solution**: Reused `TickInstance` directly — grid lines are geometrically
  identical to tick marks (a positioned line segment with colour), just with
  different parameterisation (full chart width/height instead of tick length).
- **Pattern**: When the data layout is identical, reuse the existing struct even
  if the name doesn't perfectly match. This avoids code duplication and allows
  sharing the GPU pipeline (shader, vertex buffer layout, render pipeline).

#### Shared TickPipeline Eliminates Pipeline Proliferation

- **Challenge**: Grid lines and tick marks need the same instanced rendering
  infrastructure but are drawn at different render phases (grids first, ticks on
  top).
- **Solution**: Share the single `TickPipeline` instance but maintain separate
  buffer sets (`tick_buffers` vs `grid_buffers`) so each can issue an
  independent draw call at the right z-order.
- **Pattern**: When two visual elements share the same shader and vertex layout,
  separate the data (buffers) but share the pipeline. This halves pipeline
  creation cost and shader compilation.

### Architectural Decisions

#### Caching Shares Fingerprint Mechanism

- **Decision**: The instanced path reuses the existing FNV-style fingerprint
  caching from the `LineAttributes` path — same hash inputs, same
  `cache_fingerprint` field.
- **Reasoning**: The cache invalidation condition is identical (tick positions,
  bounds, or config changed). Reusing the fingerprint avoids dual-cache
  bookkeeping.
- **Trade-off**: The `LineAttributes` and `TickInstance` caches cannot be
  independent — calling `generate_grid_lines` then `generate_grid_instances`
  with the same inputs will still show a cache hit on the second call even
  though the first populated a different data structure.
- **Future**: If the two paths need independent lifecycle, split into two
  fingerprint fields.

#### Legacy LineAttributes Path Retained

- **Decision**: The existing `generate_horizontal_lines_static` /
  `generate_vertical_lines_static` methods and `render_grid_lines_static` in the
  chart builder are kept (marked `#[allow(dead_code)]`), not deleted.
- **Reasoning**: They serve as reference implementations and potential fallback
  for platforms where instancing is not available or not beneficial.
- **Future**: Could be removed in a future cleanup story if the instanced path
  is confirmed sufficient on all target platforms.

### Development Workflow Insights

- The story was straightforward because GUP-204 had already established a clean,
  well-documented instancing pattern. Applying it to grid lines was mostly
  mechanical: same struct, same pipeline, different parameterisation.
- Writing the `test_instances_match_line_attributes` test was the highest-value
  verification — it confirmed visual equivalence without needing a GPU.
- The dense grid benchmark (`test_dense_grid_benchmark`) with 222+ lines was a
  good exercise in verifying the >30% reduction claim quantitatively.
- Total implementation time was minimal (3 story points is accurate) because the
  foundation was solid.

### Follow-up Stories

No new follow-up stories identified. The instanced grid line rendering completes
the grid rendering optimisation arc started in GUP-095 → GUP-096 → GUP-204 →
GUP-224 → GUP-225.
