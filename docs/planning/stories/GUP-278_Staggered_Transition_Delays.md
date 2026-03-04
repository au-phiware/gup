# GUP-278: Staggered Transition Delays

## Story Overview

**Initiative**: Selection API  
**Status**: ✅ Complete  
**Completed**: 2025-07-26
**Created**: 2025-07-25

## Context

GUP-276 implemented the `TransitionBuilder` with a single `.delay(ms)` that
applies uniformly to all elements. In many data visualizations, a more visually
appealing effect is to stagger the delay per element — for example, having each
bar in a bar chart animate in sequence with a slight offset, or having scatter
plot points cascade from left to right.

This story adds a `.delay_fn(|index, data| -> u64)` method to
`TransitionBuilder` that computes a per-element delay, enabling cascading and
staggered animation effects without touching the core diff or animation
infrastructure.

## User Story

> "As a visualization developer, I want to specify per-element delays in
> transitions (e.g., `delay_fn(|i, _d| i as u64 * 50)`) so that elements animate
> in a staggered sequence rather than all at once."

## Acceptance Criteria

- [x] `TransitionBuilder::delay_fn(f: impl Fn(usize, &T) -> u64)` accepts a
      closure that receives the element index and data item, returning a
      per-element delay in milliseconds.
- [x] Per-element delays are stored in `ElementTransition` alongside from/to
      values.
- [x] When both `.delay()` and `.delay_fn()` are specified, the global delay is
      added to each per-element delay.
- [x] The scatter plot example is extended to show staggered entry.
- [x] Unit test verifies per-element delays are correctly computed.

## Dependencies

### Prerequisite Stories

- GUP-276: D3-Style Data Transitions ✅ — provides `TransitionBuilder`,
  `ElementTransition`, and the commit pipeline.
- GUP-277: GPU Render Loop Transition Integration 📋 — needed for visual stagger
  effect (optional, API can be designed without it).

## Testing Strategy

- Unit test: `.delay_fn(|i, _| i as u64 * 100)` on 5 elements produces delays
  [0, 100, 200, 300, 400].
- Integration test: total transition time is `max(delays) + duration`.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md

## Implementation Summary

### What Was Implemented

- **`DelayFn<T>` type** — Type-erased closure `(usize, &T) -> u64` for
  computing per-element delays, with `Send + Sync` on native targets.
- **`ElementTransition.delay_ms`** — Optional `u64` field storing the
  per-element delay computed by the delay function.
- **`TransitionBuilder::delay_fn()`** — Fluent API method accepting a closure
  that receives `(index, &data_item)` and returns a per-element delay in
  milliseconds.
- **`CommittedTransition` helper methods**:
  - `effective_delay(el)` — global delay + per-element delay
  - `max_effective_delay()` — maximum effective delay across all elements
  - `total_ms()` — max effective delay + duration (the true transition end time)
- **Per-element interpolation** — `build_transition_instances` now computes
  `eased_t` per element using its effective delay, enabling stagger effects.
- **Staggered completion** — `tick_transition` uses `total_ms()` so the
  transition doesn't complete until the last-delayed element finishes.

### Key Files Changed

| File | Change |
|------|--------|
| `src/transition/builder.rs` | `DelayFn`, `delay_fn()`, `delay_ms` field, `CommittedTransition` helpers, 8 new tests |
| `src/selection.rs` | Per-element `eased_t` in `build_transition_instances`, staggered `tick_transition`, 3 new tests |
| `examples/data_transition_scatter.rs` | Staggered entry demo section |

### Test Counts

- **11 new tests** (8 in builder, 3 in selection)
- **2777 total lib tests** all passing
