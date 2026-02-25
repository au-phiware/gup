# GUP-197: Result Buffer Readback Optimization

**Status**: 🚧 In Progress **Priority**: Low **Effort**: 5 **Dependencies**:
GUP-194 (GPU-Resident Selection Data Cache)

## Overview

The `download_results()` method in `InteractionSystem` creates a new staging
buffer on every query to read back GPU hit test results. For cached queries
(GUP-194) this per-query buffer allocation and mapping is now the dominant
latency source (~3-4ms per query for 100K marks). A persistent mapped staging
buffer would eliminate this overhead, potentially reducing cached query latency
to sub-millisecond.

## Context

GUP-194 eliminated the per-query element data upload overhead by caching GPU
element data. However, the `download_results()` method still creates a fresh
staging buffer, copies results to it, and maps it for reading on every query.
The GPU compute dispatch itself is fast (<0.5ms), but the round-trip through
buffer creation + copy + map + unmap takes 3-4ms.

## User Story

As a developer building real-time interactive visualizations, I want hit test
result readback to be as fast as possible so that cached queries achieve true
sub-millisecond latency.

## Acceptance Criteria

1. Staging buffer for result readback is created once and reused
2. Buffer mapping uses persistent or double-buffered strategy
3. Cached query latency stays under 1ms for 100K marks in release mode
4. No correctness regression: all existing hit test results remain identical

## Technical Tasks

- [ ] Create persistent staging buffer in `InteractionSystem::new()`
- [ ] Implement double-buffered readback (map buffer N while writing to N+1)
- [ ] Benchmark latency improvement vs GUP-194 baseline
- [ ] Consider using `wgpu::MaintainBase::Poll` for non-blocking readback

## Testing Strategy

- Benchmark: compare readback latency with persistent vs per-query staging
- Integration tests: verify result correctness with persistent buffer
- Stress test: rapid successive queries to validate double-buffering

## Risk Assessment

- **Low**: Straightforward buffer management optimization
- **Medium**: Double-buffered readback adds complexity and must handle edge
  cases (e.g., buffer resize when max_results changes)

## Definition of Done

- [ ] Persistent staging buffer replaces per-query allocation
- [ ] <1ms average latency for 100K cached queries in release mode
- [ ] All existing interaction tests pass
- [ ] No increase in GPU memory usage beyond the staging buffer itself
