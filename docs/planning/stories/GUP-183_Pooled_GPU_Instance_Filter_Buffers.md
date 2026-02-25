# GUP-183: Pooled GPU Instance Filter Buffers

**Story ID**: GUP-183 **Title**: Pooled GPU Instance Filter Buffers **Status**:
📋 Planned **Priority**: Medium **Effort**: — **Created**: 2026-07-19
**Dependencies**: GUP-077 (Compute Shader Instance Sorting and Filtering)

## Overview

Pre-allocate and reuse GPU buffers for the `ComputeInstanceFilter` across
frames, eliminating the per-dispatch buffer allocation overhead that dominates
current benchmark results at 100K–1M instance scales.

## Context

GUP-077 implemented the compute shader instance filtering pipeline. Benchmarks
show that buffer allocation (output instances, visibility flags, prefix sums,
draw indirect) accounts for the majority of the GPU path's overhead. At 1M
instances, the GPU dispatch takes ~63ms largely due to creating 4 new buffers
every frame. Pre-allocating buffers for a maximum instance count and reusing
them would reduce GPU overhead to just the compute dispatch time.

## User Story

As a developer rendering 1M+ data points at 60fps, I want the GPU filtering
pipeline to reuse buffers across frames so that per-frame overhead is minimized.

## Acceptance Criteria

- [ ] `PooledComputeInstanceFilter` pre-allocates buffers for a configurable max
      instance count
- [ ] Buffers are reused across `dispatch()` calls without reallocation
- [ ] Automatic buffer growth if instance count exceeds current capacity
- [ ] Benchmark shows >10x improvement vs current per-dispatch allocation
- [ ] API is backward-compatible with existing `ComputeInstanceFilter`

## Technical Tasks

1. Create `PooledComputeInstanceFilter` wrapping `ComputeInstanceFilter`
2. Pre-allocate output, visibility, prefix_sums, and draw_indirect buffers
3. Add capacity tracking and automatic resize
4. Update benchmarks to measure steady-state performance
5. Add reuse tests verifying buffer correctness across multiple dispatches

## Dependencies

- GUP-077: Compute Shader Instance Sorting and Filtering

## Testing Strategy

- Unit tests for buffer reuse across multiple dispatches
- GPU integration tests verifying correctness after buffer reuse
- Benchmarks comparing pooled vs non-pooled at 100K, 1M scales

## Success Metrics

- GPU dispatch time reduced to <5ms at 1M instances (steady state)
- Zero buffer allocations per frame after initial setup
- GPU path faster than CPU path at 1M+ instances

## Risk Assessment

- **Risk**: Buffer over-allocation wastes GPU memory
  - **Mitigation**: Use configurable max capacity; allow shrink after idle

## Definition of Done

- [ ] Implementation compiles and runs
- [ ] Benchmarks show improvement over non-pooled path
- [ ] All existing tests continue to pass
- [ ] Documentation updated
