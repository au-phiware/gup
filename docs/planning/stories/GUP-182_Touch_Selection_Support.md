# GUP-182: Touch Selection Support

**Status**: 📋 Planned **Priority**: Low **Effort**: 3 **Dependencies**: GUP-075
(Interactive Mark Selection), GUP-012 (GPU Interaction System)

## Overview

Add touch gesture support to the mark selection system, enabling interactive
mark selection on mobile and touch-screen devices.

## Context

GUP-075 delivered mouse-based selection tools (point, rectangle, lasso) with
keyboard modifier support. The `GestureRecognizer` from GUP-012 already detects
multi-touch gestures (pinch, rotate, swipe, pan). This story integrates touch
gestures with the selection system.

## User Story

As a user viewing a visualization on a touch-enabled device, I want to tap,
drag, and pinch to select marks so that I can interact with the data without a
mouse or keyboard.

## Acceptance Criteria

1. Single tap selects/deselects individual marks
2. Long press enters toggle mode (equivalent to Ctrl+Click)
3. Two-finger tap clears selection
4. Drag gesture activates rectangle selection tool
5. Touch targets have configurable minimum size for accessibility
6. Selection tools provide haptic feedback on supported platforms

## Technical Tasks

- [ ] Map `GestureRecognizer` events to `MarkSelectionSystem` actions
- [ ] Implement long-press detection for toggle mode
- [ ] Add minimum touch target size configuration
- [ ] Integrate with `SelectionTool` begin/update/finish lifecycle
- [ ] Test on touch-enabled platforms

## Testing Strategy

- Unit tests with simulated touch events
- Manual testing on touch-enabled devices
- Accessibility review for touch target sizes

## Risk Assessment

- **Medium**: Touch behaviour varies across platforms (iOS, Android, Windows)
- **Low**: The gesture recognizer foundation already exists

## Definition of Done

- [ ] Touch gestures work for point and rectangle selection
- [ ] Long-press toggle mode works
- [ ] Minimum touch target sizes are configurable
- [ ] Manual testing on at least one touch platform
- [ ] All tests pass
