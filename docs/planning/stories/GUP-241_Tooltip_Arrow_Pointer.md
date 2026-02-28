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
**Story Completed**: 2025-07-19  
**Origin**: GUP-229 retrospective follow-up

## Retrospective

**Completed**: 2025-07-19

### Key Technical Learnings

#### SDF Union for Composite Shapes

- **Challenge**: Merging a triangular arrow with the rounded rectangle tooltip
  body so they appear as a single seamless shape (fill, border, shadow all
  unified).
- **Solution**: Used `min(d_rect, d_triangle)` — the SDF union operator. Because
  both `sdf_rounded_rect` and `sdf_triangle` produce exact Euclidean distance
  fields, the union is also an exact distance field. This means the same
  anti-aliasing (`fwidth`), border (`d + border_width`), and shadow
  (`smoothstep`) logic works on the combined shape without modification.
- **Pattern**: SDF union via `min()` is the go-to technique for combining
  geometric primitives in a single fragment shader. For future UI shapes
  (callouts, pill buttons, badges), the same approach applies: define the
  component SDFs, union them, and the existing rendering pipeline handles fill,
  border, and shadow automatically.

#### SDF Border via Offset Instead of Separate Geometry

- **Challenge**: The original shader computed the border as the annulus between
  the outer rounded rect SDF and a separately-computed inner rounded rect SDF.
  With the triangle addition, computing a separate inner triangle (shrunk by
  `border_width`) would add complexity.
- **Solution**: Replaced the separate inner rect SDF with `d + border_width`.
  For an exact distance field, `d + c` is the SDF of the shape inset by `c` —
  mathematically equivalent to the original approach but works on any SDF shape,
  not just rounded rects.
- **Pattern**: Use SDF offset (`d + c`) for borders/insets on composite shapes.
  This is simpler, handles arbitrary SDF shapes, and produces identical results
  for exact distance fields.

#### Triangle SDF Vertex Winding

- **Challenge**: The Inigo Quilez triangle SDF formula is sensitive to vertex
  winding order. Counter-clockwise vertices produce negative distances inside
  the triangle; clockwise gives positive inside. Getting the winding wrong
  inverts the shape.
- **Solution**: Carefully defined triangle vertices for each direction to ensure
  counter-clockwise winding. For example, the "bottom" arrow (pointing down)
  uses `(center, half_size.y + size)`, `(center + size, half_size.y)`,
  `(center - size, half_size.y)` — the base corners are ordered right-to-left
  to maintain CCW winding.
- **Pattern**: When using the Quilez triangle SDF, always verify winding order
  by visualisation or by checking the sign convention: CCW = negative inside.

### Architectural Decisions

#### ArrowDirection::Auto Over Implicit Direction

- **Decision**: Added an explicit `Auto` variant to `ArrowDirection` that
  resolves to `Top` or `Bottom` in `compute_tooltip_layout`, rather than always
  computing direction from tooltip position.
- **Reasoning**: Explicit variants (`Top`, `Bottom`, `Left`, `Right`) let
  callers force a specific direction for custom layouts. `Auto` is a convenience
  that picks based on tooltip flip state. `None` disables the arrow entirely.
  This three-tier approach (disabled/auto/explicit) follows the pattern of
  `shadow_radius: 0.0` for opt-in shadows.
- **Trade-off**: The `Auto` variant cannot be sent to the GPU — it must be
  resolved on the CPU before encoding. This is by design (GPU shader uses float
  encoding 0–4), but it means `ArrowDirection::Auto.to_f32()` returns 0.0
  (none) as a safety fallback.
- **Future**: If tooltips need to point left/right (e.g., side-anchored
  tooltips), the `Left`/`Right` variants are already implemented and the shader
  handles them.

#### Extra Vertical Gap for Arrow

- **Decision**: When the arrow is enabled, `compute_tooltip_layout` adds
  `arrow_size` to the vertical gap between source bounds and tooltip rect. This
  prevents the arrow from overlapping the source text.
- **Reasoning**: Without this adjustment, a 6px arrow with a 4px `offset_y`
  would have its tip 2px above the tooltip top — overlapping the source text by
  2px. Adding `arrow_size` to the gap keeps the tooltip-to-source distance
  consistent regardless of whether the arrow is enabled.
- **Trade-off**: The tooltip appears slightly further from the source when the
  arrow is enabled. This is intentional and matches standard tooltip behaviour.

### Development Workflow Insights

- The implementation was straightforward because GUP-229 had established a clean
  architecture: the SDF shader, instance buffer, and rendering pipeline all
  extended naturally. The arrow was essentially "one more SDF" unioned into the
  existing shape.
- The `bytemuck::Pod` struct change from `_padding: [f32; 2]` to
  `arrow_params: [f32; 4]` changed the struct size from 96 to 104 bytes. This
  is fine — `repr(C)` with all `[f32; N]` fields has 4-byte alignment and no
  implicit padding.
- Integration tests that create a real GPU context and run the render pass are
  essential for validating shader compilation with the new `@location(8)` vertex
  input.
- The `mask all-fix` → `cargo check` → `cargo test` workflow caught a module
  path issue (`super::hover_reveal::ArrowDirection` vs the imported form)
  immediately.

### Follow-up Stories

1. **GUP-242: Shared UI Chrome Renderer** — Already planned. With both the
   tooltip background (rounded rect + border + shadow) and now the arrow using
   SDF techniques, there's a growing body of "UI chrome" rendering code. If
   legends, annotations, or other UI overlays need similar rendering, GUP-242
   would consolidate these into a general-purpose SDF-based UI renderer.
