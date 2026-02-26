# GUP-204: GPU Instance Rendering for Axis Tick Marks

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Automatic Scale and
Axis System **Priority**: Low **Story Points**: 3 **Status**: ✅ Complete
(2025-07-18)

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

- [x] Tick marks rendered via GPU instancing (single draw call per tick type)
- [x] Per-instance data: position along axis, tick length, color
- [x] Performance improvement measured: fewer vertices uploaded per frame
- [x] Visual output identical to current vertex-pair approach
- [x] Backward compatible — existing `generate_axis_vertices()` API preserved

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

- [x] GPU instancing implemented for tick marks
- [x] Benchmark shows measurable improvement
- [x] All existing axis tests pass
- [x] Visual output unchanged

## Implementation Summary

### What Was Implemented

1. **`TickInstance` struct** (`src/axis.rs`) — A 32-byte `#[repr(C)]` +
   `bytemuck::Pod` struct with `position: [f32; 2]`, `tick_vector: [f32; 2]`,
   and `color: [f32; 4]`. Includes `instance_buffer_layout()` for pipeline
   integration.

2. **Instance generation methods** (`src/axis.rs`) — Three new methods on
   `AxisRenderer`:
   - `generate_tick_instances()` — all ticks (major first, then minor)
   - `generate_major_tick_instances()` — major ticks only
   - `generate_minor_tick_instances()` — minor ticks only
   - `generate_tick_instances_cached()` — LOD-aware with caching

3. **Instanced shader** (`src/shaders/tick_instanced.wgsl`) — Vertex shader
   accepting base geometry `t ∈ {0, 1}` and per-instance position, tick_vector,
   color attributes. Single draw call renders all ticks.

4. **`TickPipeline` struct** (`src/axis.rs`) — Complete wgpu render pipeline
   with `upload()` and `draw()` methods. Creates base vertex buffer (8 bytes)
   and instance buffer.

5. **Cache extension** (`src/axis_performance.rs`) — `AxisGeometryCache` now has
   `get_instances()` / `store_instances()` alongside the existing vertex cache.

### Key Files Changed

| File | Change |
|------|--------|
| `src/axis.rs` | TickInstance struct, generation methods, TickPipeline |
| `src/axis_performance.rs` | Instance caching in AxisGeometryCache |
| `src/shaders/tick_instanced.wgsl` | New instanced tick shader |

### Performance Comparison

| Metric | Vertex Pairs | Instanced |
|--------|-------------|-----------|
| Data per tick | 48 bytes (2 × Vertex) | 32 bytes (1 × TickInstance) |
| Base geometry | 0 bytes | 8 bytes (shared) |
| 6 major ticks | 288 bytes | 200 bytes (31% smaller) |
| Draw calls | 1 | 1 per tick type |

### Test Counts

- **16 new axis tests** (TickInstance struct, all 4 axis positions, data
  reduction, cache hit/miss, separate major/minor, cached generation)
- **3 new cache tests** (miss→hit, invalidation, independence)
- **42 existing axis tests** — all pass unchanged
- **25 existing cache tests** — all pass unchanged
