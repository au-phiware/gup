# GUP-278: Staggered Transition Delays

## Story Overview

**Initiative**: Selection API  
**Status**: 🚧 In Progress  
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

- [ ] `TransitionBuilder::delay_fn(f: impl Fn(usize, &T) -> u64)` accepts a
      closure that receives the element index and data item, returning a
      per-element delay in milliseconds.
- [ ] Per-element delays are stored in `ElementTransition` alongside from/to
      values.
- [ ] When both `.delay()` and `.delay_fn()` are specified, the global delay is
      added to each per-element delay.
- [ ] The scatter plot example is extended to show staggered entry.
- [ ] Unit test verifies per-element delays are correctly computed.

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

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
