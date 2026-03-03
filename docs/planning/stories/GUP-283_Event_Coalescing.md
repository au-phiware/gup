# GUP-283: Event Coalescing for High-Frequency Input

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete **Created**:
2025-07-26 **Completed**: 2025-07-27

## Context

GUP-013 delivered the event handling system with `EventManager` dispatch, but
noted in its risk assessment that event coalescing for high-frequency
`mousemove` events is not in scope. Without coalescing, rapid mouse movement can
generate 60+ events per frame, each triggering full handler dispatch chains.
This can cause the event queue to grow unbounded and violate the 16 ms latency
target under sustained rapid input.

## User Story

> "As a visualization developer, I want high-frequency mouse movements to be
> coalesced into a single event per frame so that my hover handlers run
> efficiently without causing frame drops or input lag."

## Acceptance Criteria

- [x] `EventManager` supports configurable coalescing that merges rapid
      `mousemove` and `touchmove` events into at most one dispatch per frame
- [x] Coalesced events carry the latest position but retain the original
      timestamp of the first event in the coalescing window
- [x] Non-coalesced events (`mousedown`, `mouseup`, `click`, `touchstart`,
      `touchend`) are always dispatched immediately
- [x] A benchmark with 1000 simulated mouse-move events per frame shows dispatch
      count reduced to 1 per frame while maintaining < 16 ms total latency
- [x] Coalescing can be disabled per event type for use cases requiring every
      event (e.g., drawing tools)

## Technical Tasks

- [x] Add a `CoalescingConfig` struct to `EventManager` with per-event-type
      enable/disable
- [x] Implement a frame-boundary coalescing buffer that accumulates events and
      flushes on `flush_frame()` or equivalent
- [x] Add unit tests for coalescing behaviour
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

- [x] All Acceptance Criteria met
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint clean: `mask all-fix`

## Implementation Summary

### Modified Files

- **`src/event.rs`** — Added `CoalescingConfig`, `PendingCoalescedEvent`
  (internal), and extended `EventManager` with coalescing infrastructure:
  `with_coalescing()`, `coalescing_config()`, `set_coalescing_config()`,
  `submit()`, `flush_frame()`, `pending_count()`, `discard_pending()`. Also
  added `EventType::is_coalescable_default()`.
- **`src/lib.rs`** — Added `CoalescingConfig` to public re-exports.

### Key APIs

- **`CoalescingConfig`** — Per-event-type enable/disable. Default: `mousemove`
  and `touchmove` coalesced. Builder methods: `enable()`, `disable()`, `none()`.
- **`EventManager::submit(event, hits)`** — Queues coalescable events for
  frame-boundary dispatch; dispatches non-coalescable events immediately.
- **`EventManager::flush_frame()`** — Dispatches all pending coalesced events
  (one per type). Returns `Vec<(event_name, merge_count, EventResult)>`.
- **`EventManager::with_coalescing(config)`** — Constructor with custom config.

### Test Counts

- 15 new unit tests in `src/event.rs` (coalescing-specific)
- 1 benchmark test (1000 events → 1 dispatch, < 16ms)
- **Total new tests: 15** (all in `event::tests`)

### Design Decisions

- **Explicit `flush_frame()` over timer-based**: The caller controls when
  coalesced events flush, matching the typical game/visualization loop pattern
  where frame boundaries are explicit. No hidden timers or threads.
- **`submit()` + `flush_frame()` over modifying `dispatch()`**: Kept the
  existing `dispatch()` API unchanged for backward compatibility. New coalescing
  is opt-in through the `submit()` entry point.
- **Latest position, first timestamp**: Coalesced events preserve the first
  timestamp (for latency measurement) but use the latest position (for accurate
  cursor tracking). This matches browser `getCoalescedEvents()` semantics.
