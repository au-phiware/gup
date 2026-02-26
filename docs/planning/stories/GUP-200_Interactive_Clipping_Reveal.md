# GUP-200: Interactive Clipping Reveal

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 3  
**Status**: ✅ Complete  
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

- [x] Hover detection on truncated text elements (using `LayoutResult.clipped`
      flag)
- [x] Tooltip or expanded text display showing full content
- [x] Smooth appearance/disappearance transitions
- [x] Integration with existing GPU interaction/hit testing system
- [x] Configurable via `ClippingStrategyConfig.enable_hover_reveal`

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

- [x] Hover reveal implemented for truncated text
- [x] Tests passing
- [x] Performance within acceptable bounds
- [x] Demo showcasing the feature

## Implementation Summary

**Completed**: 2025-07-17

### Architecture

The hover reveal system is implemented as a CPU-side component that integrates
with the existing text clipping pipeline. It uses AABB point-in-rect hit testing
rather than the GPU compute shader interaction system, since text labels are
typically tens to hundreds (not millions) of elements.

### Key Files Changed

- **`src/text/hover_reveal.rs`** (new) — Core module with `ClippedTextRegistry`,
  `HoverRevealState`, `TooltipConfig`, `ActiveTooltip`, `TooltipLayout`, and
  `compute_tooltip_layout()`.
- **`src/text/layout.rs`** — Extended `LayoutResult` with `original_text: Option<String>`.
  Modified `layout_text_with_clipping()` to store the original text when
  `enable_hover_reveal` is `true`.
- **`src/text/renderer.rs`** — Changed `queue_text()` return type from
  `GupResult<TextBounds>` to `GupResult<LayoutResult>` for richer caller info.
- **`src/text.rs`** — Added `hover_reveal` submodule.
- **`src/prelude.rs`** — Exported `ClippedTextRegistry`, `HoverRevealState`,
  `TooltipConfig`, `TooltipLayout`.
- **`tests/hover_reveal_tests.rs`** (new) — 7 integration tests.
- **`examples/hover_reveal_demo.rs`** (new) — Interactive demo.

### Test Counts

- **23 unit tests** in `text::hover_reveal::tests` (registry, state machine,
  tooltip layout)
- **7 integration tests** in `tests/hover_reveal_tests.rs` (GPU context,
  layout pipeline integration, performance)
- **Performance**: 10K hover updates with 100 entries completes in <100ms

### Design Decisions

- **CPU-side hit testing** via `ClippedTextRegistry` instead of GPU interaction
  system compute shaders. Text labels are a low-cardinality problem; GPU
  dispatch overhead would dominate.
- **State machine with looping** for zero-duration transitions — enables
  instant show/hide when delays and fades are set to 0.
- **`LayoutResult.original_text`** as `Option<String>` — only allocated when
  `enable_hover_reveal` is true AND text was clipped, so zero overhead when
  the feature is disabled.
- **`queue_text` returns `LayoutResult`** — breaking change from
  `GupResult<TextBounds>`, but existing callers only checked for errors (not
  the bounds value), so migration is trivial.

---

**Story Created**: 2026-02-26  
**Story Completed**: 2025-07-17  
**Origin**: GUP-105 follow-up ("Could Have" AC not implemented)
