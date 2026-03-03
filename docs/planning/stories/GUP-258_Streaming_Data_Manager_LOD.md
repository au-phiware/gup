# GUP-258: Streaming Data Manager for LOD

## Story Overview

**Initiative**: Advanced Scale **Status**: ✅ Complete **Created**:
2026-03-02

## Context

The billion-point architecture being assembled across the Advanced Scale
initiative requires more than a static LOD pyramid that is built once from a
snapshot of data. In production deployments — live sensor networks, financial
tick feeds, real-time telemetry dashboards — the dataset never stops growing.
Every new data point must be reflected in the visualisation with minimal
latency, yet rebuilding the entire LOD pyramid on each arrival would be
prohibitively expensive at billion-point scale.

GUP-015 (Incremental GPU Buffer Streaming) introduces the low-level transport
layer: ring buffers, double-buffered GPU memory, and dirty-region tracking that
make sub-millisecond per-point writes possible. GUP-256 (Level-of-Detail
Pyramid) establishes the hierarchical spatial structure — a multi-resolution
pyramid of GPU-backed cells — that the renderer uses to select the correct level
of detail for any given viewport zoom. GUP-244 (Streaming Data Builder API)
provides the ergonomic `DataStream<T>` entry-point that visualization developers
actually interact with when wiring up a live data source.

This story combines those three foundations into a single coherent
`StreamingLodManager<T>`: a component that accepts a `DataStream<T>` as input,
routes each arriving point to the spatially correct pyramid cell, performs an
incremental update of only the affected LOD levels, and enforces a configurable
memory budget by evicting the oldest data when the limit is reached. The result
is a live-updating, memory-bounded, LOD-correct view of a perpetually streaming
dataset — the critical missing piece between raw streaming infrastructure and
billion-point real-time rendering.

## User Story

> "As a visualization developer building a live telemetry dashboard, I want to
> connect a `DataStream<T>` to an LOD pyramid so that arriving data points
> update only the affected pyramid cells incrementally, without requiring a full
> pyramid rebuild, so that I can display billion-point live feeds at stable
> frame rates within a fixed GPU memory budget."

## Acceptance Criteria

### AC1: StreamingLodManager API

- [x] A `StreamingLodManager<T>` type is defined and publicly exported from the
      `gup` crate (or an appropriate sub-module).
- [x] `StreamingLodManager::new(pyramid: LodPyramid, stream: DataStream<T>,     budget: MemoryBudget) -> Self`
      constructs the manager; the constructor validates that the stream's
      element type is compatible with the pyramid's spatial key extractor.
- [x] `StreamingLodManager::poll(&mut self, ctx: &GupContext)` drains all
      pending updates from the `DataStream<T>` and applies incremental mutations
      to the affected pyramid cells; it does not touch cells that received no
      new data in this poll cycle.
- [x] The manager exposes a `pyramid(&self) -> &LodPyramid` accessor so that the
      caller can pass the pyramid reference to the renderer without
      relinquishing ownership of the manager.

### AC2: Incremental LOD Cell Updates

- [x] When a new data point arrives, only the pyramid cells that spatially
      contain that point (one per LOD level) are marked dirty and re-uploaded to
      the GPU; the remaining cells are untouched.
- [x] A unit test verifies that inserting a single point into a pyramid with at
      least four LOD levels causes exactly `depth` cell writes (one per level)
      and zero writes to non-containing cells.
- [x] Batch arrivals within a single `poll` call coalesce writes per cell:
      multiple points that fall into the same cell produce a single GPU upload,
      not one upload per point.

### AC3: Spatial Partitioning / Routing

- [x] Incoming points are routed to pyramid cells by a `SpatialKey` extractor
      that is generic over the data type `T: SpatiallyKeyed` (or equivalent
      trait defined in this story).
- [x] The extractor maps a point's (x, y) coordinates to the cell index at each
      LOD level using the same spatial subdivision scheme as the `LodPyramid`
      (quad-tree or equivalent).
- [x] A unit test verifies that points in distinct quadrants of the data space
      are routed to distinct level-0 cells.
- [x] A unit test verifies that all points, regardless of quadrant, share the
      same root cell at the coarsest LOD level.

### AC4: Memory Budget Enforcement

- [x] `MemoryBudget` is a newtype over `usize` (bytes) and is accepted by the
      `StreamingLodManager` constructor.
- [x] When accumulated GPU memory usage across all pyramid cells reaches the
      configured budget, the manager evicts the oldest data points (by insertion
      order) until usage is at or below the budget.
- [x] Eviction removes points from every LOD level that contained the evicted
      point and triggers an incremental update of only the affected cells.
- [x] A unit test exercises eviction: fill the pyramid past budget, assert that
      total allocated GPU bytes fall at or below the budget after the next
      `poll`, and verify that the oldest points are absent from all LOD levels
      while newer points remain.

### AC5: Demo — Live-Streaming Scatter Plot

- [x] An example `examples/streaming_lod_scatter.rs` compiles and runs without
      GPU validation errors.
- [x] The example constructs a `DataStream<ScatterPoint>` fed by a background
      thread that pushes synthetic (x, y) data at a target rate of 1 M
      points/sec.
- [x] The example renders a scatter plot driven by `StreamingLodManager::poll`
      each frame; the active LOD level changes visibly as the simulated viewport
      zooms in and out.
- [x] A comment in the example documents the measured steady-state frame time
      and peak GPU memory usage on the developer's machine.

## Technical Tasks

- [x] Define the `SpatiallyKeyed` trait (or equivalent) with an associated
      `spatial_key(&self) -> (f32, f32)` method; implement it for a
      `ScatterPoint` struct used in the demo.
- [x] Implement `MemoryBudget` newtype and `EvictionPolicy` (initially
      `OldestFirst` only); add budget accounting to the pyramid cell metadata.
- [x] Implement `StreamingLodManager<T>` struct, holding `LodPyramid`,
      `DataStream<T>`, and `EvictionPolicy`.
- [x] Implement `StreamingLodManager::poll`: drain `DataStream<T>`, route each
      point via `SpatiallyKeyed`, accumulate per-cell dirty sets, flush dirty
      cells to GPU, run budget check and evict if necessary.
- [x] Write unit tests for AC2 (incremental write counts), AC3 (spatial
      routing), and AC4 (eviction and budget enforcement).
- [x] Write `examples/streaming_lod_scatter.rs` satisfying AC5.
- [x] Ensure all new public items carry `///` doc comments with at least one
      usage example in the top-level doc comment of `StreamingLodManager`.
- [x] Run `mask all-fix` and resolve any lint or formatting issues.

## Dependencies

### Prerequisite Stories

- GUP-015: Real-Time Data Streaming Core 📋 — provides `DataStream<T>`,
  `StreamUpdate<T>`, ring-buffer transport, and double-buffered GPU uploads that
  `StreamingLodManager` drains each frame.
- GUP-256: Level-of-Detail Pyramid 📋 — provides the `LodPyramid` structure, the
  per-cell GPU-backed buffers, and the spatial subdivision scheme that
  `StreamingLodManager` writes into.
- GUP-244: Streaming Data Builder API 📋 — provides the ergonomic
  `DataStream::builder()` API that the demo and application code use to
  configure capacity, mode, and backpressure before handing the stream to
  `StreamingLodManager`.

### Enables Stories

- Applications that visualise live sensor data, financial tick feeds, or IoT
  telemetry at billion-point scale with GPU-accelerated LOD rendering are
  directly unblocked by this story — no further infrastructure story is required
  to wire a `DataStream` to a LOD-driven renderer.

## Testing Strategy

- **Unit tests**: Verify incremental cell dirty-counting (AC2), quad-tree
  routing correctness (AC3), and eviction/budget logic (AC4). Use a
  `MockGpuContext` or `wgpu` in test mode to avoid requiring a real GPU in CI.
- **Integration tests**: A headless integration test drives
  `StreamingLodManager::poll` for 1 000 iterations with synthetic data,
  asserting that GPU memory never exceeds the configured budget and that each
  poll touches only the minimal set of cells.
- **Visual validation**: Run `examples/streaming_lod_scatter.rs`, visually
  confirm that the scatter plot updates live and that LOD level changes are
  visible during zoom transitions.
- **Performance**: Measure `poll` throughput with `criterion`; expect to sustain
  ≥ 500 K point insertions per second per poll call on a mid-range GPU before
  frame-time budget is exhausted.

## Success Metrics

- [ ] `StreamingLodManager::poll` sustains ≥ 500 K point insertions/sec in the
      Criterion benchmark without exceeding the configured `MemoryBudget`.
- [ ] Single-point insertion touches exactly `pyramid_depth` GPU cell uploads
      and zero others (verified by unit test).
- [ ] The `streaming_lod_scatter` example runs at ≥ 30 FPS while a background
      thread pushes 1 M synthetic points/sec on reference hardware.
- [ ] `cargo test -- --test-threads=1` passes with zero failures.
- [ ] No GPU validation layer errors or warnings are emitted during the example
      run.

## Risk Assessment

- **Medium**: The incremental cell-update contract depends on `LodPyramid`
  (GUP-256) exposing a cell-level write API rather than only a full-pyramid
  rebuild path. If GUP-256 is implemented with coarser granularity, this story
  may need to negotiate an API extension with that story's implementer.
  _Mitigation_: Treat the cell-level write API as a primary acceptance criterion
  of GUP-256; confirm the interface before starting this story's implementation.

- **Medium**: Accurate memory accounting requires knowing the GPU byte size of
  each pyramid cell including alignment padding. If `LodPyramid` does not expose
  per-cell size metadata, budget enforcement will need to maintain its own
  accounting independently. _Mitigation_: If GUP-256 does not expose cell sizes,
  derive them from `std::mem::size_of::<T>() * cell_capacity` and document the
  approximation.

- **Low**: The 1 M points/sec background thread in the demo may produce results
  that are hardware-dependent and hard to reproduce in CI. _Mitigation_: The
  demo target rate is illustrative; the Criterion benchmark (not the demo) is
  the normative performance gate.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2025-07-17

### Files Changed

| File | Change |
| --- | --- |
| `src/lod/mod.rs` | Added `pub mod streaming`, `pub use streaming::MemoryBudget`; added `from_parts()`, `buffer_mut()`, `metadata_mut()`, `set_allocated_bytes()` crate-internal mutation methods to `LodPyramid` |
| `src/lod/streaming.rs` | **New** — `StreamingLodManager<T>`, `SpatiallyKeyed` trait, `MemoryBudget` newtype, `EvictionPolicy` enum, `ScatterPoint` type, 19 tests |
| `examples/streaming_lod_scatter.rs` | **New** — End-to-end demo: 50K synthetic points, viewport LOD transitions, performance metrics |

### Key Types

- **`SpatiallyKeyed`** — trait with `spatial_key(&self) -> (f32, f32)` for spatial routing
- **`MemoryBudget`** — newtype over `usize` for GPU memory budget configuration
- **`EvictionPolicy`** — `OldestFirst` (default), non-exhaustive for future strategies
- **`ScatterPoint`** — canonical `SpatiallyKeyed + Pod` type for examples/tests
- **`StreamingLodManager<T>`** — main struct combining `LodPyramid`, `DataStream<T>`, and `MemoryBudget`

### Test Count

19 unit/integration tests in `src/lod/streaming.rs`:
- 5 pure unit tests (MemoryBudget, grid geometry, SpatiallyKeyed impls, EvictionPolicy)
- 7 GPU integration tests (construction, insert, coalesce, routing, root cell)
- 3 eviction tests (basic budget, oldest-removed-newest-kept, auto-eviction via poll)
- 2 integration tests (poll drain, 1000-iteration stress)
- 2 type tests (ScatterPoint, EvictionPolicy default)
