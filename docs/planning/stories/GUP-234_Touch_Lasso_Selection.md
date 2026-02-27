# GUP-234: Touch Lasso Selection

**Status**: 📋 Planned **Priority**: Low **Effort**: 3 **Dependencies**: GUP-182
(Touch Selection Support)

## Overview

Extend the `TouchSelectionAdapter` to support lasso (free-form) selection via
touch gestures. The current adapter recognises single tap, long-press,
two-finger tap, and single-finger drag (rectangle). This story adds a gesture
for lasso selection, such as a long-press-then-drag or a dedicated mode toggle.

## Context

GUP-182 mapped the most common touch gestures to selection actions but deferred
lasso support. Lasso selection is useful for irregularly shaped clusters of marks
that cannot be captured with a rectangle.

## User Story

As a user on a touch device, I want to draw a free-form selection path with my
finger so that I can select groups of marks that are not axis-aligned.

## Acceptance Criteria

1. A touch gesture activates lasso selection mode (e.g. long-press-then-drag or
   three-finger drag)
2. The lasso path is rendered as visual feedback during the gesture
3. Marks within the closed lasso path are selected on finger lift
4. The gesture integrates with existing `SelectionTool::Lasso` lifecycle
5. Unit tests cover the lasso gesture path

## Technical Tasks

- [ ] Design and implement lasso activation gesture
- [ ] Extend `TouchSelectionAdapter` state machine with lasso state
- [ ] Integrate with `SelectionTool::Lasso` begin/update/finish
- [ ] Add unit tests for lasso gesture
- [ ] Update documentation

## Testing Strategy

- Unit tests with simulated multi-point touch sequences
- Manual testing on a touch-enabled device

## Risk Assessment

- **Medium**: Choosing the right gesture is a UX decision — the gesture must be
  discoverable but not conflict with drag (rectangle) or long-press (toggle)
- **Low**: The lasso tool infrastructure already exists in `MarkSelectionSystem`

## Definition of Done

- [ ] Lasso gesture activates and completes lasso selection
- [ ] Visual feedback renders the lasso path
- [ ] All tests pass
