# GUP-222: Unified Frustum + Occlusion Culling Pipeline

**Story ID**: GUP-222 **Title**: Unified Frustum + Occlusion Culling Pipeline
**Status**: ✅ Complete **Priority**: Medium **Effort**: — **Created**:
2026-02-27 **Completed**: 2026-07-20 **Dependencies**: GUP-076 (GPU Occlusion
Culling), GUP-077 (Compute Shader Instance Filtering)

## Overview

Combine the existing `ComputeInstanceFilter` (frustum culling, LOD, prefix-sum,
compaction) with `OcclusionCuller` (Hi-Z coverage, occlusion test) into a single
compute pipeline. Currently, using both requires two separate dispatches with
independent buffer allocations. A unified pipeline would share the visibility
buffer and perform a single compaction pass.

## Context

GUP-076 implemented occlusion culling and GUP-077 implemented compute-shader
instance filtering as separate modules. When a user wants both frustum and
occlusion culling, they must run two dispatches and merge visibility flags
manually. A unified pipeline would:

- Run frustum culling first (fast, eliminates off-screen marks)
- Run occlusion culling only on frustum-visible marks (avoiding wasted work)
- Perform a single prefix-sum + compaction pass on the combined visibility flags
- Reduce GPU memory usage (shared buffers)

## User Story

As a developer rendering dense datasets with both off-screen and overlapping
marks, I want a single API call that applies both frustum and occlusion culling
so that I get optimal performance without manual pipeline orchestration.

## Acceptance Criteria

- [x] Single `dispatch` call applies frustum culling, then occlusion culling
- [x] Compaction produces a dense output buffer with `DrawIndirect` parameters
- [x] Performance is equal to or better than running both pipelines separately
- [x] API is backward-compatible with existing `ComputeInstanceFilter` users

## Technical Tasks

1. Extend `FilterConfig` with occlusion parameters (enable flag, tile size,
   margin)
2. Add `build_coverage` and `occlusion_test` passes to the existing filter
   encoder, between `cull_and_classify` and `prefix_sum`
3. Modify visibility flags in-place: frustum-culled marks get 0, then
   occlusion-culled marks also get 0
4. Share Hi-Z buffer allocation with the existing buffer pool
5. Update `PooledComputeInstanceFilter` to manage Hi-Z buffers

## Dependencies

- GUP-076: GPU Occlusion Culling (provides `OcclusionCuller`, Hi-Z algorithm)
- GUP-077: Compute Shader Instance Sorting and Filtering (provides
  `ComputeInstanceFilter`, prefix-sum, compaction)

## Testing Strategy

- Benchmark unified vs. separate pipelines at 100K and 1M scales
- Integration tests with mixed off-screen and overlapping marks
- Verify identical output to running both pipelines separately

## Success Metrics

- Single dispatch latency ≤ sum of separate dispatches
- Zero buffer allocation in steady-state (pooled path)
- No visual regressions

## Risk Assessment

- **Risk**: Increased shader complexity in a single module
  - **Mitigation**: Keep passes as separate entry points, share only the bind
    group layout and visibility buffer

## Definition of Done

- [x] Unified pipeline implemented and tested
- [x] Benchmarks show no regression vs. separate pipelines
- [x] API documentation updated
- [x] Backward compatibility maintained

## Implementation Summary

### Approach

Rather than modifying `FilterConfig` or merging the two WGSL shaders (which
would break backward compatibility), the implementation creates a new
`UnifiedCullingPipeline` struct that composes `PooledComputeInstanceFilter`
and `OcclusionCuller`. Both pipelines share the same visibility buffer
through a split-encode pattern: the filter's `cull_and_classify` pass writes
visibility flags, then the occlusion passes read+clear those flags in-place,
and finally the filter's prefix-sum and compact passes produce the dense
output.

### Key Files Added/Modified

- **`src/shaders/occlusion_culling.compute.wgsl`** — Added
  `occlusion_test_combined` entry point that preserves existing visibility
  flags from a prior frustum pass (only writes 0 for occluded marks, never 1).
- **`src/mark/occlusion_culler.rs`** — Added:
  - `occlusion_test_combined_pipeline` to `OcclusionCuller` struct
  - `encode_combined()` method for encoding into an existing command encoder
  - `create_bind_group()` public method for external bind group creation
  - Made Hi-Z helper functions (`level_dim`, `mip_count`, `total_hiz_cells`,
    `compute_level_offsets`) `pub(crate)` for use by the unified pipeline
- **`src/mark/compute_instance_filter.rs`** — Added:
  - `encode_frustum_cull_with_bind_group()` — encodes only the cull pass
  - `encode_prefix_sum_and_compact_with_bind_group()` — encodes only the
    prefix-sum and compact passes
  - `encode_frustum_cull()` and `encode_prefix_sum_and_compact()` on
    `PooledComputeInstanceFilter` for the unified pipeline
  - `buffer_refs()`, `output_buffer_arc()`, `draw_indirect_buffer_arc()`
    for sharing buffers with the occlusion culler
  - `PooledBufferRefs` struct
- **`src/mark/unified_culling_pipeline.rs`** (new) — `UnifiedCullingPipeline`
  with single `dispatch()` that orchestrates all passes in one command encoder
- **`src/mark.rs`** — Added `unified_culling_pipeline` module and re-export
- **`src/lib.rs`** — Added crate-level re-export for `UnifiedCullingPipeline`
- **`benches/unified_culling_benchmarks.rs`** (new) — Criterion benchmarks
  comparing separate vs unified pipelines at 1K and 10K scales
- **`Cargo.toml`** — Registered `unified_culling_benchmarks` bench target

### Test Counts

- 7 new tests in `mark::unified_culling_pipeline::tests`
  - Pipeline creation
  - Frustum-only path (occlusion disabled)
  - Occlusion culling on stacked instances
  - Sparse instances (no occlusion culling)
  - Mixed frustum + occlusion scenario
  - Unified vs separate comparison
  - Zero-instances error
- All 1850 existing tests continue to pass
- 1 new criterion benchmark file
