# GUP-234: Touch Lasso Selection

**Status**: ✅ Complete **Completed**: 2025-07-19 **Priority**: Low **Effort**:
3 **Dependencies**: GUP-182 (Touch Selection Support)

## Overview

Extend the `TouchSelectionAdapter` to support lasso (free-form) selection via
touch gestures. The current adapter recognises single tap, long-press,
two-finger tap, and single-finger drag (rectangle). This story adds a gesture
for lasso selection, such as a long-press-then-drag or a dedicated mode toggle.

## Context

GUP-182 mapped the most common touch gestures to selection actions but deferred
lasso support. Lasso selection is useful for irregularly shaped clusters of
marks that cannot be captured with a rectangle.

## User Story

As a user on a touch device, I want to draw a free-form selection path with my
finger so that I can select groups of marks that are not axis-aligned.

## Acceptance Criteria

1. [x] A touch gesture activates lasso selection mode (e.g. long-press-then-drag
       or three-finger drag)
2. [x] The lasso path is rendered as visual feedback during the gesture
3. [x] Marks within the closed lasso path are selected on finger lift
4. [x] The gesture integrates with existing `SelectionTool::Lasso` lifecycle
5. [x] Unit tests cover the lasso gesture path

## Technical Tasks

- [x] Design and implement lasso activation gesture
- [x] Extend `TouchSelectionAdapter` state machine with lasso state
- [x] Integrate with `SelectionTool::Lasso` begin/update/finish
- [x] Add unit tests for lasso gesture
- [x] Update documentation

## Testing Strategy

- Unit tests with simulated multi-point touch sequences
- Manual testing on a touch-enabled device

## Risk Assessment

- **Medium**: Choosing the right gesture is a UX decision — the gesture must be
  discoverable but not conflict with drag (rectangle) or long-press (toggle)
- **Low**: The lasso tool infrastructure already exists in `MarkSelectionSystem`

## Definition of Done

- [x] Lasso gesture activates and completes lasso selection
- [x] Visual feedback renders the lasso path
- [x] All tests pass

## Implementation Summary

### What Was Implemented

1. **Lasso activation gesture** — Long-press-then-drag: user holds finger for
   ≥0.5 s (triggers haptic feedback), then drags to draw a free-form lasso path.
   This gesture naturally complements existing gestures without conflicts:
   - Quick drag → rectangle selection (no long-press)
   - Long-press then lift → toggle (no drag)
   - Long-press then drag → **lasso selection** (new)

2. **Refactored long-press state machine** — Replaced the `LongPressCommitted`
   state with two new states:
   - `LongPressHeld` — After long-press threshold, awaiting finger lift (toggle)
     or movement (lasso). Toggle action is deferred until finger-lift rather
     than firing at threshold time.
   - `LassoDrawing` — Drawing a lasso path after movement from `LongPressHeld`.

3. **`SelectionTool::Lasso` integration** — The lasso gesture uses the existing
   tool lifecycle:
   - `set_tool(Lasso)` + `on_mouse_down(start)` when entering `LassoDrawing`
   - `on_mouse_move(pos)` for each touch move
   - `lasso_hit_test()` + `on_mouse_up()` on finger lift
   - Tool restored to `Point` after completion

4. **Visual feedback** — During lasso drawing,
   `selection.current_lasso_points()` returns the accumulated path for rendering
   by the application (same pattern as `current_drag_rect()` for rectangle
   selection).

### Key Files Changed

| File                    | Change                                        |
| ----------------------- | --------------------------------------------- |
| `src/mark_selection.rs` | +268 lines: new states, handlers, 7 new tests |

### Test Count

**7 new unit tests** (all CPU-only, <1 ms) + **1 updated test**:

- `test_touch_lasso_long_press_then_drag_selects_marks`
- `test_touch_lasso_visual_feedback_available`
- `test_touch_lasso_empty_selects_nothing`
- `test_touch_lasso_cancel_aborts`
- `test_touch_lasso_haptic_disabled`
- `test_touch_lasso_reset_during_drawing`
- `test_touch_long_press_then_lift_without_movement_toggles`
- `test_touch_long_press_toggle` (updated for deferred-commit behavior)

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### Deferred-Commit vs Immediate-Commit in Gesture Recognition

- **Challenge**: The original `tick()` method committed the long-press toggle
  action immediately when the time threshold was reached. This made it
  impossible to reinterpret the gesture as a lasso if the user subsequently
  dragged — the toggle had already fired.
- **Solution**: Refactored to a deferred-commit model: `tick()` transitions to
  `LongPressHeld` (haptic feedback as confirmation) but does NOT commit the
  toggle. The toggle fires only when the finger lifts from `LongPressHeld`.
  Movement from `LongPressHeld` transitions to `LassoDrawing` instead.
- **Pattern**: In multi-modal gesture recognition, delay committing actions as
  long as possible. Give haptic/visual feedback at the threshold crossing to
  confirm the user has entered a new mode, but let subsequent input determine
  the final action. This enables a single gesture prefix (long-press) to branch
  into multiple outcomes (toggle on lift, lasso on drag).

#### Reusing Existing Tool Lifecycle for New Gestures

- **Challenge**: The touch adapter needed to drive the lasso selection, but the
  `SelectionTool::Lasso` lifecycle (begin/update/finish + hit testing) was
  designed for mouse input through `MarkSelectionSystem`.
- **Solution**: The adapter drives the same API surface — `set_tool(Lasso)`,
  `on_mouse_down()`, `on_mouse_move()`, `on_mouse_up()` — which means the lasso
  path, visual feedback (`current_lasso_points()`), and hit testing all work
  identically to the mouse-driven path.
- **Pattern**: When adding a new input modality (touch) for an existing feature
  (lasso), prefer routing through the same code path rather than building a
  parallel implementation. The adapter pattern isolates input mapping from tool
  logic.

### Architectural Decisions

#### Long-Press-Then-Drag vs Three-Finger Drag

- **Decision**: Used long-press-then-drag as the lasso activation gesture rather
  than three-finger drag.
- **Reasoning**: Long-press is already a recognized gesture in the adapter's
  vocabulary (it was used for toggle). Extending it to branch into lasso on drag
  is natural and discoverable — the haptic feedback confirms the user has
  entered a special mode. Three-finger drag would be harder to discover and
  perform reliably on all devices.
- **Trade-off**: The long-press delay (0.5 s) adds latency before lasso drawing
  begins. For power users who want to quickly lasso, this may feel slow.
  However, the delay is configurable via
  `TouchSelectionConfig::long_press_duration`.
- **Future**: If three-finger drag or a dedicated mode button is needed, the
  `LassoDrawing` state is already decoupled from the activation gesture, making
  it easy to add alternative entry points.

#### Deferred Toggle Commit

- **Decision**: Changed long-press toggle to fire at finger-lift instead of at
  threshold crossing.
- **Reasoning**: Required for lasso support — if the toggle fires at threshold
  time, the user can't subsequently choose to draw a lasso. The deferred model
  is also arguably better UX: the user sees haptic feedback confirming the
  threshold, then decides what to do (lift for toggle, drag for lasso).
- **Trade-off**: This is a minor behavioral change from GUP-182. The toggle
  result is identical (hold-then-lift still toggles), but the timing of the
  state change differs. Updated the existing test to match.
- **Future**: This pattern could enable additional long-press variants (e.g.
  long-press then swipe for a different action) without further refactoring.

### Development Workflow Insights

- The implementation required only 1 file change (`src/mark_selection.rs`),
  which validates the adapter pattern — all touch logic is co-located with the
  selection system it drives.
- All 7 new tests are pure CPU state-machine tests (no GPU resources needed),
  running in <1 ms total. The decoupled architecture continues to pay dividends
  for testing speed.
- The existing `point_in_polygon` function and `SelectionTool::Lasso` lifecycle
  worked without modification — the touch adapter simply drives them through the
  standard mouse-event API surface. Zero changes were needed to the core
  selection logic.
