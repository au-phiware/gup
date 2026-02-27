# GUP-234: Touch Lasso Selection

**Status**: ✅ Complete **Completed**: 2025-07-19 **Priority**: Low **Effort**: 3
**Dependencies**: GUP-182 (Touch Selection Support)

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

1. [x] A touch gesture activates lasso selection mode (e.g.
   long-press-then-drag or three-finger drag)
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
     or movement (lasso). Toggle action is deferred until finger-lift rather than
     firing at threshold time.
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

| File                    | Change                                         |
| ----------------------- | ---------------------------------------------- |
| `src/mark_selection.rs` | +268 lines: new states, handlers, 7 new tests  |

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
