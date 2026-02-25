# GUP-198: Non-Blocking Query API

**Status**: 📋 Planned **Priority**: Low **Effort**: 8 **Dependencies**: GUP-197
(Result Buffer Readback Optimization)

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

1. A `query_point_async` (or similar) method returns a handle/future
2. Results can be polled or awaited without blocking the device
3. Double-buffered staging enables continuous query streams
4. Existing synchronous API continues to work unchanged
5. Perceived latency for frame-aligned queries is <1ms

## Technical Tasks

- [ ] Design `QueryHandle` type that wraps a pending GPU readback
- [ ] Implement double-buffered staging (two `result_staging_buffer` instances)
- [ ] Add `poll_result()` and `await_result()` methods on `QueryHandle`
- [ ] Integrate with wgpu's `MaintainBase::Poll` for non-blocking device polling
- [ ] Add frame-aligned query example showing CPU-GPU overlap

## Testing Strategy

- Integration test: submit query, do CPU work, then consume result
- Latency benchmark: compare perceived latency with synchronous API
- Stress test: continuous query stream with double-buffered readback

## Risk Assessment

- **Medium**: API design must be ergonomic without exposing GPU lifetime details
- **Low**: Double-buffering is well-understood; wgpu supports the needed
  primitives

## Definition of Done

- [ ] Non-blocking query API with `QueryHandle`
- [ ] Double-buffered staging buffers
- [ ] Perceived latency <1ms for frame-aligned queries
- [ ] Synchronous API backward compatibility
- [ ] Integration tests and benchmarks
