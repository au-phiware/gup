# GUP-242: Shared UI Chrome Renderer

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Rendering Infrastructure  
**Priority**: Low  
**Story Points**: 5  
**Status**: 🚧 In Progress  
**Dependencies**: GUP-229 (Tooltip Background Rendering)

## Problem Statement

The tooltip background renderer (GUP-229) introduced a dedicated SDF shader and
instanced renderer for rounded rectangles with borders and shadows. As the
project adds more UI chrome elements (legend boxes, annotation callouts, context
menus, selection boxes), each will need similar rendering capabilities.
Consolidating these into a shared renderer avoids duplication and ensures
consistent visual treatment.

## User Story

**As a** library developer  
**I want** a shared renderer for UI overlay elements  
**So that** legends, tooltips, annotations, and other chrome share consistent
rendering without duplicating shader and pipeline code

## Acceptance Criteria

- [ ] A general-purpose `UiQuadRenderer` that renders rounded rectangles with
      configurable fill, border, corner radius, and shadow
- [ ] Tooltip background rendering migrated to use the shared renderer
- [ ] API supports queuing multiple heterogeneous UI elements per frame
- [ ] Suitable for future legend boxes, annotation backgrounds, and focus
      highlights

## Technical Tasks

1. Extract the tooltip background shader into a general-purpose `ui_quad.wgsl`
2. Create `UiQuadRenderer` with the same instanced rendering approach
3. Migrate `TooltipBackgroundRenderer` to delegate to `UiQuadRenderer`
4. Add typed builder for common UI quad configurations
5. Test with multiple simultaneous UI element types

## Dependencies

- GUP-229 (Tooltip Background Rendering) — provides the initial implementation

## Testing Strategy

- Unit tests for quad configuration builder
- Integration tests rendering multiple UI quad types in a single pass
- Backward-compatibility tests for tooltip rendering

## Success Metrics

- Single shader/pipeline handles all UI chrome rendering
- No regression in tooltip background rendering
- New UI elements (legends, annotations) can reuse the renderer

## Risk Assessment

- **Abstraction overhead**: A too-generic API may be harder to use than
  purpose-built renderers. Keep the API focused on rounded rectangles with
  optional extras.

## Definition of Done

- [ ] `UiQuadRenderer` implemented and tested
- [ ] Tooltip background delegates to shared renderer
- [ ] Documentation and usage examples
- [ ] Tests passing

---

**Story Created**: 2025-07-18  
**Origin**: GUP-229 retrospective follow-up
