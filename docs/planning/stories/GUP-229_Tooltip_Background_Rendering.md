# GUP-229: Tooltip Background Rendering

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 3  
**Status**: 📋 Planned  
**Dependencies**: GUP-200 (Interactive Clipping Reveal)

## Problem Statement

The hover reveal tooltip (GUP-200) currently renders only the text content
without a visual background. In a typical data visualization, tooltip text
overlaps with chart elements and is difficult to read without a contrasting
background box, border, and optional corner radius.

## User Story

**As a** chart user  
**I want** tooltips to have a visible background with configurable appearance  
**So that** I can clearly read tooltip content against any chart background

## Acceptance Criteria

- [ ] Tooltip renders with a solid background rectangle behind the text
- [ ] Background color, border color, and border width are configurable via
      `TooltipConfig`
- [ ] Optional corner radius for rounded tooltip boxes
- [ ] Background opacity matches the tooltip fade-in/fade-out animation
- [ ] Tooltip shadow or drop-shadow effect (optional)
- [ ] Background rendering uses the existing GPU mark/rectangle pipeline

## Technical Tasks

1. Create a tooltip background renderer using the Rectangle mark or a dedicated
   solid-color quad shader
2. Wire background rendering into the `TooltipLayout` positioning system
3. Ensure background renders behind (before) the tooltip text
4. Add corner radius support to the background rectangle
5. Test background rendering with various tooltip configurations

## Dependencies

- GUP-200 (Interactive Clipping Reveal) — provides `TooltipConfig`,
  `TooltipLayout`, and `ActiveTooltip`

## Testing Strategy

- Unit tests for background bounds calculation
- Visual integration tests verifying background renders behind text
- Tests for rounded corner rendering

## Success Metrics

- Tooltips are clearly readable over any chart background
- No visual artifacts at tooltip edges

## Risk Assessment

- **GPU pipeline complexity**: May need a new simple-quad render pipeline if the
  mark system is too heavy for this use case.
- **Z-ordering**: Background must render before text in the same render pass.

## Definition of Done

- [ ] Background rectangle renders behind tooltip text
- [ ] Corner radius supported
- [ ] Configurable via `TooltipConfig`
- [ ] Tests passing
- [ ] Updated demo showcasing the feature

---

**Story Created**: 2026-02-27  
**Origin**: GUP-200 retrospective follow-up
