# GUP-234: Adaptive Build Coverage Cell Budget

**Story ID**: GUP-234 **Title**: Adaptive Build Coverage Cell Budget **Status**:
✅ Complete **Priority**: Low **Effort**: — **Created**: 2026-02-28
**Completed**: 2026-07-16 **Dependencies**: GUP-076 (GPU Occlusion Culling),
GUP-223 (Coarse Hi-Z Early Reject)

## Overview

The `build_coverage` pass in the occlusion culling shader has a fixed 4096-cell
limit per instance to prevent very long loops for huge marks. For large marks at
fine tile sizes (e.g., tile_size=4), this limit causes incomplete coverage maps
— only a portion of the mark's cells are written. This reduces the effectiveness
of both occlusion testing and the coarse Hi-Z early reject (GUP-223).

An adaptive approach — writing at a coarser level for large marks or using a
tile-size-aware budget — would improve coverage completeness for mixed-size
datasets without impacting performance for small marks.

## Context

GUP-223 discovered during testing that large marks (radius > 0.5 in clip space)
exceed the 4096-cell limit when using tile_size=4 on a default viewport (200×150
cells). Tests had to use tile_size=16 to keep cell counts manageable. In
production, users may prefer fine tile sizes for better culling accuracy of
small marks while also having large background marks in the scene.

## User Story

As a developer rendering mixed-size datasets with fine tile sizes, I want the
coverage map to be complete for large marks so that occlusion culling is
effective regardless of mark size.

## Acceptance Criteria

- [x] Large marks fully populate the coverage map regardless of tile size
- [x] Small marks are not affected (performance maintained)
- [x] No increase in GPU memory usage for small-mark-only datasets
- [x] Coarse Hi-Z early reject (GUP-223) works with tile_size=4 for large marks

## Technical Tasks

1. Evaluate approaches: coarse-level writes for large marks, adaptive cell
   budget, or tile-size-aware limits
2. Implement chosen approach in `build_coverage` entry point
3. Update tests to verify coverage completeness with tile_size=4 and large marks
4. Benchmark to ensure no regression for small-mark datasets

## Dependencies

- GUP-076: GPU Occlusion Culling (provides build_coverage entry point)
- GUP-223: Coarse Hi-Z Early Reject (benefits from improved coverage)

## Testing Strategy

- Verify coverage map completeness for marks at various radii and tile sizes
- Compare occlusion results with and without the adaptive budget
- Benchmark small-mark-only vs mixed-size datasets

## Success Metrics

- Complete coverage for marks up to radius 1.0 at tile_size=4
- No performance regression for small-mark datasets (< 2% variation)
- GUP-223 coarse tests work correctly at tile_size=4

## Risk Assessment

- **Risk**: Writing at coarser levels introduces approximation in the coverage
  map
  - **Mitigation**: Use conservative (larger) bounding boxes at coarser levels
    to avoid undercounting

## Definition of Done

- [x] Adaptive coverage implemented and tested
- [x] Benchmarks confirm no regression
- [x] Documentation updated

## Implementation Summary

### What was implemented

- **WGSL shader** (`src/shaders/occlusion_culling.compute.wgsl`):
  - `build_coverage` now computes the finest mip level where each mark's cell
    count fits within the 4096-cell budget. Small marks still write directly to
    level 0. Large marks write to coarser levels.
  - New `fill_coverage_down` entry point propagates non-zero values at a coarse
    level to their 2×2 children at the next finer level using `atomicMax`.
    Dispatched once per level from coarsest to level 1.
- **Rust pipeline** (`src/mark/occlusion_culler.rs`):
  - Added `fill_coverage_down_pipeline` to `OcclusionCuller`.
  - Updated `dispatch`, `encode_combined`, and `PooledOcclusionCuller::dispatch`
    to run fill-down passes between `build_coverage` and `generate_hiz_level`.
  - Updated module documentation to describe the 4-pass pipeline.
- **Tests**: 3 new GPU tests, 4 existing tests updated to use `tile_size=4`.
- **Benchmarks**: Mixed-size benchmark updated from `tile_size=16` to
  `tile_size=4`.

### Key files changed

| File                                         | Change                                       |
| -------------------------------------------- | -------------------------------------------- |
| `src/shaders/occlusion_culling.compute.wgsl` | Adaptive build_coverage + fill_coverage_down |
| `src/mark/occlusion_culler.rs`               | Pipeline + dispatch changes + 3 new tests    |
| `benches/occlusion_culling_benchmarks.rs`    | Mixed-size benchmark uses tile_size=4        |

### Test counts

- 20 occlusion culler tests (17 existing + 3 new), all passing
- 7 unified culling pipeline tests, all passing

## Retrospective

**Completed**: 2026-07-16

### Key Technical Learnings

#### Multi-Level Write + Fill-Down is Cleanest Approach

- **Challenge**: Several approaches were considered — removing the budget,
  strided writes, direct coarse-level writes, or multi-level writes with
  fill-down. The key constraint is that `generate_hiz_level` uses `atomicStore`
  (not `atomicMin`), so any direct writes to coarser levels would be
  overwritten.
- **Solution**: Write at the finest level that fits within the 4096-cell budget,
  then run a fill-down pass (`fill_coverage_down`) from coarsest to finest. Each
  fill step reads one coarse cell and writes `atomicMax` to its 4 children.
  After all fill passes, level 0 is complete, and `generate_hiz_level` runs on
  the fully-populated data.
- **Pattern**: When GPU passes have write-order dependencies (later passes
  overwrite earlier writes), use a separate fill/propagation pass between them
  rather than trying to merge the logic.

#### Fill-Down Pass Cost is Negligible

- **Challenge**: Adding up to 7 extra compute dispatches (one per mip level)
  could add overhead.
- **Solution**: The coarser levels have very few cells (4, 12, 35, 130, 475,
  1900, 7500 for a 200×150 base grid). Each dispatch is ≤30 workgroups. The
  total fill-down work (~10K cells) is trivial compared to the build_coverage
  pass (~30K+ cells) and the occlusion test.
- **Pattern**: Mip-chain operations have exponentially decreasing cost at
  coarser levels, making multi-dispatch approaches practical.

#### atomicMax Preserves Correctness Across Small and Large Marks

- **Challenge**: When fill-down propagates a large mark's z-value to level 0,
  small marks that already wrote their own z-values to those same cells must not
  be overwritten incorrectly.
- **Solution**: Both build_coverage (small marks) and fill_coverage_down use
  `atomicMax`, so the highest z-value (latest-drawn mark) always wins. This is
  correct for the coverage semantics (coverage map records the frontmost mark at
  each cell).
- **Pattern**: `atomicMax` is the correct merge operation for z-order coverage
  maps where higher z = drawn later = on top.

### Architectural Decisions

#### Fill-Down as Separate Entry Point

- **Decision**: Added `fill_coverage_down` as a new WGSL entry point rather than
  modifying `generate_hiz_level` or `build_coverage`.
- **Reasoning**: Clean separation of concerns. `build_coverage` handles
  per-instance logic, `fill_coverage_down` handles per-cell propagation,
  `generate_hiz_level` handles MIN-based mip chain construction. Each pass has a
  single, well-defined responsibility.
- **Trade-off**: One more pipeline to create and dispatch vs. cleaner code.
- **Future**: The fill-down pass could potentially be fused with
  `generate_hiz_level` by using `atomicMax` for the first (downward) pass and
  then a separate MIN pass, but the current approach is simpler and fast enough.

#### No Configuration Changes

- **Decision**: No new fields in `OcclusionParams` or `OcclusionGpuConfig`. The
  adaptive behavior is entirely internal to the shader.
- **Reasoning**: The 4096-cell budget is an implementation detail, not a
  user-tunable parameter. Users control tile_size and conservative_margin; the
  adaptive level selection is an optimization that should "just work."
- **Trade-off**: No way to disable the adaptive behavior from Rust, but there's
  no reason to want to.
- **Future**: If profiling reveals the fill-down is a bottleneck for some
  workload (unlikely), a config flag could skip it.

### Development Workflow Insights

- The implementation was straightforward once the correct approach was
  identified. The key insight — that fill-down must run before
  `generate_hiz_level` to avoid atomicStore overwrites — required careful
  analysis of the data flow between passes.
- All 17 existing occlusion tests passed immediately after the shader change,
  confirming backward compatibility. The only change needed was updating the 4
  large-mark tests from tile_size=16 to tile_size=4.
- Three dispatch methods needed updating (`dispatch`, `encode_combined`,
  `PooledOcclusionCuller::dispatch`). This highlights that the occlusion
  pipeline encoding logic could benefit from a shared helper method to reduce
  duplication.

### Follow-up Stories

No follow-up stories identified. The adaptive coverage approach fully resolves
the 4096-cell constraint. The existing occlusion pipeline architecture is
well-factored and doesn't need further refactoring for this feature.
