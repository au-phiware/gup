# GUP-276: D3-Style Data Transitions

## Story Overview

**Initiative**: Selection API  
**Status**: 🚧 In Progress  
**Created**: 2025-07-24

## Context

D3.js's enter/update/exit transition pattern is one of the most influential
ideas in data visualization. When data is rebound to a selection — say, a new
dataset arrives, or the user filters — elements animate smoothly from their old
state to their new state. New elements _enter_ (typically fading or growing in),
removed elements _exit_ (fading or shrinking out), and unchanged or updated
elements _transition_ in place, their attribute values interpolating from old to
new. This triple-group diffing pattern lets the viewer track what changed rather
than perceiving a jarring hard cut.

GUP-002 established the core `Selection<T, M>` type with data binding. GUP-138
added a keyframe-based `AnimationTimeline` and `KeyframeAnimation` that run
entirely on the GPU — a 2-keyframe animation is already capable of encoding a
"from → to" linear interpolation. GUP-141 added Catmull-Rom and B-spline modes
for smoother curves, and GUP-142 added event callbacks that fire at timeline
markers (enabling "on transition end" notifications). GUP-168 implemented the
`attr(name, closure)` binding pipeline so that data attributes can be declared
by name rather than by manually building instance buffers.

What does not yet exist is the _rebind-and-animate_ pattern: when
`selection.data(new_data, key_fn)` is called, Gup does not currently diff the
old and new datasets by stable key, does not capture the current rendered values
as "from" state, and does not schedule a GPU animation to reach the new "to"
state. This story implements that missing pattern, exposing a `transition()`
method on `Selection` that returns a `TransitionBuilder` — deliberately echoing
D3's own `.transition()` API to lower the migration barrier for D3 users.

## User Story

> "As a visualization developer, I want to call
> `selection.data(new_data, key_fn).transition().duration(500).ease(EasingFn::CubicInOut).attr(name, value_fn)`
> so that when my dataset changes, existing elements animate smoothly to their
> new attribute values, new elements fade in, and removed elements fade out —
> all driven by GPU interpolation with no per-frame CPU work."

## Acceptance Criteria

### AC1: Key-Function Data Rebinding

- [ ] `Selection::data_keyed(new_data, key_fn)` (or an overload of `data()`)
      accepts a key closure `|item: &T| -> K where K: Eq + Hash` and returns a
      `BoundSelection` that partitions elements into three groups: _enter_ (key
      present in new data but not old), _update_ (key present in both), and
      _exit_ (key present in old data but not new).
- [ ] The existing `Selection::data()` without a key function continues to work
      and performs positional (index-based) matching, as before.
- [ ] The key function is evaluated on the CPU at rebind time; no GPU readback
      is required to compute the diff.
- [ ] Unit test: given old data `[A, B, C]` and new data `[B, C, D]` with
      identity-key function, the enter set is `{D}`, the update set is `{B, C}`,
      and the exit set is `{A}`.

### AC2: TransitionBuilder API

- [ ] `Selection::transition()` returns a `TransitionBuilder` associated with
      that selection.
- [ ] `TransitionBuilder` exposes chainable methods:
  - `.duration(ms: u64)` — total interpolation time in milliseconds.
  - `.delay(ms: u64)` — time before the transition begins.
  - `.ease(fn: EasingFn)` — easing function applied to normalised `t ∈ [0,1]`;
    accepts at minimum the variants already defined in GUP-138 (`Linear`,
    `CubicBezier`, `EaseIn`, `EaseOut`, `EaseInOut`).
  - `.attr(name: &str, value_fn: impl Fn(&T) -> impl IntoAttrValue)` — declares
    the target ("to") value for a named attribute; mirrors `Selection::attr()`.
- [ ] Calling `.commit()` (or equivalent) on a fully-configured
      `TransitionBuilder` captures the current GPU-resident attribute values as
      "from" state and schedules the animation.
- [ ] A transition with no `.attr()` calls is a no-op and emits a
      `tracing::warn!` rather than panicking.

### AC3: GPU Interpolation for Update Elements

- [ ] For each element in the _update_ group, a 2-keyframe GPU animation
      (leveraging the `KeyframeAnimation` infrastructure from GUP-138) is
      created with keyframe 0 = current ("from") attribute values and keyframe 1
      = target ("to") attribute values.
- [ ] The easing function and duration specified on the `TransitionBuilder` are
      applied to the animation timeline.
- [ ] Spline interpolation modes from GUP-141 are accessible when
      `.ease(EasingFn::CatmullRom { tension })` or `.ease(EasingFn::BSpline)` is
      specified (or an equivalent API that selects the spline path through the
      `InterpolationMode` enum).
- [ ] After the transition completes, the animation is cleaned up: the element
      reverts to static rendering at the final "to" value so that no live
      timeline resources persist for completed transitions.
- [ ] Integration test: after a transition of 500 ms has elapsed (by advancing
      the animation clock), the rendered attribute value equals the "to" value
      within floating-point tolerance.

### AC4: Enter and Exit Animations

- [ ] _Enter_ elements begin at an "enter" attribute state (by default:
      `opacity = 0.0`, or whatever the mark-appropriate "invisible" initial
      value is) and animate to the "to" values supplied in the transition.
- [ ] _Exit_ elements animate from their current state toward an "exit"
      attribute state (by default: `opacity = 0.0`) and are removed from the
      selection's data and GPU buffers once the transition completes.
- [ ] The default enter/exit behaviour can be overridden per attribute by
      calling `.enter_attr(name, initial_fn)` and `.exit_attr(name, final_fn)`
      on the `TransitionBuilder`.
- [ ] Integration test: after a transition completes, exit elements are no
      longer present in the selection's data buffer (verified by querying
      element count).

### AC5: Transition Event Callbacks

- [ ] `TransitionBuilder::on_start(callback)` fires once when the transition
      begins (after any delay has elapsed).
- [ ] `TransitionBuilder::on_end(callback)` fires once when all three groups
      (enter, update, exit) have finished their animations.
- [ ] Callbacks are routed through the `AnimationTimelineWithEvents`
      infrastructure from GUP-142; no new event machinery is introduced.
- [ ] Test: `on_end` fires exactly once per transition call, even if the same
      selection undergoes multiple sequential transitions.

### AC6: Animated Scatter Plot Example

- [ ] A working example `examples/data_transition_scatter.rs` demonstrates:
  - An initial dataset of 20 points rendered as `Circle` marks.
  - A function that rebinds a new dataset of 20 points (with 10 shared keys, 5
    new, 5 removed).
  - `selection.data_keyed(new_data, |p| p.id).transition().duration(800).ease(EasingFn::EaseInOut).attr("cx", |p| p.x).attr("cy", |p| p.y)`.
  - The example compiles and renders without GPU validation errors.
- [ ] The example includes inline comments explaining the enter/update/exit
      groups.

## Technical Tasks

- [ ] **Diff engine**: implement
      `fn diff_by_key<T, K>(old: &[T], new: &[T], key: impl Fn(&T) -> K) -> DiffResult<T>`
      in a new `src/transition/diff.rs` module, returning typed `enter`,
      `update` (with `(old_item, new_item)` pairs), and `exit` slices.
- [ ] **TransitionBuilder struct**: add `src/transition/builder.rs` with
      `TransitionBuilder<T, M>` fields for duration, delay, easing, `attr`
      closures, `enter_attr` overrides, `exit_attr` overrides, and callbacks.
- [ ] **Selection::transition()**: add method to `Selection<T, M>` returning
      `TransitionBuilder<T, M>` (requires the selection to already have an
      active `DiffResult` from a keyed data call, or operates on the whole
      selection as the update group when called without prior diffing).
- [ ] **Selection::data_keyed()**: add
      `data_keyed(new_data: Vec<T>, key_fn: impl Fn(&T) -> K)` to `Selection`
      that stores the diff result and updates internal data without immediately
      rebuilding GPU buffers (the transition drive the buffer update).
- [ ] **From-state snapshot**: on `TransitionBuilder::commit()`, iterate over
      update and exit elements, read their current CPU-side attribute values
      (from the stored attr bindings), and record them as keyframe 0.
- [ ] **2-keyframe animation setup**: for each element group (enter, update,
      exit), create a `KeyframeAnimation` with 2 keyframes using the from/to
      values; attach to per-element timeline.
- [ ] **Per-element timeline management**: design a lightweight per-instance
      timeline store inside `Selection` (a `Vec<Option<AnimationTimeline>>`
      parallel to the data vec) so each element can have an independent
      in-flight transition.
- [ ] **Cleanup on completion**: use `AnimationTimelineWithEvents::on_complete`
      (GUP-142) to trigger removal of exit elements from the data vec and GPU
      buffer, and to drop the per-element timeline.
- [ ] **EasingFn extension**: if `EasingFn` does not already expose `CatmullRom`
      / `BSpline` variants, extend the enum (or provide a newtype wrapper) to
      map to `InterpolationMode` from GUP-141.
- [ ] **Example**: write `examples/data_transition_scatter.rs` per AC6.
- [ ] **Unit tests** for diff engine (AC1), TransitionBuilder construction
      (AC2), from-state snapshot correctness (AC3), exit removal (AC4), and
      callback firing (AC5).

## Dependencies

### Prerequisite Stories

- GUP-002: Core Selection Type ✅ — provides `Selection<T, M>`, data binding,
  and the GPU instance buffer architecture that transitions mutate.
- GUP-138: Advanced Temporal Animation ✅ — provides `KeyframeAnimation`,
  `AnimationTimeline`, easing functions, and the WGSL interpolation shader that
  will drive per-element from→to animation.
- GUP-141: Spline Animation Curves ✅ — provides `InterpolationMode::CatmullRom`
  and `BSpline`, available as transition easing options.
- GUP-142: Animation Event System ✅ — provides `AnimationTimelineWithEvents`
  and marker-based callbacks used to implement `on_start` / `on_end` and to
  trigger exit-element cleanup.
- GUP-168: Selection Attribute Binding Pipeline ✅ — provides
  `attr(name, closure)` and `IntoAttrValue`, which `TransitionBuilder::attr()`
  mirrors and reuses for computing "to" values.

### Enables Stories

- Any story building higher-level chart types (scatter plots, bar charts, line
  charts) that require animated data updates will naturally build on this
  transition API.
- A future "Staggered Transition" story (sequential delay per element) can add
  `.delay_fn(|i, _d| i as u64 * 50)` to `TransitionBuilder` without touching the
  core diff or animation machinery.

## Testing Strategy

- **Unit tests** (`src/transition/diff.rs`): exhaustively test the diff engine
  with empty old, empty new, all-enter, all-exit, all-update, and mixed cases
  with duplicate keys.
- **Unit tests** (`src/transition/builder.rs`): verify that `TransitionBuilder`
  correctly records duration, delay, easing, and attr closures; verify that
  `commit()` with no attr bindings emits a warning and is a no-op.
- **Integration tests** (`tests/transition_integration.rs`): advance a mock
  animation clock by the full transition duration and assert that (a) update
  element attributes equal "to" values, (b) exit elements are absent from the
  data buffer, (c) `on_end` fires exactly once.
- **Visual validation**: run `examples/data_transition_scatter.rs` and inspect
  that circles move smoothly from old positions to new positions with no
  flickering or GPU validation layer errors.
- **Performance**: a transition of 10,000 elements should not stall the CPU for
  more than ~1 ms at `commit()` time (diff + snapshot); the per-frame GPU work
  is bounded by the existing animation shader.

## Success Metrics

- [ ] The diff engine correctly categorises enter/update/exit for all edge cases
      (empty datasets, all-new data, all-removed data).
- [ ] An 800 ms update-group transition on 1,000 `Circle` marks produces
      rendered positions within `1e-4` of the target values after the timeline
      reaches 1.0, with no GPU validation errors.
- [ ] Exit elements are fully removed from GPU buffers within one frame after
      their transition duration elapses.
- [ ] `on_end` fires exactly once per `commit()` call in the integration test
      suite.
- [ ] The scatter plot example compiles (`cargo check --examples`) and renders
      without errors.

## Risk Assessment

- **Medium**: The "from-state snapshot" requires reading current attribute
  values for each transitioning element at `commit()` time. If those values are
  only stored on the GPU (e.g., after GUP-177 GPU-side shader binding), a
  CPU-side readback would be needed, which is expensive. _Mitigation_: for this
  story, rely on the CPU-side attr closure values stored by GUP-168; note in the
  Context section that GPU-side binding (GUP-177) will require a follow-up to
  source from-state values from a GPU readback or a CPU mirror buffer.

- **Medium**: Per-element timelines introduce a `Vec<Option<AnimationTimeline>>`
  parallel to the data buffer. If elements are frequently added and removed,
  this structure can fragment. _Mitigation_: use a simple parallel `Vec` for
  this story; a more efficient slot-map approach can be addressed in a follow-up
  if profiling reveals fragmentation costs.

- **Low**: The `AnimationTimeline` from GUP-138 was designed for whole-selection
  animations, not per-element timelines. It may not be cheap to instantiate
  thousands of independent timelines. _Mitigation_: benchmark during
  implementation; if instantiation overhead is significant, consider a shared
  timeline with per-element phase offsets instead.

- **Low**: The `EasingFn` enum in GUP-138 may not map cleanly onto
  `InterpolationMode` from GUP-141 (one is for timing, the other is for spatial
  interpolation). _Mitigation_: treat them as orthogonal: `ease()` controls the
  time curve; `interpolation_mode()` (if added to `TransitionBuilder`) controls
  the path shape between keyframes. The story can expose both axes separately
  with no naming conflict.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
