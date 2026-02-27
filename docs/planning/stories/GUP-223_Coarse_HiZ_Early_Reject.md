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

## Retrospective

**Completed**: 2026-02-28

### Key Technical Learnings

#### Build Coverage Cell Limit Interaction

- **Challenge**: Initial GPU tests for large marks failed because the
  `build_coverage` pass has a 4096-cell-per-instance limit. Large marks (radius
  0.8+) at `tile_size=4` exceed this limit, causing incomplete coverage maps and
  incorrect occlusion results.
- **Solution**: Used `tile_size=16` for large-mark tests, reducing the base grid
  from 200×150 to 50×38 and keeping cell counts under the limit.
- **Pattern**: When testing GPU features that depend on coverage map
  completeness, adjust tile_size to keep marks within the `max_cells` budget.
  This is a pre-existing constraint that affects all occlusion testing, not just
  the coarse Hi-Z path.

#### WGSL Unsigned Integer Loop Boundaries

- **Challenge**: WGSL `u32` underflow — iterating
  `for (var level = num_levels-1; level >= 1; level--)` would wrap at 0 and loop
  indefinitely.
- **Solution**: Iterate forward from level 1 upward, tracking the best level
  seen so far. Break early when the mark no longer meets the ≥4-cells-per-axis
  threshold at higher (coarser) levels, since cell counts only decrease.
- **Pattern**: Always iterate forward with unsigned integers in WGSL compute
  shaders.

#### Hi-Z Mip Chain Semantics for Correctness

- **Challenge**: Understanding when the coarse-level occlusion check is safe.
  Each mip level stores the MINIMUM z of its 2×2 children, so
  `min_z > instance_z` means ALL children have z > instance_z (mark is fully
  behind). But interior cells only cover a subset of the mark's footprint.
- **Solution**: The interior-cell approach is correct for both the "visible" and
  "occluded" cases because: (a) for visibility, if ANY interior cell has
  `min_z == 0` or `min_z ≤ z`, the mark has visible pixels in that region; (b)
  for occlusion, interior cells cover a contiguous region fully within the
  mark's bounding box, and the min_z property ensures all level-0 children are
  occluded.
- **Pattern**: Min-mip chains enable safe coarse-level queries for regions fully
  contained within the query bounds.

### Architectural Decisions

#### Shared Helper Function Approach

- **Decision**: Extracted `coarse_hiz_test()` as a WGSL helper function called
  by both `occlusion_test` and `occlusion_test_combined`.
- **Reasoning**: DRY principle — both entry points need identical two-level
  logic. The helper returns a tri-state result (VISIBLE/OCCLUDED/AMBIGUOUS)
  using const codes.
- **Trade-off**: Slightly more complex control flow (two return paths) vs
  duplicated code.
- **Future**: The helper could be extended to support N-level cascading tests if
  needed.

#### No Rust-Side Changes Required

- **Decision**: The entire optimization is in the WGSL shader; no changes to
  Rust structs, pipeline creation, or buffer management.
- **Reasoning**: The Hi-Z mip chain is already built by GUP-076. The two-level
  test simply reads existing mip levels. No new uniform parameters or pipeline
  variants are needed.
- **Trade-off**: No way to configure the coarse threshold (4 cells per axis)
  from Rust, but this is a reasonable fixed heuristic.
- **Future**: If the threshold needs tuning, add a field to
  `OcclusionGpuConfig`.

### Development Workflow Insights

- The story was very focused — only the WGSL shader needed modification, with
  Rust changes limited to tests, benchmarks, and documentation. This made the
  implementation clean and fast.
- The `build_coverage` 4096-cell limit is a practical constraint that surfaced
  during testing. It's documented in the test code but may warrant its own story
  if large-mark coverage becomes a bottleneck in real workloads.
- Running `mask all-fix` caught a minor formatting issue in the benchmark code
  (multi-line `BenchmarkId::new` call that could fit on one line).

### Follow-up Stories

1. **GUP-234: Adaptive Build Coverage Cell Budget** — The current 4096-cell
   limit per instance in `build_coverage` prevents large marks from fully
   populating the coverage map at fine tile sizes. An adaptive approach (e.g.,
   writing at a coarser level for large marks, or using tile-size-aware budgets)
   would improve coverage completeness for mixed-size datasets.
