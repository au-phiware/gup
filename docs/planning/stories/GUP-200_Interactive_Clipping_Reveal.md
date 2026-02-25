# GUP-200: Interactive Clipping Reveal

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 3  
**Status**: 📋 Planned  
**Dependencies**: GUP-105 (Text Clipping Detection), GUP-012 (GPU Interaction
System)

## Problem Statement

When text is truncated with ellipsis or hidden by clipping strategies, users
have no way to see the full content. A hover/click interaction that reveals the
complete text would significantly improve the user experience for data
visualizations with dense or constrained labels.

## User Story

**As a** chart user  
**I want** to hover over truncated text to see the full content  
**So that** I can read complete labels even when space is constrained

## Acceptance Criteria

- [ ] Hover detection on truncated text elements (using `LayoutResult.clipped`
      flag)
- [ ] Tooltip or expanded text display showing full content
- [ ] Smooth appearance/disappearance transitions
- [ ] Integration with existing GPU interaction/hit testing system
- [ ] Configurable via `ClippingStrategyConfig.enable_hover_reveal`

## Technical Tasks

1. Connect `LayoutResult.clipped` with interaction hit test regions
2. Store original (un-truncated) text alongside truncated rendering
3. Implement tooltip or expanded overlay rendering
4. Add configuration to enable/disable hover reveal
5. Integration tests with interaction system

## Testing Strategy

- Integration tests for hover detection on clipped text
- Visual tests for tooltip rendering
- Performance tests to ensure hover checking adds minimal overhead

## Definition of Done

- [ ] Hover reveal implemented for truncated text
- [ ] Tests passing
- [ ] Performance within acceptable bounds
- [ ] Demo showcasing the feature

---

**Story Created**: 2026-02-26  
**Origin**: GUP-105 follow-up ("Could Have" AC not implemented)
