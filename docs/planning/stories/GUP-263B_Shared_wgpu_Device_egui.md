# GUP-263B: Shared wgpu Device for egui

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
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

- [ ] `GupWidget` optionally accepts a shared `wgpu::Device` / `wgpu::Queue`
      from eframe's render state.
- [ ] When a shared device is provided, the chart texture is registered directly
      with egui's `TextureManager` (no pixel readback).
- [ ] The pixel-buffer fallback path is preserved for environments where device
      sharing is not available.
- [ ] No additional GPU device is created when using the shared path.
- [ ] All existing tests continue to pass.

## Technical Tasks

- [ ] Upgrade gup core crate to wgpu 27 (prerequisite — may be a separate
      story).
- [ ] Add `GupWidget::with_render_state(render_state: &egui_wgpu::RenderState)`
      constructor that extracts device/queue.
- [ ] Create `GupRenderContext` (like gup-bevy's) that wraps shared GPU
      resources.
- [ ] Register the off-screen texture directly as an egui texture ID.
- [ ] Preserve the fallback pixel-buffer path for backward compatibility.

## Dependencies

### Prerequisite Stories

- GUP-263: egui Integration ✅ — Established the pixel-buffer integration.
- wgpu 27 upgrade (may be captured in a separate story).

## Testing Strategy

- **Integration tests**: Verify rendering produces identical output via both
  shared-device and pixel-buffer paths.
- **Performance benchmark**: Measure frame time reduction from zero-copy path.

## Success Metrics

- [ ] No second GPU device created when using shared path.
- [ ] Frame-time overhead reduced compared to pixel-buffer path.
- [ ] All existing tests pass.

## Risk Assessment

- **Medium**: Depends on wgpu 27 upgrade which may have breaking API changes.
- **Low**: The shared-device pattern is proven in gup-bevy.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete
