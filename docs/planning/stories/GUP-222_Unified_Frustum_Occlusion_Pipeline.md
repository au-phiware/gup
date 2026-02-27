# GUP-222: Unified Frustum + Occlusion Culling Pipeline

**Story ID**: GUP-222 **Title**: Unified Frustum + Occlusion Culling Pipeline
**Status**: 🚧 In Progress **Priority**: Medium **Effort**: — **Created**: 2026-02-27
**Dependencies**: GUP-076 (GPU Occlusion Culling), GUP-077 (Compute Shader
Instance Filtering)

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

- [ ] Single `dispatch` call applies frustum culling, then occlusion culling
- [ ] Compaction produces a dense output buffer with `DrawIndirect` parameters
- [ ] Performance is equal to or better than running both pipelines separately
- [ ] API is backward-compatible with existing `ComputeInstanceFilter` users

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

- [ ] Unified pipeline implemented and tested
- [ ] Benchmarks show no regression vs. separate pipelines
- [ ] API documentation updated
- [ ] Backward compatibility maintained
