# GUP-204: GPU Instance Rendering for Axis Tick Marks

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: 📋 Planned

## Overview

Replace the current per-tick vertex pair approach with GPU instancing for axis
tick marks. Each tick would be a single instance with position/length uniforms,
reducing vertex count and CPU-side generation cost significantly for axes with
many ticks.

## Context

The current `AxisRenderer` generates two vertices per tick mark (a line segment
pair). For axes with many ticks (especially when minor ticks are enabled), this
produces large vertex arrays that must be uploaded to the GPU each frame on
cache miss. GPU instancing would allow a single quad or line segment to be
instanced across all tick positions, with per-instance data specifying only the
position and length.

## User Story

> "As a developer building charts with dense tick marks, I want axis ticks to
> render efficiently using GPU instancing so that even complex axes with many
> minor ticks don't impact frame rate."

## Acceptance Criteria

- [ ] Tick marks rendered via GPU instancing (single draw call per tick type)
- [ ] Per-instance data: position along axis, tick length, color
- [ ] Performance improvement measured: fewer vertices uploaded per frame
- [ ] Visual output identical to current vertex-pair approach
- [ ] Backward compatible — existing `generate_axis_vertices()` API preserved

## Technical Tasks

1. Define a `TickInstance` struct with position, length, and color fields
2. Create an instanced render pipeline for tick marks
3. Integrate with `AxisRenderer` as an alternative rendering path
4. Benchmark comparison: instanced vs current vertex-pair approach
5. Update `AxisGeometryCache` to cache instance data

## Dependencies

- **GUP-094**: Axis Performance Optimization ✅ (provides caching and LOD
  infrastructure)
- **GUP-074**: Mark Performance Optimization (GPU Instancing) ✅ (provides
  instancing patterns)

## Testing Strategy

- Visual regression: instanced ticks match vertex-pair ticks
- Performance benchmark: measure vertex count reduction
- Integration: works with LOD system from GUP-094

## Definition of Done

- [ ] GPU instancing implemented for tick marks
- [ ] Benchmark shows measurable improvement
- [ ] All existing axis tests pass
- [ ] Visual output unchanged
