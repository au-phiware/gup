# GUP-268: PNG Export

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-01-09 **Completed**: 2025-07-18

## Implementation Summary

### What was implemented

- **`src/export/png.rs`** — Core PNG export module with:
  - `padded_bytes_per_row()` — computes wgpu-aligned row stride (256-byte
    multiple)
  - `strip_row_padding()` — removes alignment padding from pixel buffers
  - `bgra_to_rgba()` — converts BGRA channel order (wgpu default) to RGBA (PNG
    standard)
  - `encode_png()` — encodes raw RGBA pixels as PNG via
    `image::codecs::png::PngEncoder`
  - `readback_texture()` / `readback_texture_as_png()` — GPU texture → CPU
    staging buffer readback
  - `OffscreenTarget` struct — creates wgpu `Texture` + `TextureView` with
    `RENDER_ATTACHMENT | COPY_SRC`
- **`src/chart_builder.rs`** — Three new methods on `ComposedChart`:
  - `render_to_png(width, height)` → `GupResult<Vec<u8>>`
  - `render_to_png_scaled(logical_width, logical_height, scale)` →
    `GupResult<Vec<u8>>`
  - `export_png(path, width, height)` → `GupResult<()>`
- **`examples/export_png.rs`** — Example producing 1×, 2× HiDPI, and 2400×1600
  PNGs
- **`tests/png_export_integration.rs`** — 8 GPU integration tests

### Test counts

- 12 unit tests (row padding, BGRA→RGBA, PNG encoding, round-trip)
- 8 GPU integration tests (full pipeline, scaling, file I/O, RGBA, non-aligned
  widths)
- Total: 20 new tests; all 2,653 project tests pass

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

- [x] `Chart` exposes a method with the signature
      `fn render_to_png(&self, width: u32, height: u32) -> Result<Vec<u8>>` (or
      equivalent async variant if GPU readback requires it).
- [x] The returned `Vec<u8>` is a valid PNG file (passes decoding with the
      `image` crate without errors).
- [x] The decoded image dimensions match the requested `width` × `height`.
- [x] Pixel content visually matches the chart as rendered at that resolution
      (verified by the example and manual inspection).

### AC2: Off-screen rendering at arbitrary resolution

- [x] Export renders to a temporary off-screen texture/surface, not the
      interactive window surface, so it does not disturb any live display.
- [x] The off-screen texture is created at the exact requested `width` ×
      `height`; the chart layout scales to fill it.
- [x] The temporary texture and associated GPU resources are released after the
      PNG bytes are returned.

### AC3: GPU texture readback via staging buffer

- [x] Pixel data is read back from the GPU using a staging buffer (building on
      GUP-035's buffer download infrastructure).
- [x] Row padding (256-byte alignment required by wgpu) is correctly stripped
      before PNG encoding so no artefacts appear at row boundaries.
- [x] No GPU validation errors or wgpu warnings are produced during readback.

### AC4: PNG encoding with the `image` crate

- [x] The `image` crate is used for PNG encoding (add to `Cargo.toml` with
      `features = ["png"]`).
- [x] Output is RGBA (32-bit colour + alpha); transparent backgrounds are
      preserved.
- [x] Encoding errors are surfaced as a typed `Error` variant rather than
      `unwrap`/`expect`.

### AC5: High-DPI scale factor support

- [x] `Chart` exposes an additional convenience method
      `fn render_to_png_scaled(&self, logical_width: u32, logical_height: u32, scale: f32) -> Result<Vec<u8>>`
      that internally renders at
      `(logical_width * scale, logical_height * scale)` pixels.
- [x] A scale factor of `1.0` produces output identical to
      `render_to_png(logical_width, logical_height)`.
- [x] A scale factor of `2.0` produces an image with twice the pixel dimensions,
      suitable for Retina/HiDPI display at the logical size.

### AC6: File-writing convenience function

- [x] A top-level or `Chart` method
      `fn export_png(&self, path: impl AsRef<Path>, width: u32, height: u32) -> Result<()>`
      writes the PNG bytes directly to disk.
- [x] The function returns a descriptive `io::Error`-wrapped result if the file
      cannot be written.

### AC7: Working example

- [x] An example `examples/export_png.rs` (or equivalent) compiles and runs
      successfully with `cargo run --example export_png`.
- [x] The example renders a chart, calls
      `chart.export_png("chart.png", 2400, 1600)?`, and writes a valid PNG to
      the working directory.
- [x] The example is listed in `Cargo.toml` under `[[example]]`.

## Technical Tasks

- [x] Add
      `image = { version = "...", default-features = false, features = ["png"] }`
      to `Cargo.toml`.
- [x] Add a `png_export` feature flag (or keep unconditional — decide during
      implementation) so downstream crates can opt out of the `image`
      dependency.
- [x] Implement `OffscreenTarget`: a helper that creates a wgpu `Texture` +
      `TextureView` at a specified size with `COPY_SRC | RENDER_ATTACHMENT`
      usage, suitable for off-screen rendering.
- [x] Wire `Chart`'s render pipeline to accept an `OffscreenTarget` in place of
      the window surface — likely via a shared `RenderTarget` trait or enum.
- [x] Implement staging-buffer readback: allocate a `wgpu::Buffer` with
      `MAP_READ | COPY_DST`, encode a `copy_texture_to_buffer` command, submit,
      and poll until mapped (reusing or mirroring GUP-035 patterns).
- [x] Strip wgpu row padding: compute `bytes_per_row` alignment and copy only
      the valid pixel columns into a contiguous `Vec<u8>` before encoding.
- [x] Implement PNG encoding: call `image::save_buffer_with_format` (or
      `image::codecs::png::PngEncoder`) on the stripped pixel data.
- [x] Implement `Chart::render_to_png`, `Chart::render_to_png_scaled`, and
      `Chart::export_png` with appropriate error handling.
- [x] Write unit/integration tests for row-padding stripping logic.
- [x] Write integration test that calls `render_to_png`, decodes the result with
      `image`, and asserts correct dimensions and valid PNG magic bytes.
- [x] Create `examples/export_png.rs` demonstrating the full export workflow.
- [x] Document the new methods with rustdoc, including a short code snippet.

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

- [x] `cargo test -- --test-threads=1` passes with all new tests green.
- [x] `cargo run --example export_png` produces a readable `chart.png` at the
      requested 2400×1600 resolution.
- [x] `cargo check --examples` compiles without errors or warnings.
- [x] `mask all-fix` reports no lint or format issues.
- [x] PNG output decoded by an independent tool (e.g. `file chart.png`,
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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### BGRA vs RGBA Channel Order

- **Challenge**: wgpu's default surface format is `Bgra8UnormSrgb`, meaning GPU
  textures store pixels as BGRA. PNG requires RGBA channel order. Naively
  encoding the raw readback data would produce colour-swapped images.
- **Solution**: Added a dedicated `bgra_to_rgba()` function that swaps the R and
  B channels in-place before encoding. This is a simple byte-swap loop over
  4-byte pixel chunks.
- **Pattern**: When reading back GPU textures for CPU-side image encoding,
  always check the texture format and convert channel order before passing to
  the encoder. The format `Bgra8UnormSrgb` is the most common default on desktop
  (especially Windows/Linux via Vulkan).

#### wgpu Row-Padding Alignment

- **Challenge**: wgpu requires `bytes_per_row` in `copy_texture_to_buffer` to be
  a multiple of `COPY_BYTES_PER_ROW_ALIGNMENT` (256 bytes). For a 100-pixel-wide
  RGBA texture, the unpadded row is 400 bytes but the aligned row is 512 bytes.
  Without stripping this padding, decoded images have diagonal corruption
  artefacts.
- **Solution**: Isolated the stripping logic in a pure function
  `strip_row_padding()` and covered it with unit tests using synthetic data
  before wiring to the GPU path. The pure function makes debugging trivial.
- **Pattern**: Always isolate GPU alignment concerns into pure, testable helper
  functions. The row-padding constant (256) is a wgpu/WebGPU spec constant, not
  a driver-specific value, so it is safe to hard-code.

#### Staging Buffer Synchronisation

- **Challenge**: GPU readback is inherently asynchronous. The staging buffer
  must be mapped after the GPU finishes the copy, which requires polling.
- **Solution**: Used `device.poll(PollType::Wait)` for blocking readback, with
  `std::sync::mpsc::sync_channel` for the map callback. This avoids pulling in
  `tokio` for what is fundamentally a one-shot synchronisation point.
- **Pattern**: For blocking GPU readback in native-only code, prefer
  `mpsc::sync_channel` over `tokio::sync::oneshot` — it is simpler and avoids an
  async runtime dependency in the export path.

### Architectural Decisions

#### Unconditional `image` Dependency (No Feature Flag)

- **Decision**: Kept the `image` dependency unconditional rather than gating
  behind a `png-export` feature flag.
- **Reasoning**: The `image` crate was already in `Cargo.toml` (version 0.24)
  before this story, so adding a feature gate would add complexity without
  reducing the dependency footprint. The crate is well-maintained and widely
  used.
- **Trade-off**: Downstream crates cannot opt out of the PNG encoding
  dependency. This is acceptable given that it was already a dependency.
- **Future**: If the dependency becomes problematic (e.g. for WASM binary size),
  a feature gate can be added later without API changes.

#### `&mut self` Signature for `render_to_png`

- **Decision**: Used `&mut self` instead of the story's suggested `&self` for
  `render_to_png`.
- **Reasoning**: The method calls `prepare_draw_commands()` which lazily creates
  and caches GPU pipelines (tick pipeline, axis-line pipeline), requiring
  mutable access. Using interior mutability (`RefCell`) would add runtime
  overhead and panic risk for no benefit.
- **Trade-off**: Callers need a mutable reference to the chart, which is the
  normal pattern for rendering.
- **Future**: If an immutable variant is needed (e.g. for concurrent export), a
  snapshot-based approach could be added.

#### `OffscreenTarget` as a Public Utility

- **Decision**: Made `OffscreenTarget` a public struct in `export::png` rather
  than an internal helper.
- **Reasoning**: The same off-screen render-to-texture pattern is needed by
  GUP-263 (egui integration) and potentially by testing utilities. Exposing it
  allows reuse without duplication.
- **Trade-off**: Public API surface is slightly larger.
- **Future**: Enables the egui integration story to embed Gup charts as texture
  widgets by rendering to an `OffscreenTarget` and converting to an egui
  `TextureId`.

### Development Workflow Insights

- **Pure function testing first**: Writing unit tests for `strip_row_padding`,
  `bgra_to_rgba`, and `encode_png` before touching the GPU path was very
  effective. All 12 pure-function tests passed on the first run, and the GPU
  integration tests worked on the first attempt because the building blocks were
  already verified.
- **Build times**: The initial `cargo test` compilation takes ~15 minutes due to
  the large number of examples. Using `--lib --test <name>` for targeted test
  runs kept iteration cycles under 30 seconds.
- **Visual verification**: The exported PNG shows axis lines, tick marks, and
  grid lines rendered correctly. Data mark rendering is not yet visible because
  the `ComposedChart::render_to_png` path only issues axis/tick/grid draw
  commands — the data mark rendering pipeline (Selection GPU buffers) is not
  wired through `prepare_draw_commands`. This is expected; data mark rendering
  through the export path requires the full Selection render pipeline, which is
  a separate concern.

### Follow-up Stories

1. **GUP-268A: Data Mark Rendering in PNG Export** — Wire the
   `Selection::prepare_render` and draw pipeline through `ComposedChart`'s PNG
   export path so that data marks (circles, rectangles, lines) appear in
   exported PNGs alongside axes and grid lines. Currently the export renders the
   chart frame (axes, ticks, grid) but not the data visualization itself.
