# GUP-194: GPU-Resident Selection Data Cache

**Status**: 🚧 In Progress **Priority**: Medium **Effort**: 5 **Dependencies**:
GUP-181 (GPU-Accelerated Selection Hit Testing)

## Overview

Pre-upload and cache mark positions and sizes on the GPU so that hit testing
queries avoid the per-query marshalling and upload overhead. Currently, each
call to `hit_test_gpu` extracts element data via the `Renderable` trait,
converts it to `ElementData`, and uploads it to the GPU. For 100K+ marks this
dominates query latency (~50ms in debug, ~5ms in release).

By keeping element data GPU-resident and only re-uploading when positions
change, hit testing can achieve true sub-millisecond latency.

## Context

GUP-181 integrated `MarkSelectionSystem` with `InteractionSystem` but each GPU
query re-creates the element buffer. The `InteractionSystem` already supports
spatial index caching (it rebuilds only when `>1000` elements and the index
isn't built), but element data itself is uploaded fresh every time.

## User Story

As a developer building real-time interactive visualizations with 100K+ marks, I
want the GPU hit testing data to be cached between queries so that hover
interactions remain responsive at sub-millisecond latency.

## Acceptance Criteria

1. Element data is uploaded to the GPU once and reused across queries
2. Dirty flag invalidates the cache when positions change
3. Spatial index is rebuilt only when the cache is invalidated
4. Hit test latency stays under 1ms for 100K marks in release mode
5. Memory usage stays within 2x of the current per-query approach

## Technical Tasks

- [ ] Add `upload_element_data_cached` method to `InteractionSystem`
- [ ] Track element data version/dirty flag
- [ ] Skip element extraction and upload when cache is valid
- [ ] Benchmark latency improvement vs GUP-181 baseline

## Testing Strategy

- Unit tests for cache invalidation logic
- Integration tests comparing cached vs uncached query results
- Performance benchmark: 100K marks, cached vs uncached latency

## Risk Assessment

- **Low**: Straightforward caching with dirty flag
- **Medium**: Memory pressure from keeping two copies (CPU + GPU) of element
  data for very large datasets

## Definition of Done

- [ ] Cached GPU element data works for point/rect/lasso queries
- [ ] <1ms latency for 100K marks in release mode (after initial upload)
- [ ] Cache invalidation works correctly when positions change
- [ ] All tests pass
