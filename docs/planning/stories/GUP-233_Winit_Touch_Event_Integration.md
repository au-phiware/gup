# GUP-233: Winit Touch Event Integration

**Status**: ✅ Complete **Completed**: 2025-07-19 **Priority**: Low **Effort**:
2 **Dependencies**: GUP-182 (Touch Selection Support)

## Overview

Add a convenience `From<winit::event::Touch>` conversion for
`TouchEvent`/`TouchPhase` and update the `interactive_selection_demo` example to
handle `WindowEvent::Touch` events through the `TouchSelectionAdapter`,
providing an end-to-end touch selection workflow on desktop touch screens.

## Context

GUP-182 delivered the `TouchSelectionAdapter` with its own windowing-agnostic
`TouchEvent` and `TouchPhase` types. The demo example currently only processes
`WindowEvent::CursorMoved` and `WindowEvent::MouseInput`. Adding winit touch
event mapping completes the integration and provides a working reference for
users.

## User Story

As a developer integrating Gup with winit, I want a zero-boilerplate way to
convert winit touch events into Gup touch events so that I can enable touch
selection without writing manual mapping code.

## Acceptance Criteria

1. [x] `From<winit::event::Touch>` and `From<winit::event::TouchPhase>` impls
       convert winit touch events to Gup types
2. [x] The `interactive_selection_demo` handles `WindowEvent::Touch` events
3. [x] Conversions are behind a `winit` feature flag (or always available if
       winit is already a required dependency)
4. [x] Documentation and doc-tests cover the conversion

## Technical Tasks

- [x] Implement `From<winit::event::Touch>` for `TouchEvent`
- [x] Implement `From<winit::event::TouchPhase>` for `TouchPhase`
- [x] Update `interactive_selection_demo.rs` to process touch events
- [x] Add unit tests for conversions
- [x] Add doc examples

## Testing Strategy

- Unit tests for `From` conversions
- Manual testing on a touch-enabled device or emulator

## Risk Assessment

- **Low**: Straightforward type mapping; winit is already a dependency

## Definition of Done

- [x] `From` impls exist and compile
- [x] Demo processes touch events
- [x] All tests pass

## Implementation Summary

### What Was Implemented

1. **`From<winit::event::TouchPhase>` for `TouchPhase`** — Direct 1:1 mapping of
   all four variants (Started, Moved, Ended, Cancelled).

2. **`From<winit::event::Touch>` for `TouchEvent`** — Maps `id`, `location`
   (f64→f32), and `phase`. Sets `timestamp` to `0.0` since winit's `Touch` type
   does not carry a timestamp.

3. **`TouchEvent::from_winit(touch, timestamp)`** — Convenience constructor that
   accepts an explicit timestamp for accurate long-press and two-finger-tap
   gesture recognition.

4. **`interactive_selection_demo.rs` touch integration** — Added
   `TouchSelectionAdapter` and `Instant` start time to the demo. Handles
   `WindowEvent::Touch` by converting via `TouchEvent::from_winit()` with
   elapsed time. Ticks the adapter each frame for long-press detection.

### Key Files Changed

| File                                     | Change                                      |
| ---------------------------------------- | ------------------------------------------- |
| `src/mark_selection.rs`                  | +164 lines: From impls, from_winit(), tests |
| `examples/interactive_selection_demo.rs` | +31 lines: touch handling, adapter, docs    |

### Test Count

**7 new unit tests** + **3 doc-tests** (all CPU-only, <1 ms):

- `test_touch_phase_from_winit_started`
- `test_touch_phase_from_winit_moved`
- `test_touch_phase_from_winit_ended`
- `test_touch_phase_from_winit_cancelled`
- `test_touch_event_from_winit`
- `test_touch_event_from_winit_with_timestamp`
- `test_touch_event_from_winit_position_truncation`

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### Winit Touch Lacks Timestamps

- **Challenge**: `winit::event::Touch` does not carry a timestamp, but Gup's
  `TouchEvent` requires one for long-press and two-finger-tap gesture timing.
- **Solution**: Provided two conversion paths: `From<winit::event::Touch>` sets
  `timestamp` to `0.0` for simple cases, and `TouchEvent::from_winit(touch, ts)`
  accepts an explicit timestamp derived from `Instant::now()`.
- **Pattern**: When mapping between two types where the target has more fields
  than the source, provide both a lossy `From` impl (with documented defaults)
  and a named constructor that accepts the missing data. Document the trade-off.

#### DeviceId Construction in Doc-Tests

- **Challenge**: Creating a `winit::event::Touch` in doc-tests requires a
  `DeviceId`, which is an opaque platform type.
- **Solution**: `DeviceId::dummy()` exists precisely for this use case.
  Initially considered `unsafe { std::mem::zeroed() }` but the dummy constructor
  is the correct, safe approach.
- **Pattern**: Check library types for `dummy()`, `default()`, or test helpers
  before reaching for `unsafe` in doc-tests.

### Architectural Decisions

#### Always-Available vs Feature-Gated Conversions

- **Decision**: Made the `From` impls always available without a feature flag.
- **Reasoning**: `winit` is already a required (non-optional) dependency in
  Cargo.toml. Adding a feature flag would add complexity without benefit since
  every build already links winit.
- **Trade-off**: If winit becomes optional in the future, these impls would need
  to be gated behind `#[cfg(feature = "winit")]`.
- **Future**: If Gup supports alternative windowing systems (e.g. SDL2, glutin),
  the touch types remain windowing-agnostic and new `From` impls can be added
  per-backend.

#### Rebuilding Instances on Every Touch Event

- **Decision**: The demo calls `rebuild_instances()` on every touch event, not
  just on gesture completion.
- **Reasoning**: Touch events can trigger intermediate visual feedback (e.g.
  rectangle selection preview during drag). For 200 data points this is
  negligible.
- **Trade-off**: For larger datasets, a dirty flag or event-result-based
  approach would be more efficient.

### Development Workflow Insights

- This was a small, well-scoped story (effort 2) that took minimal time. The
  GUP-182 retrospective explicitly called out this exact follow-up, making the
  scope crystal clear.
- The `mask all-fix` workflow continues to be reliable — no issues with
  formatting or lint.
- Doc-tests serve as both documentation and regression tests. The three new
  doc-tests exercise the conversion paths and serve as copy-paste examples for
  users.

### Follow-up Stories

No new follow-up stories were identified. The existing GUP-234 (Touch Lasso
Selection) remains the natural next step for the touch interaction subsystem.
