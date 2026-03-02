# GUP-283: Event Coalescing for High-Frequency Input

## Story Overview

**Initiative**: Interaction & Spatial Index
**Status**: 📋 Planned
**Created**: 2025-07-26

## Context

GUP-013 delivered the event handling system with `EventManager` dispatch, but
noted in its risk assessment that event coalescing for high-frequency
`mousemove` events is not in scope. Without coalescing, rapid mouse movement
can generate 60+ events per frame, each triggering full handler dispatch chains.
This can cause the event queue to grow unbounded and violate the 16 ms latency
target under sustained rapid input.

## User Story

> "As a visualization developer, I want high-frequency mouse movements to be
> coalesced into a single event per frame so that my hover handlers run
> efficiently without causing frame drops or input lag."

## Acceptance Criteria

- [ ] `EventManager` supports configurable coalescing that merges rapid
      `mousemove` and `touchmove` events into at most one dispatch per frame
- [ ] Coalesced events carry the latest position but retain the original
      timestamp of the first event in the coalescing window
- [ ] Non-coalesced events (`mousedown`, `mouseup`, `click`, `touchstart`,
      `touchend`) are always dispatched immediately
- [ ] A benchmark with 1000 simulated mouse-move events per frame shows dispatch
      count reduced to 1 per frame while maintaining < 16 ms total latency
- [ ] Coalescing can be disabled per event type for use cases requiring every
      event (e.g., drawing tools)

## Technical Tasks

- [ ] Add a `CoalescingConfig` struct to `EventManager` with per-event-type
      enable/disable
- [ ] Implement a frame-boundary coalescing buffer that accumulates events and
      flushes on `flush_frame()` or equivalent
- [ ] Add unit tests for coalescing behaviour
- [ ] Update the interactive example to demonstrate coalescing (optional)

## Dependencies

### Prerequisite Stories

- GUP-013: Event Handling System ✅ — provides EventManager that this extends

## Testing Strategy

- **Unit tests**: verify coalescing merges multiple moves into one dispatch;
  verify non-coalescable events pass through immediately
- **Benchmark**: 1000 simulated mouse-move events dispatched in < 1 ms total

## Risk Assessment

- **Low**: Coalescing is a well-understood technique. Main risk is choosing the
  right frame boundary signal (explicit `flush_frame()` call vs. timer-based).

## Definition of Done

- [ ] All Acceptance Criteria met
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint clean: `mask all-fix`
