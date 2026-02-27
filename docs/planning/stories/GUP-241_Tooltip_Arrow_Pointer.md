# GUP-241: Tooltip Arrow/Pointer

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 2  
**Status**: 💡 New  
**Dependencies**: GUP-229 (Tooltip Background Rendering)

## Problem Statement

The tooltip background (GUP-229) renders a rounded rectangle, but there is no
visual indicator connecting the tooltip to the source text element. A triangular
arrow/pointer extending from the tooltip toward the hovered element would
improve the visual association and match user expectations from common UI
tooltip patterns.

## User Story

**As a** chart user  
**I want** tooltips to have an arrow pointing at the source element  
**So that** I can clearly see which element the tooltip refers to

## Acceptance Criteria

- [ ] Tooltip has an optional triangular pointer/arrow
- [ ] Arrow direction is configurable (top, bottom, left, right)
- [ ] Arrow automatically points toward the source element
- [ ] Arrow colour matches the tooltip background
- [ ] Arrow integrates with the existing SDF shader

## Technical Tasks

1. Extend `tooltip_bg.wgsl` with a triangle SDF union for the arrow
2. Add `arrow_size` and `arrow_enabled` fields to `TooltipConfig`
3. Compute arrow position from `TooltipLayout` source bounds
4. Test arrow rendering at different positions and orientations

## Dependencies

- GUP-229 (Tooltip Background Rendering) — provides the SDF shader and renderer

## Testing Strategy

- Unit tests for arrow position computation
- Integration tests verifying shader compilation with arrow variants
- Visual verification with the hover reveal demo

## Success Metrics

- Arrow clearly indicates which element the tooltip refers to
- No visual artifacts at the arrow-to-rectangle junction

## Risk Assessment

- **SDF complexity**: Union of rectangle and triangle SDF requires careful edge
  handling at the junction.

## Definition of Done

- [ ] Arrow renders on tooltip boxes
- [ ] Configurable via `TooltipConfig`
- [ ] Tests passing
- [ ] Demo updated

---

**Story Created**: 2025-07-18  
**Origin**: GUP-229 retrospective follow-up
