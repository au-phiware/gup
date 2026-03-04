# GUP-285: BrushMark GPU Overlay Rendering

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete
**Created**: 2025-07-25 **Completed**: 2026-03-05

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

- [x] When `BrushMark.visible` is `true`, a filled+stroked rectangle is rendered
      at `screen_rect` coordinates using the `BrushStyle` colours.
- [x] The overlay rectangle disappears immediately when `BrushMark.visible`
      becomes `false` (drag ended or cancelled).
- [x] The overlay is drawn after all data marks in the render pass (highest
      z-order).
- [x] No GPU validation errors on Vulkan or Metal backends.
- [x] The `brush_selection` example visually shows the brush rectangle during
      drag.

## Technical Tasks

- [x] Create a `BrushOverlayRenderer` (or extend `Selection`/render loop) that
      allocates a single `RectangleInstance` GPU buffer for the overlay.
- [x] Each frame, if `BrushMark.visible`, update the vertex buffer with the
      current `screen_rect` and `BrushStyle`.
- [x] Render the overlay as the last draw call in the chart's render pass.
- [x] Update `examples/brush_selection.rs` to use the overlay renderer.
- [x] Write a visual test confirming overlay appears and disappears.

## Implementation Summary

### What Was Implemented

- **`BrushOverlayRenderer`** — a dedicated GPU renderer in `src/brush.rs` that
  allocates a single `RectangleInstance` storage buffer, reuses the cached
  Rectangle render pipeline via `PipelineCache`, and draws one instanced quad
  each frame the brush is visible.
- **`update()` method** — converts `BrushMark.screen_rect` (clip-space min/max)
  to `RectangleInstance` centre + size, uploads `BrushStyle` fill/stroke colours
  each frame.
- **`render()` method** — issues a single indexed draw call as the last step in
  the render pass, ensuring highest z-order.
- **`set_viewport_size()`** — keeps the viewport dimensions uniform in sync with
  the window size for correct SDF rendering.
- **Example update** — `examples/brush_selection.rs` now lazily creates a
  `BrushOverlayRenderer`, calls `update` before the render pass, and `render`
  after data marks inside the pass.

### Key Files Changed

| File                           | Change                                 |
| ------------------------------ | -------------------------------------- |
| `src/brush.rs`                 | Added `BrushOverlayRenderer` + 5 tests |
| `src/lib.rs`                   | Re-exported `BrushOverlayRenderer`     |
| `examples/brush_selection.rs`  | Integrated overlay renderer            |

### Test Counts

- 5 new GPU tests (`overlay_renderer_creation`, `overlay_visible_when_brush_shown`,
  `overlay_hidden_when_brush_hidden`, `overlay_hidden_for_default_brush_mark`,
  `overlay_reuses_cached_pipeline`)
- 31 total brush module tests — all passing

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document
