# GUP-196: Hit Test Result Buffer Query Count

**Priority**: Medium **Complexity**: Low **Created**: 2025-08-08 **Status**: ✅
Complete (2025-08-09)

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

- [x] Hit test shader receives actual query count via uniform (not
      `arrayLength(&queries)`)
- [x] Result indexing uses `element_index * query_count + query_index`
- [x] Single-query dispatch supports up to 100K candidates
- [x] All existing interaction tests pass
- [x] Multi-query batch dispatch continues to work correctly

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

- [x] Hit test shader uses uniform query count
- [x] All existing interaction and spatial index tests pass
- [x] Test demonstrating >3125 candidates with single query
- [x] `mask all-fix` passes

## Implementation Summary

### What Was Implemented

1. **WGSL Shader Changes** (`src/shaders/hit_test.compute.wgsl`):
   - Added `HitTestConfig` struct with `query_count: u32` field
   - Added `@group(0) @binding(3) var<uniform> config: HitTestConfig;`
   - Changed result indexing from `element_index * arrayLength(&queries)` to
     `element_index * config.query_count`
   - Updated bounds checking and shared memory caching to use
     `config.query_count`

2. **Rust Changes** (`src/interaction.rs`):
   - Added `HitTestConfig` struct (`#[repr(C)]`, `Pod`, `Zeroable`) with
     `query_count: u32` and 3×u32 padding for 16-byte alignment
   - Added `hit_test_config_buffer` (uniform, COPY_DST) to `InteractionSystem`
   - Updated `dispatch_hit_test_compute` to upload config before dispatching
   - Updated `dispatch_gpu_morton_query` to upload config (query_count=1) before
     the indirect hit test pass
   - Updated `create_compute_bind_group` and
     `create_gathered_hit_test_bind_group` to include binding 3

3. **Tests** (`tests/gpu_resident_pipeline_tests.rs`):
   - `test_single_query_exceeds_old_candidate_limit`: Verifies 5000 candidates
     with 1 query all produce results (exceeding old 3125 limit)
   - `test_batch_query_still_works`: Verifies multi-query batch dispatch still
     works correctly with the uniform query count

### Key Files Changed

| File                                   | Change                                        |
| -------------------------------------- | --------------------------------------------- |
| `src/shaders/hit_test.compute.wgsl`    | HitTestConfig struct + uniform-based indexing |
| `src/interaction.rs`                   | Config buffer, upload, bind group changes     |
| `tests/gpu_resident_pipeline_tests.rs` | 2 new tests, removed outdated NOTE            |

### Test Results

- **2 new tests** added
- **51+ existing** interaction/spatial index tests continue to pass
- **1214** library tests pass (2 pre-existing flaky perf tests excluded)
- All examples compile
