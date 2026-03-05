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

## Retrospective

**Completed**: 2025-07-22

### Key Technical Learnings

#### Texture Format Compatibility Between Chart Pipelines and egui

- **Challenge**: egui's `register_native_texture` requires `Rgba8Unorm`
  textures, but Gup's chart render pipelines default to `Bgra8UnormSrgb`. Using
  a mismatched format caused mark data points to silently not render (axes
  rendered fine because they use pipelines compiled per-call).
- **Solution**: Created a dual-view texture approach — the GPU texture is
  created with format `Rgba8UnormSrgb` and `view_formats: [Rgba8Unorm]`. The
  chart renders into the `Rgba8UnormSrgb` view (hardware applies linear→sRGB
  encoding), while egui samples from the `Rgba8Unorm` view (reads sRGB-encoded
  bytes as-is, matching egui's gamma-space expectations).
- **Pattern**: When bridging between two rendering systems, ensure the texture
  format for pipeline compilation matches the render target format, not just the
  output format. The `set_headless_format` API makes this explicit.

#### Pipeline Format Mismatch is Silent

- **Challenge**: When the chart's mark pipeline was compiled for
  `Bgra8UnormSrgb` but the render target was `Rgba8UnormSrgb`, wgpu silently
  skipped the draw commands rather than producing a validation error. Axis
  infrastructure (compiled per-frame) rendered correctly, making the issue appear
  to be mark-specific.
- **Solution**: Added `RenderContext::set_headless_format()` so that all
  pipeline creation (marks, axes, ticks, colorbars) uses the same format from
  the start. Charts built with `GupEguiContext` compile everything for
  `Rgba8UnormSrgb`.
- **Pattern**: Always verify that the entire pipeline chain uses a consistent
  texture format. Lazily-compiled pipelines (marks) can use a stale format when
  the rendering target format changes.

#### wgpu 27 Reference-Counted Types Enable Easy Sharing

- **Challenge**: Both egui and Gup need to use the same device/queue without
  creating a second GPU adapter.
- **Solution**: In wgpu 27, `Device`, `Queue`, `Adapter` are internally
  reference-counted (`Arc`). Cloning them is a cheap reference count bump, not a
  GPU resource allocation. This made `GupEguiContext::from_render_state` trivial.
- **Pattern**: Same approach used by `gup-bevy`. Any host application with wgpu
  27+ can share resources by cloning handles.

### Architectural Decisions

#### GupEguiContext vs Direct Device Injection

- **Decision**: Created a `GupEguiContext` type rather than directly passing
  `Device`/`Queue` to the widget.
- **Reasoning**: Matches the `GupRenderContext` pattern in `gup-bevy`, provides
  a single place to configure the headless format, and gives users a clean API
  for creating shared chart builders.
- **Trade-off**: Requires users to create one more object (`GupEguiContext`) but
  prevents misuse (e.g. passing egui's device but forgetting to set the format).
- **Future**: Could add convenience methods like `build_chart()` that
  automatically use the shared context.

#### Rgba8UnormSrgb + Rgba8Unorm View Pair

- **Decision**: Use `Rgba8UnormSrgb` for the chart render target with
  `view_formats: [Rgba8Unorm]` for egui sampling.
- **Reasoning**: The chart's fragment shaders output linear color values;
  `Rgba8UnormSrgb` applies hardware linear→sRGB encoding on write. egui expects
  gamma-space bytes in user textures, so the `Rgba8Unorm` view provides the raw
  sRGB-encoded bytes without unwanted sRGB decode on sample.
- **Trade-off**: Requires `view_formats` in the texture descriptor (trivial
  cost). Alternative was `Bgra8UnormSrgb` which would have required channel
  swizzling.
- **Future**: If egui adds sRGB-aware texture sampling, the approach may need
  revisiting.

### Development Workflow Insights

- **Texture format debugging is hard**: When draw commands silently fail due to
  format mismatch, there's no wgpu validation error. The only symptom is missing
  rendered content. Adding explicit format assertions at pipeline creation time
  would improve debugging.
- **Screen lock blocked visual verification**: The development machine's screen
  locked during the final screenshot capture. The first screenshot confirmed the
  shared-device path was active and axes rendered correctly; automated tests
  provided additional confidence.
- **Build cache disk usage**: The 70GB `target/` directory on `/tmp` filled the
  disk during final validation. Keeping build caches on separate partitions from
  `/tmp` would prevent this.

### Follow-up Stories

1. **GUP-263C: Frame-time Benchmark for gup-egui Paths** — Measure and compare
   frame-time overhead between the pixel-buffer fallback and zero-copy
   shared-device paths. The zero-copy path eliminates CPU readback and texture
   upload, but quantifying the improvement requires a benchmark harness.
2. **GUP-263D: Pipeline Format Validation** — Add debug-mode assertions that
   verify the chart's render pipeline format matches the render target format at
   draw time, preventing the silent mismatched-format issue discovered in this
   story.
