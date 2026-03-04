# GUP-285: BrushMark GPU Overlay Rendering

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress **Created**:
2025-07-25

## Context

GUP-278 delivered a `BrushBehavior` API with full drag lifecycle management, CPU
hit testing, viewport-aware coordinate transforms, and a `BrushMark` type that
tracks overlay state (visible, screen*rect, style). However, the `BrushMark`
currently stores rendering \_intent* without producing visible GPU geometry. The
brush selection works correctly — events fire, IDs are returned — but the user
does not see a semi-transparent rectangle tracking their drag.

This story bridges the gap by wiring `BrushMark.screen_rect` into the chart's
render loop as a `RectangleInstance` overlay, drawn after all data marks to
ensure correct z-order.

## User Story

> "As an end user, I want to see a semi-transparent rectangle track my drag so
> that I understand the current selection region before I release the mouse."

## Acceptance Criteria

- [ ] When `BrushMark.visible` is `true`, a filled+stroked rectangle is rendered
      at `screen_rect` coordinates using the `BrushStyle` colours.
- [ ] The overlay rectangle disappears immediately when `BrushMark.visible`
      becomes `false` (drag ended or cancelled).
- [ ] The overlay is drawn after all data marks in the render pass (highest
      z-order).
- [ ] No GPU validation errors on Vulkan or Metal backends.
- [ ] The `brush_selection` example visually shows the brush rectangle during
      drag.

## Technical Tasks

- [ ] Create a `BrushOverlayRenderer` (or extend `Selection`/render loop) that
      allocates a single `RectangleInstance` GPU buffer for the overlay.
- [ ] Each frame, if `BrushMark.visible`, update the vertex buffer with the
      current `screen_rect` and `BrushStyle`.
- [ ] Render the overlay as the last draw call in the chart's render pass.
- [ ] Update `examples/brush_selection.rs` to use the overlay renderer.
- [ ] Write a visual test confirming overlay appears and disappears.

## Dependencies

### Prerequisite Stories

- GUP-278: Brush Mark for Rectangular Selection ✅ — provides `BrushMark`,
  `BrushStyle`, and the state management.
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides
  `RectangleInstance` / `RectangleVertex` GPU geometry.

## Testing Strategy

- Visual validation: Run `brush_selection` example, confirm rectangle tracks
  cursor during drag and disappears on release.
- GPU validation: No validation errors on wgpu validation layer.

## Risk Assessment

- **Low**: Single-instance buffer update per frame is negligible overhead.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
