# GUP-196: Hit Test Result Buffer Query Count

**Priority**: Medium **Complexity**: Low **Created**: 2025-08-08 **Status**: 📋
Planned

## Overview

Pass the actual query count (not the buffer capacity) to the hit test compute
shader via a uniform, so that result indexing uses
`element_index * actual_query_count + query_index` instead of
`element_index * arrayLength(&queries)`. This raises the effective candidate
limit from ~3,125 to 100,000 for single-query dispatches.

## Context

The hit test shader (`hit_test.compute.wgsl`) indexes results as
`element_index * arrayLength(&queries) + query_index`. Since the query buffer is
pre-allocated for `max_queries = 32`, `arrayLength(&queries)` always returns 32
regardless of how many queries were actually dispatched. With a result buffer of
100K entries, only `100,000 / 32 = 3,125` candidate elements can store results
before overflowing.

For single-query dispatches (point or region queries), result indices should use
`element_index * 1 + 0 = element_index`, supporting up to 100K candidates. This
limitation was discovered during GUP-193 (GPU-Resident Candidate Pipeline)
testing with 10K elements.

## User Story

As a developer querying million-point datasets, I want the hit test shader to
support the full candidate capacity so that single-query dispatches can test up
to 100K candidates without silently dropping results.

## Acceptance Criteria

- [ ] Hit test shader receives actual query count via uniform (not
      `arrayLength(&queries)`)
- [ ] Result indexing uses `element_index * query_count + query_index`
- [ ] Single-query dispatch supports up to 100K candidates
- [ ] All existing interaction tests pass
- [ ] Multi-query batch dispatch continues to work correctly

## Technical Tasks

1. Add a `HitTestConfig` uniform buffer with `query_count: u32` field
2. Update hit test shader to use `config.query_count` for result indexing
3. Update `dispatch_hit_test_compute` and `dispatch_gpu_morton_query` to upload
   the config uniform before dispatching
4. Update bind group layout and creation to include the new uniform binding
5. Verify batch query path still works

## Dependencies

- **Requires**: GUP-193 (GPU-Resident Candidate Pipeline)

## Testing Strategy

- Modify existing hit test tests to verify result indexing with 1 query
- Add test with >3125 candidates and 1 query to verify no silent drops
- Verify batch queries (>1 query) still produce correct results

## Risk Assessment

- **Low**: Simple shader change. Main risk is breaking existing bind group
  layouts. Careful migration of all bind group creation sites needed.

## Definition of Done

- [ ] Hit test shader uses uniform query count
- [ ] All existing interaction and spatial index tests pass
- [ ] Test demonstrating >3125 candidates with single query
- [ ] `mask all-fix` passes
