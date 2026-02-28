# GUP-242: Shared UI Chrome Renderer

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Rendering Infrastructure  
**Priority**: Low  
**Story Points**: 5  
**Status**: ✅ Complete  
**Completed**: 2025-07-20  
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

- [x] A general-purpose `UiQuadRenderer` that renders rounded rectangles with
      configurable fill, border, corner radius, and shadow
- [x] Tooltip background rendering migrated to use the shared renderer
- [x] API supports queuing multiple heterogeneous UI elements per frame
- [x] Suitable for future legend boxes, annotation backgrounds, and focus
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

- [x] `UiQuadRenderer` implemented and tested
- [x] Tooltip background delegates to shared renderer
- [x] Documentation and usage examples
- [x] Tests passing

## Implementation Summary

### What Was Implemented

A general-purpose `UiQuadRenderer` that consolidates all UI chrome rendering
(rounded rectangles with fill, border, corner radius, shadow, and arrow pointer)
into a single shared shader and GPU pipeline.  `TooltipBackgroundRenderer` was
refactored into a thin facade that delegates to the shared renderer.

### Key Files Changed

- **`src/text/ui_quad.rs`** (new) — `UiQuadRenderer`, `UiQuadInstance`,
  `UiQuadConfig` builder, `UiQuadArrow` enum
- **`src/shaders/ui_quad.wgsl`** (new) — Shared SDF shader for all UI chrome
  rendering (rounded rect + arrow + shadow + border)
- **`src/text/tooltip_bg.rs`** — Rewritten to delegate to `UiQuadRenderer`
- **`src/shaders/tooltip_bg.wgsl`** (removed) — Replaced by `ui_quad.wgsl`
- **`src/text.rs`** — Added `ui_quad` submodule and re-exports
- **`src/prelude.rs`** — Exported `UiQuadRenderer`, `UiQuadInstance`,
  `UiQuadConfig`, `UiQuadArrow`
- **`tests/ui_quad_tests.rs`** (new) — 9 GPU integration tests

### Test Counts

- 6 unit tests in `text::ui_quad::tests`
- 5 unit tests in `text::tooltip_bg::tests`
- 9 integration tests in `tests/ui_quad_tests.rs`
- 8 existing integration tests in `tests/tooltip_bg_tests.rs` (all pass
  unchanged)

---

**Story Created**: 2025-07-18  
**Origin**: GUP-229 retrospective follow-up
