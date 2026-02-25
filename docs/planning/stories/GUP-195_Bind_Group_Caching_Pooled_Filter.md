# GUP-195: Bind Group Caching for Pooled Filter

**Story ID**: GUP-195 **Title**: Bind Group Caching for Pooled Filter
**Status**: 🚧 In Progress **Priority**: Low **Effort**: — **Created**:
2025-07-24 **Dependencies**: GUP-183 (Pooled GPU Instance Filter Buffers)

## Overview

Cache the wgpu bind group in `PooledComputeInstanceFilter` when the input buffer
does not change between dispatches. Currently a new bind group is created on
every `dispatch()` call even though the output, visibility, prefix-sum,
draw-indirect, and config buffers are pooled. When the input buffer is also
stable (common in streaming/append scenarios), the bind group can be reused,
further reducing per-frame CPU overhead.

## Context

GUP-183 eliminated per-dispatch buffer allocation, reducing 1M-instance dispatch
from ~64ms to ~11ms. Profiling suggests the remaining CPU-side overhead includes
bind group creation (~µs range) and command encoding. While small individually,
caching the bind group removes one more allocation per frame and simplifies the
encode path.

## User Story

As a developer using `PooledComputeInstanceFilter` with a stable input buffer, I
want the bind group to be cached so that per-frame CPU overhead is further
minimized.

## Acceptance Criteria

- [ ] Bind group is cached when input buffer identity matches the previous
      dispatch
- [ ] Cache is invalidated when buffers are grown or input buffer changes
- [ ] No correctness regressions vs uncached path
- [ ] Benchmark shows measurable improvement in command-encoding overhead

## Technical Tasks

1. Track input buffer identity (e.g. `wgpu::Id` or pointer) in
   `PooledComputeInstanceFilter`
2. Cache `BindGroup` alongside buffer references
3. Invalidate cache on `grow()` or input buffer change
4. Add tests verifying cache hit/miss behavior
5. Benchmark bind group caching at 100K and 1M scales

## Dependencies

- GUP-183: Pooled GPU Instance Filter Buffers

## Testing Strategy

- Unit tests for cache invalidation on grow and input buffer change
- GPU integration tests verifying correctness with cached bind groups
- Micro-benchmarks measuring bind group creation overhead

## Success Metrics

- Zero bind group allocations per frame when input buffer is stable
- Measurable reduction in per-dispatch CPU overhead

## Risk Assessment

- **Risk**: Stale bind group if buffer is replaced externally
  - **Mitigation**: Always compare buffer identity before reuse

## Definition of Done

- [ ] Implementation compiles and all tests pass
- [ ] Benchmark shows improvement
- [ ] Documentation updated
