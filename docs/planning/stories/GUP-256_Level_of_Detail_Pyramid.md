# GUP-256: Level-of-Detail Pyramid for Billion-Point Rendering

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**:
2025-07-23

## Context

Gup's implementation strategy describes a "Billion-Point Architecture" centred
on a hierarchical Level-of-Detail (LOD) pyramid — a `Vec<GpuBuffer<VertexData>>`
where each tier stores a progressively coarser representation of the full
dataset. At a given zoom level only the appropriate tier is rendered, keeping
GPU throughput proportional to the visible detail rather than to the raw data
size. Without this structure, rendering 100 M+ points requires either full-
dataset passes (GPU-memory bound) or CPU-side decimation (CPU-throughput bound),
neither of which scales to a billion points at interactive frame rates.

The GPU primitives needed to build this system are already in place. GUP-003
delivered typed `GpuBuffer<T>` allocation and upload; GUP-030 added a buffer
pool for efficient reuse; and GUP-077 demonstrated GPU compute pipelines for
per-instance work (frustum culling, LOD classification, prefix-sum compaction)
on datasets up to 10 M instances. This story extends that foundation with a
purpose-built pyramid builder: a compute shader that aggregates raw points into
grid cells to produce each coarser LOD tier, and a host-side `LodPyramid` type
that manages the resulting buffers under a configurable memory budget.

The result is the foundational data structure that GUP-257 (Adaptive Viewport
Renderer) and GUP-258 (Streaming Data Manager) both depend on. Neither of those
stories can progress until the pyramid structure and its build pipeline exist.

## User Story

> "As a visualization developer, I want to build an LOD pyramid from a large
> point dataset on the GPU so that I can render hundreds of millions of points
> at interactive frame rates by selecting the coarsest tier that still looks
> correct at the current zoom level."

## Acceptance Criteria

### AC1: LodPyramid Data Structure

- [ ] A `LodPyramid` struct is defined in the `gup` crate, holding
      `Vec<GpuBuffer<VertexData>>` — one buffer per LOD level (level 0 = full
      resolution, level N = coarsest).
- [ ] `LodPyramid` exposes `fn level_count(&self) -> usize` and
      `fn buffer(&self, level: usize) -> &GpuBuffer<VertexData>`.
- [ ] `LodPyramid` stores per-level metadata: point count, grid cell size, and
      the spatial bounding box of the source data.
- [ ] Construction from CPU-side `&[VertexData]` via `LodPyramidBuilder` is
      supported as a synchronous fallback (for tests and small datasets).

### AC2: GPU Compute Pyramid Builder

- [ ] A WGSL compute shader performs grid-based point aggregation: the data
      space is divided into an N×N grid, and one representative point per
      occupied cell is written to the output buffer.
- [ ] The shader dispatches one workgroup per grid cell (or uses a two-pass
      design), producing a compact output with no CPU readback between levels.
- [ ] Each LOD level is derived from the previous level's output buffer, not
      from the original raw data, so the total build cost scales with the output
      size rather than the input size.
- [ ] The compute pipeline integrates with the `GpuBufferPool` from GUP-030 for
      intermediate and output buffer allocation.
- [ ] Building a 5-level LOD pyramid from 100 M source points completes in under
      10 seconds on a mid-range discrete GPU (measured in the benchmark
      described in AC5).

### AC3: LOD Level Selection Heuristic

- [ ] A pure function
      `fn select_lod_level(viewport: &Viewport2D, point_count: u64, levels: usize) -> usize`
      is provided, implementing `level = f(viewport_size, data_density)`.
- [ ] The heuristic targets a maximum on-screen point density (configurable,
      default 4 points per pixel) to choose the coarsest level whose density
      does not exceed the threshold at the current zoom.
- [ ] The function is unit-tested with representative viewport/density
      combinations, including edge cases (single-level pyramid, fully zoomed in,
      fully zoomed out).
- [ ] The selection logic is documented with a brief explanation of the formula
      in code comments.

### AC4: Configurable Memory Budget

- [ ] `LodPyramidBuilder` accepts a `max_gpu_bytes: u64` field that caps total
      GPU memory consumed across all levels.
- [ ] When the computed pyramid would exceed the budget, the builder drops the
      highest-resolution levels first (fewest points lost) and emits a
      `tracing::warn!` naming the number of levels dropped.
- [ ] The configured budget and actual allocated bytes are exposed as fields on
      the constructed `LodPyramid`.
- [ ] A unit test verifies that a budget smaller than the full pyramid size
      results in a pyramid with fewer levels and no GPU allocation error.

### AC5: Benchmark

- [ ] A Criterion benchmark `benches/lod_pyramid.rs` is added that builds a
      5-level LOD pyramid from 100 M synthetic points and reports total wall
      time.
- [ ] The benchmark is gated behind `#[cfg(feature = "gpu-bench")]` or
      equivalent so it is not required in headless CI environments without a
      GPU.
- [ ] The benchmark result is documented in the story retrospective, including
      hardware configuration.

## Technical Tasks

- [ ] Define `VertexData` layout (or confirm the existing layout from GUP-003/
      GUP-077 is appropriate) and document it in `src/lod/mod.rs`.
- [ ] Implement the `LodPyramid` struct and its `LodPyramidBuilder` in a new
      `src/lod/` module.
- [ ] Write `src/lod/shaders/aggregate.wgsl`: the grid-based point aggregation
      compute shader. Include one representative-point selection strategy (e.g.,
      grid-cell centroid or first-point-wins with atomic CAS).
- [ ] Create the wgpu `ComputePipeline` for `aggregate.wgsl`, binding:
      `@group(0) @binding(0)` input `VertexData` storage buffer (read-only),
      `@group(0) @binding(1)` output `VertexData` storage buffer (read-write),
      `@group(0) @binding(2)` uniform buffer for grid dimensions and bounding
      box.
- [ ] Implement the multi-level build loop: for each level, dispatch the compute
      shader on the previous level's output, then allocate the next output
      buffer from `GpuBufferPool`.
- [ ] Implement `select_lod_level` in `src/lod/selection.rs` with unit tests.
- [ ] Implement memory budget enforcement in `LodPyramidBuilder::build()`.
- [ ] Add a CPU fallback path in `LodPyramidBuilder` (iterates raw data on the
      CPU; used when the wgpu device does not support compute shaders or during
      tests).
- [ ] Write integration tests in `tests/lod_pyramid.rs` exercising the full
      build path on at least three dataset sizes (1 K, 100 K, 1 M points).
- [ ] Add `benches/lod_pyramid.rs` for the 100 M-point benchmark.
- [ ] Export `LodPyramid`, `LodPyramidBuilder`, and `select_lod_level` from the
      crate root under a `lod` re-export, and document them with `///` doc
      comments.

## Dependencies

### Prerequisite Stories

- GUP-003: GPU Buffer Management ✅ — provides `GpuBuffer<T>`, typed allocation,
  and upload helpers used by every pyramid level.
- GUP-004: Basic Render Context ✅ — provides the wgpu `Device` and `Queue`
  needed to create compute pipelines and dispatch workgroups.
- GUP-030: GPU Buffer Pool Management ✅ — provides `GpuBufferPool` for
  efficient reuse of intermediate and output buffers during pyramid
  construction.
- GUP-077: Compute Shader Instance Filtering ✅ — establishes the compute shader
  pipeline pattern (dispatch, prefix-sum compaction, indirect draw integration)
  that this story extends for aggregation.

### Enables Stories

- GUP-257: Adaptive Viewport Renderer — consumes `LodPyramid::buffer(level)` and
  `select_lod_level` to pick the right tier per frame; cannot be implemented
  without this story.
- GUP-258: Streaming Data Manager for LOD — incrementally updates pyramid levels
  as new data arrives; requires `LodPyramid`'s buffer layout and build pipeline
  to be stable.

## Testing Strategy

- **Unit tests**: `select_lod_level` edge cases; memory budget enforcement
  (levels dropped, bytes reported); `LodPyramid` metadata accessors.
- **Integration tests**: Full build pipeline at 1 K, 100 K, and 1 M points using
  the headless wgpu test adapter; assert that each level's point count is
  strictly less than the previous level's; assert no GPU validation errors.
- **Visual validation**: A `examples/lod_pyramid_debug.rs` example that renders
  each LOD level of a synthetic dataset in a grid layout so visual coarsening is
  directly observable; inspect with `cargo run --example lod_pyramid_debug`.
- **Performance**: The Criterion benchmark (`benches/lod_pyramid.rs`) at 100 M
  points targeting <10 s on a mid-range GPU; results recorded in the
  retrospective.

## Success Metrics

- [ ] `LodPyramid` constructs without GPU validation errors for inputs of 1 K,
      100 K, 1 M, and (benchmark-only) 100 M points.
- [ ] Each LOD level contains strictly fewer points than the level below it.
- [ ] `select_lod_level` unit tests pass with 100 % coverage of the heuristic
      branches.
- [ ] Memory budget enforcement test confirms no GPU allocation beyond
      `max_gpu_bytes` is attempted.
- [ ] Benchmark wall time for 100 M points / 5 levels is recorded and is ≤ 10 s.
- [ ] `cargo test -- --test-threads=1` passes; `mask all-fix` produces no
      warnings; `cargo check --examples` succeeds.

## Risk Assessment

- **Medium — Aggregation shader correctness**: Grid-cell representative
  selection using atomic operations in WGSL is non-trivial; first-point-wins
  with `atomicCompareExchangeWeak` may produce incorrect results on some
  backends. _Mitigation_: Start with a deterministic two-pass approach (count
  phase then scatter phase) which avoids atomics in the representative-selection
  step. Include a CPU reference implementation and diff its output against the
  GPU path in integration tests.

- **Medium — 100 M-point benchmark feasibility in CI**: A headless GPU may not
  be available in every CI environment; the 10 s target is hardware-dependent.
  _Mitigation_: Gate the benchmark behind a feature flag and document the
  hardware on which the target was validated. The build itself is
  infrastructure-only; functional correctness is verified at smaller scales.

- **Low — Buffer pool pressure at very high point counts**: Allocating multiple
  large output buffers simultaneously could exhaust the device's VRAM.
  _Mitigation_: The memory budget enforcement in AC4 provides an explicit safety
  valve. Implement build levels sequentially (not all at once) and release
  intermediate buffers back to the pool before allocating the next level.

- **Low — `VertexData` layout mismatch with downstream stories**: GUP-257 and
  GUP-258 may require fields not present in the GUP-003/GUP-077 layout.
  _Mitigation_: Document the chosen layout clearly and treat it as a stable
  public API from the moment this story ships. Flag any layout additions as
  breaking changes.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
