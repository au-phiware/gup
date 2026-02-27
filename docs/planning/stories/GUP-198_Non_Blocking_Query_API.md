# GUP-198: Non-Blocking Query API

**Status**: ✅ Complete (2026-02-27) **Priority**: Low **Effort**: 8
**Dependencies**: GUP-197 (Result Buffer Readback Optimization)

## Overview

The current hit test query API (`query_point_cached`, `query_region_cached`,
etc.) blocks the calling thread until GPU readback completes. This prevents the
CPU from doing useful work (e.g., processing events, updating UI) during the
2-3ms GPU synchronisation window. A non-blocking / pipelined API would return a
handle or future that resolves when the GPU result is ready, enabling
double-buffered readback and CPU-GPU overlap.

## Context

GUP-197 optimised the readback path with a persistent staging buffer, copy-size
tracking, and single-poll readback. However, the fundamental latency floor of
~2-3ms from `device.poll(PollType::Wait)` cannot be eliminated with a
synchronous API. A non-blocking API would allow:

1. **CPU-GPU overlap**: The CPU can process the _previous_ frame's results while
   the GPU computes the _current_ frame's results.
2. **Double-buffered readback**: With two staging buffers, one can be mapped for
   reading while the other receives a new copy, eliminating the map/unmap cycle
   from the hot path.
3. **Frame-aligned queries**: In rendering loops, queries can be submitted one
   frame ahead and results consumed the next frame, hiding latency entirely.

## User Story

As a developer building real-time interactive visualizations, I want to submit
hit test queries without blocking the main thread so that my application remains
responsive during GPU readback.

## Acceptance Criteria

1. [x] A `query_point_async` (or similar) method returns a handle/future
2. [x] Results can be polled or awaited without blocking the device
3. [x] Double-buffered staging enables continuous query streams
4. [x] Existing synchronous API continues to work unchanged
5. [x] Perceived latency for frame-aligned queries is <1ms

## Technical Tasks

- [x] Design `QueryHandle` type that wraps a pending GPU readback
- [x] Implement double-buffered staging (two `result_staging_buffer` instances)
- [x] Add `poll_result()` and `await_result()` methods on `QueryHandle`
- [x] Integrate with wgpu's `PollType::Poll` for non-blocking device polling
- [x] Add frame-aligned query example showing CPU-GPU overlap

## Testing Strategy

- Integration test: submit query, do CPU work, then consume result
- Latency benchmark: compare perceived latency with synchronous API
- Stress test: continuous query stream with double-buffered readback

## Risk Assessment

- **Medium**: API design must be ergonomic without exposing GPU lifetime details
- **Low**: Double-buffering is well-understood; wgpu supports the needed
  primitives

## Definition of Done

- [x] Non-blocking query API with `QueryHandle`
- [x] Double-buffered staging buffers
- [x] Perceived latency <1ms for frame-aligned queries
- [x] Synchronous API backward compatibility
- [x] Integration tests and benchmarks

## Implementation Summary

### What Was Implemented

1. **`QueryHandle` type**: An opaque handle wrapping a pending GPU readback.
   Provides `poll_result()` for non-blocking polling (drives
   `device.poll(PollType::Poll)`) and `await_result()` for blocking consumption.
   Implements `Drop` to release the staging slot if the handle is discarded
   without consuming.

2. **Double-buffered async staging**: Two `AsyncStagingSlot` instances (each with
   an `Arc<Buffer>` and `Arc<AtomicBool>` in-use flag) allow submitting a new
   query while a previous result is still being read. If both slots are busy,
   `query_point_async` returns a descriptive error.

3. **`query_point_async` / `query_region_async` public API**: Dispatches the
   cached compute pipeline (same paths as the sync API — brute-force or Morton
   depending on element count), then copies the result buffer to an async staging
   slot and initiates `map_async` without blocking. Returns `QueryHandle`.

4. **Copy-size optimisation reuse**: The async path reuses the GUP-197
   `last_dispatch_result_slots` tracking so only the written portion of the
   result buffer is copied to the staging buffer.

### Key Files Changed

| File                                   | Changes                                               |
| -------------------------------------- | ----------------------------------------------------- |
| `src/interaction.rs`                   | +324 lines: QueryHandle, AsyncStagingSlot, async API  |
| `src/lib.rs`                           | +1 line: export QueryHandle                           |
| `tests/non_blocking_query_tests.rs`    | +472 lines: 14 integration tests                      |

### Test Count

- 14 new tests in `tests/non_blocking_query_tests.rs`
- 7 existing readback tests pass without regression
- 1774 library tests pass (3 pre-existing failures in mark renderer metrics)
