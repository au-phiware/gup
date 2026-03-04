# GUP-277: GPU Render Loop Transition Integration

## Story Overview

**Initiative**: Selection API  
**Status**: ✅ Complete  
**Created**: 2025-07-25  
**Completed**: 2025-07-26

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

- [x] When `CommittedTransition` is active, `prepare_render_bound()` generates
      interpolated instance data between from/to values based on elapsed time.
- [x] The elapsed time is tracked via a `Selection::tick_transition(dt_ms: f64)`
      method that advances the transition clock.
- [x] At `t >= duration + delay`, the transition auto-completes (calls
      `complete_transition()`).
- [x] `KeyframeAnimation` instances from GUP-138 are used for the interpolation.
- [x] No GPU validation errors during animated rendering.

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md

## Implementation Summary

### What Was Implemented

1. **`elapsed_ms` field on `CommittedTransition`** — Tracks accumulated time
   since transition start. Initialised to 0.0 on commit.

2. **`EasingFn::apply(t: f32) -> f32`** — CPU-side easing curve evaluation
   matching the GPU-side behaviour. Supports Linear, EaseIn (quadratic),
   EaseOut, EaseInOut (cubic), CubicBezier, CatmullRom, and BSpline.

3. **`KeyframeAnimation::evaluate(time: f32) -> f32`** — CPU-side keyframe
   interpolation that mirrors the GPU `keyframe_animation` WGSL function.
   Supports empty/single/multi-keyframe animations with boundary clamping.

4. **`AttrValue::lerp()` and `AttrValue::as_f32_first()`** — Component-wise
   linear interpolation for Float, Vec2, Vec4 attribute values, plus a
   helper to extract the first f32 component.

5. **`Selection::tick_transition(dt_ms: f64) -> bool`** — Advances the
   transition clock. Auto-completes via `complete_transition()` when
   elapsed time reaches `delay + duration`. Returns whether the transition
   is still active.

6. **`prepare_render_bound()` transition integration** — When a
   `CommittedTransition` is active, the method delegates to
   `build_transition_instances()` which creates 2-keyframe
   `KeyframeAnimation` instances per attribute, applies easing, and
   interpolates between from/to `AttrValue`s.

### Key Files Changed

| File | Changes |
|------|---------|
| `src/transition/builder.rs` | Added `elapsed_ms` field, `EasingFn::apply()`, 6 tests |
| `src/shader_function.rs` | Added `KeyframeAnimation::evaluate()`, 5 tests |
| `src/selection.rs` | Added `tick_transition()`, `build_transition_instances()`, `AttrValue::lerp()`, `AttrValue::as_f32_first()`, modified `prepare_render_bound()`, 11 tests |

### Test Counts

- **22 new tests**: 6 easing, 5 keyframe evaluation, 5 AttrValue, 4 tick_transition, 2 integration
- **2766 total library tests passing** (up from 2744)
