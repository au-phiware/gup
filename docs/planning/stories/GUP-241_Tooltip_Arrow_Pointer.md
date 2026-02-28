# GUP-241: Tooltip Arrow/Pointer

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 2  
**Status**: ✅ Complete  
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

- [x] Tooltip has an optional triangular pointer/arrow
- [x] Arrow direction is configurable (top, bottom, left, right)
- [x] Arrow automatically points toward the source element
- [x] Arrow colour matches the tooltip background
- [x] Arrow integrates with the existing SDF shader

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

- [x] Arrow renders on tooltip boxes
- [x] Configurable via `TooltipConfig`
- [x] Tests passing
- [x] Demo updated

## Implementation Summary

**Completed**: 2025-07-19

### Architecture

The tooltip arrow is implemented as an SDF union of the existing rounded
rectangle with a triangle SDF. A new `sdf_triangle()` function (exact Inigo
Quilez formulation) computes the signed distance to an arbitrary triangle. The
`sdf_tooltip()` function selects triangle vertices based on the arrow direction
(top, bottom, left, right) and unions the result with `sdf_rounded_rect()` via
`min(d_rect, d_triangle)`. The combined SDF is used uniformly for fill, border,
shadow, and anti-aliasing — the arrow seamlessly merges with the rectangle at
the junction.

### Key Files Changed

- **`src/text/hover_reveal.rs`** — Added `ArrowDirection` enum
  (`None`/`Top`/`Bottom`/`Left`/`Right`/`Auto`) with GPU encoding. Extended
  `TooltipConfig` with `arrow_direction` and `arrow_size` fields. Extended
  `TooltipLayout` with `arrow_direction`, `arrow_size`, and `arrow_offset`.
  Updated `compute_tooltip_layout` to resolve Auto direction based on tooltip
  flip state, compute arrow offset from source bounds, and add extra gap for
  the arrow height. Added 8 unit tests.
- **`src/shaders/tooltip_bg.wgsl`** — Added `sdf_triangle()` (exact 2D triangle
  SDF), `sdf_tooltip()` (rect+arrow union). Added `arrow_params` vertex input
  (`@location(8)`). Updated vertex shader to expand the quad for arrow size.
  Refactored fragment shader to use the combined SDF for all rendering.
- **`src/text/tooltip_bg.rs`** — Replaced `_padding` with `arrow_params` in
  `TooltipBgInstance` (struct now 104 bytes). Updated `queue()` to populate
  arrow parameters. Added vertex attribute for `arrow_params` at offset 88.
  Added 2 unit tests.
- **`src/prelude.rs`** — Exported `ArrowDirection`.
- **`examples/hover_reveal_demo.rs`** — Enabled arrow with `Auto` direction and
  `6.0` size.
- **`tests/tooltip_bg_tests.rs`** — Added 2 GPU integration tests (single arrow,
  all directions).

### Test Counts

- **31 unit tests** in `text::hover_reveal::tests` (23 existing + 8 new arrow tests)
- **7 unit tests** in `text::tooltip_bg::tests` (5 existing + 2 new arrow tests)
- **8 integration tests** in `tests/tooltip_bg_tests.rs` (6 existing + 2 new arrow tests)
- All existing hover_reveal integration tests continue to pass

---

**Story Created**: 2025-07-18  
**Origin**: GUP-229 retrospective follow-up
