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

## Retrospective

**Completed**: 2026-03-05

### Key Technical Learnings

#### Rectangle Pipeline Bind Group Layout

- **Challenge**: The `BrushOverlayRenderer` reuses the cached Rectangle render
  pipeline, but the pipeline's bind group 0 expects **two** bindings — a storage
  buffer for instance data (binding 0) and a uniform buffer for viewport
  dimensions (binding 1) — not just the storage buffer alone.
- **Solution**: Inspected `MarkInfoImpl::create_bind_group_layout` and found that
  marks with custom shaders (`has_custom_shaders() == true`) always include a
  viewport dimensions uniform at binding 1.  Added the matching
  `ViewportUniforms` buffer to the overlay's bind group.
- **Pattern**: When reusing a pipeline from `PipelineCache`, always match the
  full bind group layout exactly — the pipeline layout is determined at creation
  time and cannot be changed.

#### wgpu v26 API Changes

- **Challenge**: The `Adapter::request_device` method signature changed in
  wgpu v26 — it no longer takes a second `Option<&Path>` trace parameter.
- **Solution**: Removed the extra `None` argument to match the v26 API.
- **Pattern**: Always check method signatures against the actual dependency
  version rather than relying on memory.

### Architectural Decisions

#### Dedicated Renderer vs Selection Reuse

- **Decision**: Created a standalone `BrushOverlayRenderer` rather than reusing
  `Selection<BrushMark, Rectangle>`.
- **Reasoning**: `Selection` is designed for binding collections of data to mark
  instances with shader function pipelines. The brush overlay is always exactly
  one instance with known, fixed attributes — no data binding, no shader
  functions. A dedicated type is simpler, has zero allocation overhead, and makes
  the API clearer.
- **Trade-off**: Small amount of duplicated buffer management code between
  `BrushOverlayRenderer` and `SelectionRenderState`.
- **Future**: If more single-instance overlays are needed (e.g., crosshair,
  tooltip pointer), a shared `SingleInstanceRenderer<M: Mark>` could be
  extracted.

#### Identity Viewport Transform

- **Decision**: Set the viewport transform (group 1 uniform) to identity since
  brush coordinates are already in clip space.
- **Reasoning**: The `BrushBehavior` stores drag positions in whatever coordinate
  space the caller provides. In the `brush_selection` example, positions are
  converted to clip space before being passed in. An identity viewport transform
  preserves these coordinates.
- **Trade-off**: The overlay doesn't respond to zoom/pan — this is intentional
  since the brush rectangle should always track screen-space cursor position.
- **Future**: If brush needs to work with zoomed views, the caller can apply
  the inverse viewport transform before passing positions to `BrushBehavior`.

### Development Workflow Insights

- The ZFS pool being nearly full (`96% capacity`) caused disk space issues during
  compilation and pre-commit hooks. Symlinking `target/` to `/tmp` on a
  different ZFS pool was an effective workaround.
- The `mask all-fix` markdown lint warnings are all pre-existing in other story
  files; the Rust formatting and lint checks passed cleanly for the changed
  files.
- GPU tests with `#[tokio::test]` work well for testing renderer creation and
  state management without needing a window or surface.

### Follow-up Stories

No new stories identified — the next logical step is GUP-286 (GPU Accelerated
Brush Region Query) which replaces the CPU-side `filter_by_rect` with a compute
shader, and depends on this story's overlay rendering being in place.
