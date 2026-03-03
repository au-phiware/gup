# GUP-268: PNG Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-01-09

## Context

Gup renders visualisations entirely on the GPU, which means every visual effect
— transparency, blend modes, SDF text, custom mark shaders — exists only as
pixels on a GPU texture. Capturing that output faithfully requires reading those
pixels back to the CPU and encoding them as a raster image. PNG is the natural
first format: lossless, widely supported, and the de facto standard for chart
exports in reporting, documentation, and publishing workflows.

Unlike SVG export (which must reconstruct vector paths from GPU geometry), PNG
export is architecturally simpler: render to an off-screen texture at the
desired resolution, read the pixel data back via a staging buffer, and encode it
with a standard PNG encoder. The GPU texture readback mechanism already exists
in GUP-035's Advanced Buffer Download System, so this story primarily concerns
wiring that capability into a clean, resolution-aware export API on `Chart`.

High-DPI ("Retina") output is an important consideration: callers should be able
to specify a device-pixel scale factor so that charts exported for print or
high-density screens are crisp rather than blurry when displayed at their
intended physical size.

## User Story

> "As a visualisation developer, I want to export a Gup chart as a PNG file at
> an arbitrary resolution so that I can embed pixel-perfect chart images in
> reports, documentation, and web pages without loss of GPU-rendered effects."

> "As a visualisation developer, I want to specify a scale factor for HiDPI
> export so that charts rendered for print or Retina displays are sharp at their
> intended display size."

## Acceptance Criteria

### AC1: `Chart::render_to_png` returns raw PNG bytes

- [ ] `Chart` exposes a method with the signature
      `fn render_to_png(&self, width: u32, height: u32) -> Result<Vec<u8>>` (or
      equivalent async variant if GPU readback requires it).
- [ ] The returned `Vec<u8>` is a valid PNG file (passes decoding with the
      `image` crate without errors).
- [ ] The decoded image dimensions match the requested `width` × `height`.
- [ ] Pixel content visually matches the chart as rendered at that resolution
      (verified by the example and manual inspection).

### AC2: Off-screen rendering at arbitrary resolution

- [ ] Export renders to a temporary off-screen texture/surface, not the
      interactive window surface, so it does not disturb any live display.
- [ ] The off-screen texture is created at the exact requested `width` ×
      `height`; the chart layout scales to fill it.
- [ ] The temporary texture and associated GPU resources are released after the
      PNG bytes are returned.

### AC3: GPU texture readback via staging buffer

- [ ] Pixel data is read back from the GPU using a staging buffer (building on
      GUP-035's buffer download infrastructure).
- [ ] Row padding (256-byte alignment required by wgpu) is correctly stripped
      before PNG encoding so no artefacts appear at row boundaries.
- [ ] No GPU validation errors or wgpu warnings are produced during readback.

### AC4: PNG encoding with the `image` crate

- [ ] The `image` crate is used for PNG encoding (add to `Cargo.toml` with
      `features = ["png"]`).
- [ ] Output is RGBA (32-bit colour + alpha); transparent backgrounds are
      preserved.
- [ ] Encoding errors are surfaced as a typed `Error` variant rather than
      `unwrap`/`expect`.

### AC5: High-DPI scale factor support

- [ ] `Chart` exposes an additional convenience method
      `fn render_to_png_scaled(&self, logical_width: u32, logical_height: u32, scale: f32) -> Result<Vec<u8>>`
      that internally renders at
      `(logical_width * scale, logical_height * scale)` pixels.
- [ ] A scale factor of `1.0` produces output identical to
      `render_to_png(logical_width, logical_height)`.
- [ ] A scale factor of `2.0` produces an image with twice the pixel dimensions,
      suitable for Retina/HiDPI display at the logical size.

### AC6: File-writing convenience function

- [ ] A top-level or `Chart` method
      `fn export_png(&self, path: impl AsRef<Path>, width: u32, height: u32) -> Result<()>`
      writes the PNG bytes directly to disk.
- [ ] The function returns a descriptive `io::Error`-wrapped result if the file
      cannot be written.

### AC7: Working example

- [ ] An example `examples/export_png.rs` (or equivalent) compiles and runs
      successfully with `cargo run --example export_png`.
- [ ] The example renders a chart, calls
      `chart.export_png("chart.png", 2400, 1600)?`, and writes a valid PNG to
      the working directory.
- [ ] The example is listed in `Cargo.toml` under `[[example]]`.

## Technical Tasks

- [ ] Add
      `image = { version = "...", default-features = false, features = ["png"] }`
      to `Cargo.toml`.
- [ ] Add a `png_export` feature flag (or keep unconditional — decide during
      implementation) so downstream crates can opt out of the `image`
      dependency.
- [ ] Implement `OffscreenTarget`: a helper that creates a wgpu `Texture` +
      `TextureView` at a specified size with `COPY_SRC | RENDER_ATTACHMENT`
      usage, suitable for off-screen rendering.
- [ ] Wire `Chart`'s render pipeline to accept an `OffscreenTarget` in place of
      the window surface — likely via a shared `RenderTarget` trait or enum.
- [ ] Implement staging-buffer readback: allocate a `wgpu::Buffer` with
      `MAP_READ | COPY_DST`, encode a `copy_texture_to_buffer` command, submit,
      and poll until mapped (reusing or mirroring GUP-035 patterns).
- [ ] Strip wgpu row padding: compute `bytes_per_row` alignment and copy only
      the valid pixel columns into a contiguous `Vec<u8>` before encoding.
- [ ] Implement PNG encoding: call `image::save_buffer_with_format` (or
      `image::codecs::png::PngEncoder`) on the stripped pixel data.
- [ ] Implement `Chart::render_to_png`, `Chart::render_to_png_scaled`, and
      `Chart::export_png` with appropriate error handling.
- [ ] Write unit/integration tests for row-padding stripping logic.
- [ ] Write integration test that calls `render_to_png`, decodes the result with
      `image`, and asserts correct dimensions and valid PNG magic bytes.
- [ ] Create `examples/export_png.rs` demonstrating the full export workflow.
- [ ] Document the new methods with rustdoc, including a short code snippet.

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides the wgpu `Device`, `Queue`, and
  rendering primitives that off-screen rendering builds on.
- GUP-035: Advanced Buffer Download System ✅ — GPU staging-buffer readback
  patterns used directly by PNG export.
- GUP-018: Observable Plot-Style Chart Builders ✅ — `Chart` API that
  `render_to_png` is added to.

### Enables Stories

- GUP-263: egui Integration — render-to-texture is the same off-screen mechanism
  used to embed Gup charts as egui image widgets.
- GUP-269: HTML Export — HTML export can inline the PNG produced by this story
  as a `<img src="data:image/png;base64,...">` element.

## Testing Strategy

- **Unit tests**: Row-padding stripping logic tested with synthetic byte
  sequences at various widths and `bytes_per_row` alignments.
- **Integration tests**: `render_to_png(800, 600)` produces a `Vec<u8>` whose
  first 8 bytes match the PNG magic signature
  `[137, 80, 78, 71, 13, 10, 26, 10]` and whose IHDR chunk reports width=800,
  height=600.
- **Visual validation**: Run `cargo run --example export_png` and open the
  written `chart.png` to visually confirm it matches the on-screen rendering.
- **HiDPI test**: `render_to_png_scaled(400, 300, 2.0)` produces a 800×600 image
  (same pixel count as the base integration test above).
- **GPU validation**: Ensure `WGPU_VALIDATION=1` (or the test harness
  equivalent) reports no validation errors during off-screen rendering and
  readback.
- **No resource leaks**: Verify (via `wgpu` debug layers or manual inspection)
  that the temporary `OffscreenTarget` texture is dropped after each export
  call.

## Success Metrics

- [ ] `cargo test -- --test-threads=1` passes with all new tests green.
- [ ] `cargo run --example export_png` produces a readable `chart.png` at the
      requested 2400×1600 resolution.
- [ ] `cargo check --examples` compiles without errors or warnings.
- [ ] `mask all-fix` reports no lint or format issues.
- [ ] PNG output decoded by an independent tool (e.g. `file chart.png`,
      `identify chart.png`) confirms correct dimensions and bit depth.

## Risk Assessment

- **Medium**: wgpu `bytes_per_row` alignment (must be a multiple of 256) means
  the readback buffer is wider than the texture in pixel data. Incorrect
  stripping produces corrupted rows. _Mitigation_: Isolate stripping logic in a
  pure function and cover it with unit tests using synthetic data before wiring
  to real GPU readback.

- **Medium**: Off-screen rendering requires the chart's render pipeline to
  target a `TextureView` rather than a swap-chain surface. If the pipeline is
  tightly coupled to the window surface type, refactoring may be larger than
  expected. _Mitigation_: Introduce a `RenderTarget` abstraction early; if
  coupling is deep, treat it as a prerequisite sub-task and keep it focused.

- **Low**: The `image` crate adds a compile-time dependency. Some downstream
  users may not need PNG export. _Mitigation_: Gate behind a `png-export` Cargo
  feature so it is opt-in; enable it by default so the out-of-the-box experience
  includes export capability.

- **Low**: Async GPU polling behaviour differs between native (wgpu `poll`) and
  WASM (`requestAnimationFrame`). PNG export is primarily a native / server-side
  feature, but if WASM support is required later, the readback approach must be
  revisited. _Mitigation_: Clearly document that `render_to_png` is a
  blocking/native-only API for now; WASM support can be added as a follow-up.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
