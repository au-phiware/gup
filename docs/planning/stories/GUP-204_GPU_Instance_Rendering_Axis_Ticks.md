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

| File                              | Change                                                |
| --------------------------------- | ----------------------------------------------------- |
| `src/axis.rs`                     | TickInstance struct, generation methods, TickPipeline |
| `src/axis_performance.rs`         | Instance caching in AxisGeometryCache                 |
| `src/shaders/tick_instanced.wgsl` | New instanced tick shader                             |

### Performance Comparison

| Metric        | Vertex Pairs          | Instanced                   |
| ------------- | --------------------- | --------------------------- |
| Data per tick | 48 bytes (2 × Vertex) | 32 bytes (1 × TickInstance) |
| Base geometry | 0 bytes               | 8 bytes (shared)            |
| 6 major ticks | 288 bytes             | 200 bytes (31% smaller)     |
| Draw calls    | 1                     | 1 per tick type             |

### Test Counts

- **16 new axis tests** (TickInstance struct, all 4 axis positions, data
  reduction, cache hit/miss, separate major/minor, cached generation)
- **3 new cache tests** (miss→hit, invalidation, independence)
- **42 existing axis tests** — all pass unchanged
- **25 existing cache tests** — all pass unchanged

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### Tick Vector vs Separate Length/Direction

- **Challenge**: The story described per-instance data as "position along axis,
  tick length, color". A naïve approach would store a scalar length and rely on
  the shader to compute the perpendicular direction.
- **Solution**: Storing a 2D `tick_vector` (direction × length) instead of a
  scalar length avoids per-instance direction calculation in the shader and
  keeps the CPU-side generation symmetric with the existing vertex-pair code.
- **Pattern**: When instancing line segments, encode the full offset vector
  rather than decomposing into length + direction. This simplifies the shader to
  a single multiply-add.

#### Base Geometry Design for Line Instancing

- **Challenge**: GPU instancing typically uses quads or triangle meshes as base
  geometry. For line-list ticks, the base geometry is just two vertices.
- **Solution**: Used a single `f32` parameter `t ∈ {0.0, 1.0}` as the base
  vertex buffer (8 bytes total). The vertex shader computes
  `pos = instance.position + instance.tick_vector * t`, which gives the on-axis
  point at t=0 and the tick endpoint at t=1.
- **Pattern**: For instanced line segments, a 1D parameter buffer is the minimal
  base geometry — no index buffer needed, just `draw(0..2, 0..N)`.

#### Cache Dual-Keying

- **Challenge**: The `AxisGeometryCache` already cached `Vec<Vertex>`. Adding
  `Vec<TickInstance>` caching required deciding whether to share or duplicate
  the cache key.
- **Solution**: Used independent cache keys for vertices and instances. This
  allows mixed usage (vertex-pair rendering for the axis line, instanced
  rendering for ticks) without cache thrashing.
- **Pattern**: When extending a cache with a new data type, prefer independent
  key slots over a single shared key to avoid invalidation cross-talk.

### Architectural Decisions

#### Additive API (Not Replacement)

- **Decision**: Added instanced rendering as an alternative path alongside the
  existing vertex-pair API, rather than replacing it.
- **Reasoning**: The vertex-pair approach is used by `chart_builder.rs` and
  other callers who draw axis lines and ticks together in a single `LineList`
  draw call. Forcing callers to switch to instanced rendering would require a
  pipeline change.
- **Trade-off**: Slightly more API surface, but zero breaking changes.
- **Future**: Callers can migrate to instanced ticks incrementally. The
  `TickPipeline` makes it easy to adopt when a caller is ready.

#### TickPipeline Owns Its Pipeline

- **Decision**: `TickPipeline` is a standalone struct that owns a
  `wgpu::RenderPipeline`, rather than extending the existing `BasicPipeline`.
- **Reasoning**: The instanced tick pipeline has fundamentally different vertex
  buffer layouts (1D base + instance buffer) vs `BasicPipeline` (position +
  color per vertex). Sharing would require runtime branching.
- **Trade-off**: One more pipeline object to manage, but clean separation.

### Development Workflow Insights

- The `test_tick_instances_match_vertex_pairs` test was the most valuable: it
  directly proves that the instanced path reconstructs identical line endpoints
  to the vertex-pair path, satisfying the "visual output identical" AC.
- The `bytemuck::Pod` + `#[repr(C)]` pattern made GPU buffer serialization
  trivial — the struct can be uploaded directly with `bytemuck::cast_slice`.
- Pre-existing test failures in `mark::renderer::tests` (3 tests) and pre-commit
  hook failures (markdown formatting, deprecated criterion API) were unrelated
  to this story.

### Follow-up Stories

1. **GUP-224: Migrate chart_builder to instanced tick rendering** — Update
   `ComposedChart::generate_axis_geometry()` to use `TickPipeline` for tick
   marks while keeping vertex pairs for the axis line only. This would realise
   the full performance benefit in production rendering.

2. **GUP-225: Instanced grid line rendering** — Apply the same instancing
   pattern to grid lines, which currently use the same vertex-pair approach as
   ticks but can have even more lines (one per tick across the chart area).
