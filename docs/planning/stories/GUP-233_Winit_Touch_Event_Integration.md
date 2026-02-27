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
