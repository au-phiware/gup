# GUP-223: Coarse Hi-Z Early Reject for Large Marks

**Story ID**: GUP-223 **Title**: Coarse Hi-Z Early Reject for Large Marks
**Status**: ✅ Complete **Priority**: Low **Effort**: — **Created**: 2026-02-27
**Completed**: 2026-02-28 **Dependencies**: GUP-076 (GPU Occlusion Culling)

## Overview

The current occlusion test (GUP-076) always operates at Hi-Z level 0 (finest
resolution) for correctness. For small marks (1–4 cells) this is optimal, but
for large marks covering hundreds of cells it is unnecessarily slow. A two-level
approach—coarse Hi-Z reject first, then level-0 confirm only for ambiguous
marks—would improve performance for scenes with mixed mark sizes.

## Context

GUP-076 generates a full Hi-Z mip chain but only uses level 0 for the occlusion
test because coarse levels suffer from edge effects (cells straddling the mark
boundary include empty areas). A safe coarse-level test can be achieved by
testing only _interior_ cells (shrinking the test area by one cell on each edge
at the coarse level). This requires the mark to cover at least 4 cells per axis
at the test level.

## User Story

As a developer rendering a mix of small and large marks in a dense dataset, I
want occlusion testing to adapt to mark size so that large marks don't cause
excessive per-thread iteration in the compute shader.

## Acceptance Criteria

- [x] Two-level test: coarse level rejects clearly visible marks quickly
- [x] Level-0 fallback for marks that cannot be resolved at coarse level
- [x] Interior-cell shrinking avoids false positives at coarse levels
- [x] No visual regressions compared to level-0-only testing
- [x] Measurable improvement for datasets with large marks (>32 pixels radius)

## Technical Tasks

1. Add coarse-level selection logic: find the coarsest level where the mark
   covers ≥ 4 cells per axis
2. Implement interior-cell shrinking (inset by 1 cell on each edge)
3. If all interior cells show the mark is visible → mark visible (early out)
4. If all interior cells show occlusion → mark occluded (skip level-0)
5. Otherwise → fall back to level-0 testing

## Dependencies

- GUP-076: GPU Occlusion Culling (provides Hi-Z buffer, mip chain, test
  infrastructure)

## Testing Strategy

- Compare two-level vs. level-0-only output for correctness
- Benchmark with mixed mark sizes (small scatter + large background rectangles)
- Visual regression tests with known dense datasets

## Success Metrics

- ≥ 2× reduction in average per-mark cell iterations for mixed-size datasets
- Identical occlusion results for small marks
- No false positives (incorrectly culled marks)

## Risk Assessment

- **Risk**: Interior-cell shrinking may prevent coarse-level culling for
  medium-sized marks
  - **Mitigation**: Only use coarse level for marks covering ≥ 4 cells per axis;
    smaller marks use level 0 which is already efficient

## Definition of Done

- [x] Two-level test implemented and tested
- [x] Benchmarks show improvement for mixed-size datasets
- [x] No visual regressions
- [x] Documentation updated

## Implementation Summary

### What was implemented

- **WGSL shader** (`src/shaders/occlusion_culling.compute.wgsl`): Added
  `coarse_hiz_test()` helper function that implements the two-level Hi-Z
  approach. Both `occlusion_test` and `occlusion_test_combined` entry points now
  call this helper before falling back to the existing level-0 test.
- **Algorithm**: Iterates mip levels 1..N to find the coarsest where the mark
  covers ≥ 4 cells per axis, shrinks by 1 cell on each edge (interior only),
  checks all interior cells with early exit on ambiguity.
- **Rust tests** (`src/mark/occlusion_culler.rs`): 5 new GPU tests covering
  visible large marks, occluded large marks, mixed sizes, stacked large marks,
  and partially visible large marks.
- **Benchmarks** (`benches/occlusion_culling_benchmarks.rs`): New
  `bench_mixed_size_dispatch` benchmark with mixed small scatter + large
  background datasets.

### Key files changed

| File                                         | Change                                               |
| -------------------------------------------- | ---------------------------------------------------- |
| `src/shaders/occlusion_culling.compute.wgsl` | Added `coarse_hiz_test()`, updated both test entries |
| `src/mark/occlusion_culler.rs`               | Updated module doc, added 5 GPU tests                |
| `benches/occlusion_culling_benchmarks.rs`    | Added mixed-size benchmark                           |

### Test counts

- 17 occlusion culler tests (12 existing + 5 new), all passing
- 7 unified culling pipeline tests, all passing
