# GUP-263B: Shared wgpu Device for egui

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-07-22

## Context

GUP-263 delivered the egui integration using a pixel-buffer bridge between Gup's
wgpu 26 device and egui's wgpu 27. When the main `gup` crate upgrades to wgpu
27, both Gup and egui-wgpu will use the same wgpu version, enabling direct
device/queue sharing — the same pattern used by `gup-bevy`.

With shared GPU resources, the chart texture can be registered directly with
egui's `TextureManager` without any CPU-side pixel readback, enabling zero-copy
compositing.

## User Story

> "As a developer using gup-egui, I want the chart to render on the same GPU
> device as egui so that there is no redundant GPU device creation or CPU-side
> pixel transfer."

## Acceptance Criteria

- [x] `GupWidget` optionally accepts a shared `wgpu::Device` / `wgpu::Queue`
      from eframe's render state.
- [x] When a shared device is provided, the chart texture is registered directly
      with egui's `TextureManager` (no pixel readback).
- [x] The pixel-buffer fallback path is preserved for environments where device
      sharing is not available.
- [x] No additional GPU device is created when using the shared path.
- [x] All existing tests continue to pass.

## Technical Tasks

- [x] Upgrade gup core crate to wgpu 27 (prerequisite — completed in
      GUP-262B).
- [x] Add `GupWidget::with_render_state(render_state: &egui_wgpu::RenderState)`
      constructor that extracts device/queue.
- [x] Create `GupEguiContext` (like gup-bevy's `GupRenderContext`) that wraps
      shared GPU resources.
- [x] Register the off-screen texture directly as an egui texture ID.
- [x] Preserve the fallback pixel-buffer path for backward compatibility.

## Dependencies

### Prerequisite Stories

- GUP-263: egui Integration ✅ — Established the pixel-buffer integration.
- GUP-262B: Bevy 0.18 + wgpu 27 Upgrade ✅ — wgpu 27 already in place.

## Testing Strategy

- **Integration tests**: Verify rendering produces identical output via both
  shared-device and pixel-buffer paths.
- **Performance benchmark**: Measure frame time reduction from zero-copy path.

## Success Metrics

- [x] No second GPU device created when using shared path.
- [x] Frame-time overhead reduced compared to pixel-buffer path.
- [x] All existing tests pass.

## Risk Assessment

- **Medium**: Depends on wgpu 27 upgrade which may have breaking API changes.
- **Low**: The shared-device pattern is proven in gup-bevy.

## Definition of Done

- [x] All Acceptance Criteria satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete

## Implementation Summary

### What Was Implemented

1. **`GupEguiContext`** (`gup-egui/src/context.rs`): Wraps eframe's
   `RenderState` into a Gup `RenderContext` on the same GPU device/queue. Sets
   `headless_format` to `Rgba8UnormSrgb` so chart pipelines match the render
   target format.

2. **`GupWidget::with_render_state`** (`gup-egui/src/widget.rs`): New
   constructor that uses the zero-copy shared-device path. Creates an offscreen
   `Rgba8UnormSrgb` texture, registers a `Rgba8Unorm` view with egui's
   `Renderer`, and renders charts directly to GPU — no CPU readback.

3. **`RenderContext::set_headless_format`** (`src/render.rs`): New method to
   override the default surface format for headless/embedded rendering. Used by
   `GupEguiContext` to ensure all chart pipelines compile for `Rgba8UnormSrgb`.

4. **`DynChart::render_to_texture_view`**: New trait method for zero-copy
   rendering to an external texture view.

5. **`Drop` for `SharedDeviceState`**: Frees the registered egui texture on
   widget disposal.

6. **Updated `egui_chart` example**: Demonstrates the shared-device path with
   render path indicator in the UI sidebar.

### Key Files Changed

- `gup-egui/src/context.rs` — New: `GupEguiContext`
- `gup-egui/src/widget.rs` — `GupWidget::with_render_state`, shared rendering
- `gup-egui/src/lib.rs` — Re-exports
- `gup-egui/Cargo.toml` — Added `wgpu` dependency
- `gup-egui/examples/egui_chart.rs` — Updated to shared-device path
- `gup-egui/tests/widget_tests.rs` — 3 new tests
- `src/render.rs` — `headless_format` field and setter

### Test Counts

- 8 bridge tests (existing, passing)
- 9 widget tests (6 existing + 3 new, all passing)
- 5 doc-tests (ignored, require windowed context)
