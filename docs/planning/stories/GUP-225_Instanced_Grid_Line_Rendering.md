# GUP-225: Instanced Grid Line Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: 📋 Planned

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

- [ ] Grid lines rendered via GPU instancing
- [ ] Per-instance data: position along perpendicular axis, line length, color
- [ ] Performance improvement measured for dense grids
- [ ] Visual output identical to current vertex-pair approach
- [ ] Backward compatible with existing grid API

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

- [ ] GPU instancing implemented for grid lines
- [ ] Benchmark shows measurable improvement for dense grids
- [ ] All existing grid tests pass
- [ ] Visual output unchanged
