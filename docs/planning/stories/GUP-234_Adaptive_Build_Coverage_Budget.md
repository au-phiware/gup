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
