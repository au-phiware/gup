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

2. **Double-buffered async staging**: Two `AsyncStagingSlot` instances (each
   with an `Arc<Buffer>` and `Arc<AtomicBool>` in-use flag) allow submitting a
   new query while a previous result is still being read. If both slots are
   busy, `query_point_async` returns a descriptive error.

3. **`query_point_async` / `query_region_async` public API**: Dispatches the
   cached compute pipeline (same paths as the sync API — brute-force or Morton
   depending on element count), then copies the result buffer to an async
   staging slot and initiates `map_async` without blocking. Returns
   `QueryHandle`.

4. **Copy-size optimisation reuse**: The async path reuses the GUP-197
   `last_dispatch_result_slots` tracking so only the written portion of the
   result buffer is copied to the staging buffer.

### Key Files Changed

| File                                | Changes                                              |
| ----------------------------------- | ---------------------------------------------------- |
| `src/interaction.rs`                | +324 lines: QueryHandle, AsyncStagingSlot, async API |
| `src/lib.rs`                        | +1 line: export QueryHandle                          |
| `tests/non_blocking_query_tests.rs` | +472 lines: 14 integration tests                     |

### Test Count

- 14 new tests in `tests/non_blocking_query_tests.rs`
- 7 existing readback tests pass without regression
- 1774 library tests pass (3 pre-existing failures in mark renderer metrics)

## Retrospective

**Completed**: 2026-02-27

### Key Technical Learnings

#### wgpu `map_async` Callback Model

- **Challenge**: wgpu's `map_async` takes a callback that only receives
  `Result<(), BufferAsyncError>` — it does not give access to the mapped data.
  The actual buffer read must happen after the callback fires, outside the
  callback.
- **Solution**: Use `futures_channel::oneshot` to bridge the callback into an
  async-compatible signal. The callback sends the completion status, and the
  `QueryHandle` methods (`poll_result` / `await_result`) check the channel
  before reading the buffer.
- **Pattern**: For wgpu async readback, the reliable sequence is:
  `copy_buffer_to_buffer` → `submit` → `map_async(callback → channel)` →
  `device.poll()` → `channel.try_recv()` → `get_mapped_range()` → `unmap()`.

#### Arc<Buffer> for Shared Ownership Across Handle and System

- **Challenge**: `QueryHandle` needs access to the staging buffer to read
  results, but the buffer is owned by `InteractionSystem`. Buffer is not
  `Clone`.
- **Solution**: Wrap each async staging buffer in `Arc<Buffer>`. The system and
  the handle both hold `Arc` clones. The `AtomicBool` in-use flag coordinates
  slot availability without requiring mutual exclusion.
- **Pattern**: For GPU resources that need to be shared between a "producer"
  (the system that submits work) and a "consumer" (the handle that reads
  results), `Arc<Buffer>` + `Arc<AtomicBool>` is a lightweight, lock-free
  coordination strategy.

#### Separate Sync and Async Staging Buffers

- **Challenge**: The sync API (GUP-197) uses a persistent
  `result_staging_buffer` that is mapped/unmapped synchronously. Reusing it for
  async would create conflicts if sync and async queries are interleaved.
- **Solution**: Keep the existing sync staging buffer untouched and add two
  separate async staging buffers. This means zero changes to the sync path and
  zero risk of regression.
- **Pattern**: When adding a non-blocking variant of an existing blocking API,
  prefer separate resources over shared resources with mode flags. The small
  memory overhead (2 extra buffers × 3.2MB each for 100K max results) is
  negligible compared to the simplicity benefit.

### Architectural Decisions

#### Two Slots (Not N)

- **Decision**: Fixed double-buffering (2 slots) rather than a configurable
  pool.
- **Reasoning**: In a typical render loop, at most 1 query is in-flight at a
  time. Two slots handle the "submit frame N, consume frame N+1" pipeline
  perfectly. A pool would add complexity (slot selection, growth, shrink) with
  no practical benefit.
- **Trade-off**: If a user submits 3+ concurrent async queries, they get an
  error. This is by design — the API is optimised for the common pipeline case.
- **Future**: If a use case arises for more than 2 concurrent async queries, the
  `AsyncStagingSlot` array could be replaced with a `Vec` and a configurable
  limit.

#### QueryHandle Owns Results Processing

- **Decision**: `QueryHandle` reads and processes the mapped buffer itself (via
  `read_and_unmap`), rather than delegating back to `InteractionSystem`.
- **Reasoning**: If the handle delegated back to the system, the user would need
  `&mut InteractionSystem` to consume results, which defeats the purpose of
  decoupling submission from consumption. Self-contained handles enable patterns
  like storing the handle in a different struct and consuming it later.
- **Trade-off**: The handle duplicates the `InteractionResult` → `ElementHit`
  processing logic from `download_results`. However, the processing is simple
  (filter + map) and benefits from being colocated with the readback.

### Development Workflow Insights

- The implementation was straightforward because GUP-197 had already established
  the staging buffer and copy-size optimisation patterns. The async variant was
  essentially "factor out the blocking poll and let the caller decide when to
  consume." Building on solid sync foundations made the async extension clean.
- `futures_channel::oneshot` was already a dependency (used by the existing
  `download_results` method), so no new dependencies were needed.
- The test for `both_slots_busy_returns_error` validates an important safety
  property: the system never silently blocks or overwrites a pending result.
  This kind of negative-path test is easy to forget but crucial for correctness
  in concurrent GPU APIs.
- Frame-aligned latency tests use `tokio::time::sleep(16ms)` to simulate the
  inter-frame gap. This is realistic for 60 FPS render loops and reliably
  demonstrates that the GPU finishes well within one frame.

### Follow-up Stories

No new follow-up stories were identified. The interaction system's query
pipeline is now complete through the full sync → optimised readback → async
chain (GUP-194 → GUP-197 → GUP-198).
