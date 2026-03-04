# GUP-277: GPU Render Loop Transition Integration

## Story Overview

**Initiative**: Selection API  
**Status**: 🚧 In Progress  
**Created**: 2025-07-25

## Context

GUP-276 implemented the data transition API: `data_keyed()` for key-based
diffing, `TransitionBuilder` for configuring transitions, and
`CommittedTransition` snapshots that store per-element from/to attribute values.
However, the transition data is currently a CPU-side snapshot — it is not yet
wired into the GPU render loop to create actual `KeyframeAnimation` instances
that interpolate attribute values each frame.

This story bridges the gap by implementing the render-side integration: when a
`CommittedTransition` is active on a Selection, the render loop should create
2-keyframe animations from the from/to snapshots, advance them based on elapsed
time, and interpolate attribute values in the vertex shader.

## User Story

> "As a visualization developer, I want committed transitions to automatically
> animate attribute values on the GPU so that I see smooth visual interpolation
> without writing manual animation loop code."

## Acceptance Criteria

- [ ] When `CommittedTransition` is active, `prepare_render_bound()` generates
      interpolated instance data between from/to values based on elapsed time.
- [ ] The elapsed time is tracked via a `Selection::tick_transition(dt_ms: f64)`
      method that advances the transition clock.
- [ ] At `t >= duration + delay`, the transition auto-completes (calls
      `complete_transition()`).
- [ ] `KeyframeAnimation` instances from GUP-138 are used for the interpolation.
- [ ] No GPU validation errors during animated rendering.

## Dependencies

### Prerequisite Stories

- GUP-276: D3-Style Data Transitions ✅ — provides `CommittedTransition`,
  `TransitionBuilder`, `EasingFn`, and the enter/update/exit data model.
- GUP-138: Advanced Temporal Animation ✅ — provides `KeyframeAnimation`.
- GUP-168: Selection Attribute Binding Pipeline ✅ — provides `attr()` and
  `prepare_render_bound()`.

## Testing Strategy

- Unit tests for `tick_transition` clock advancement.
- Integration test: tick through a 500ms transition and verify interpolated
  values at 0%, 50%, and 100%.
- Visual test with a windowed example showing smooth animation.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
