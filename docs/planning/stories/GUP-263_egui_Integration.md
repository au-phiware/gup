# GUP-263: egui Integration

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2026-03-02

## Context

egui is the most widely-used immediate-mode GUI framework in the Rust ecosystem.
It is the basis for desktop applications, game tooling, and developer dashboards
across a broad range of Rust projects. Today, Gup has no first-class path for
embedding charts inside an egui application: developers must manage their own
wgpu device integration, texture upload bookkeeping, and input forwarding by
hand — a high barrier that prevents adoption wherever an egui UI is already in
place.

GUP-004 established `GupContext`, the core GPU render context, and GUP-018
delivered the Observable Plot-style chart builder API. These two foundations are
sufficient to render any Gup chart to a wgpu texture off-screen. The missing
piece is a thin adapter layer that bridges the wgpu texture into egui's own
texture system and routes egui pointer events back into Gup's interaction model.

The integration is delivered as a `gup-egui` crate (or a `egui` Cargo feature on
the main `gup` crate) that exports a single `GupWidget<C>` struct. The widget
owns a `GupContext`, renders the chart to an off-screen wgpu texture each frame
the chart data has changed, registers that texture with egui's `TextureManager`,
and displays it via `egui::Image`. Pointer events emitted by egui are translated
into Gup's internal event types and dispatched into the chart so that
interactive features (tooltips, zooming, selection) continue to work inside an
egui panel.

GUP-268 (PNG Export) will share the same render-to-texture path; completing this
story first will provide a validated off-screen rendering pipeline that GUP-268
can reuse directly.

## User Story

> "As a Rust developer building a desktop application with egui, I want to embed
> a live-updating Gup chart in an egui panel so that I can display
> GPU-accelerated visualizations without abandoning my existing egui UI."

> "As a Gup library author, I want a clean example showing Gup embedded inside
> egui so that new users understand how to integrate Gup with the most popular
> Rust GUI framework."

## Acceptance Criteria

### AC1: GupWidget Compiles and Renders

- [ ] A `GupWidget<C>` struct exists in the `gup-egui` crate (or behind a
      `features = ["egui"]` flag on the `gup` crate) where `C` is any type that
      implements the Gup chart trait produced by GUP-018.
- [ ] `GupWidget` implements `egui::Widget` so it can be passed directly to
      `ui.add(widget)`.
- [ ] The widget renders the chart to a wgpu off-screen texture and uploads it
      to egui's `TextureManager`, producing a correctly sized, visually correct
      chart image inside the egui panel.
- [ ] No GPU validation errors (wgpu validation layer) are emitted during normal
      rendering.

### AC2: Efficient Incremental Rendering

- [ ] The chart texture is only re-rendered when the chart data or panel size
      has changed; unchanged frames reuse the previously uploaded texture
      without issuing new GPU draw calls.
- [ ] A `dirty` / `mark_dirty()` API is exposed so host code can explicitly
      signal that data has changed and a re-render is required.
- [ ] Resizing the egui panel triggers a texture resize and re-render without
      panicking or leaking GPU resources.

### AC3: Interaction Bridge

- [ ] `egui::Response` pointer events (hover, click, drag) are translated into
      the Gup event types established by the event system and dispatched into
      the chart's event handler.
- [ ] Coordinate mapping correctly accounts for the widget's panel offset so
      that hover coordinates match the logical chart coordinate space.
- [ ] Events that egui does not consume (e.g. scroll inside the widget) are
      correctly forwarded rather than silently dropped.

### AC4: Example Application

- [ ] A runnable example at `examples/egui_chart.rs` (or `examples/egui/`) is
      included that: - Opens an egui window using `eframe`. - Renders a
      live-updating scatter plot (data changes each second) inside a `SidePanel`
      or `CentralPanel`. - Demonstrates zoom/hover interactivity within the
      panel.
- [ ] The example compiles cleanly: `cargo check --example egui_chart`.
- [ ] The example is documented with inline comments explaining the integration
      steps.

### AC5: Integration Guide

- [ ] A `docs/EGUI_INTEGRATION.md` guide covers: - Adding `gup-egui` (or the
      `egui` feature) to `Cargo.toml`. - Minimal code required to embed a
      chart. - How to push live data updates. - Known limitations (e.g. headless
      / software-renderer environments).

## Technical Tasks

- [ ] Decide crate layout: standalone `gup-egui` crate vs. `gup` feature flag.
      Prefer a separate crate to keep the dependency on `egui`/`eframe` out of
      the core `gup` crate.
- [ ] Add `gup-egui/Cargo.toml` with dependencies: `gup`, `egui`, `eframe`,
      `wgpu`.
- [ ] Implement off-screen render target in `GupWidget`: - Allocate a
      `wgpu::Texture` at the widget's current pixel dimensions. - Render the
      chart into it via `GupContext` each time `dirty` is set. - Upload the
      texture to egui using `egui::TextureManager::alloc` /
      `egui_wgpu::RenderState`.
- [ ] Implement `egui::Widget for GupWidget<C>`: - Call
      `ui.image(texture_id, size)` to display the rendered texture. - Collect
      `egui::Response` and translate pointer positions and click/drag events
      into Gup event types. - Dispatch translated events into the chart.
- [ ] Implement dirty-tracking: - `GupWidget::mark_dirty(&mut self)` sets an
      internal flag. - During `Widget::ui`, if dirty or size changed, re-render
      and upload before displaying.
- [ ] Handle panel resize: detect when `ui.available_size()` changes, recreate
      the wgpu texture at the new size, and re-render.
- [ ] Handle GPU device loss: propagate `GupContext` device-loss errors to the
      caller via a `Result`-returning helper rather than panicking.
- [ ] Write unit tests for coordinate mapping (egui panel offset → chart logical
      coordinates).
- [ ] Write integration test: construct a headless `GupWidget`, call
      `mark_dirty`, invoke the render path, and assert no GPU errors are raised.
- [ ] Write `examples/egui_chart.rs` with live-updating scatter plot.
- [ ] Write `docs/EGUI_INTEGRATION.md`.
- [ ] Add the new crate to the workspace `Cargo.toml` `members` list.

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides `GupContext`, the GPU device and
  queue abstraction used to render to the off-screen texture.
- GUP-018: Observable Plot Chart Builders ✅ — provides the chart API that
  `GupWidget<C>` wraps and renders.
- GUP-268: PNG Export 📋 — shares the off-screen render-to-texture path; this
  story can proceed in parallel but should align the texture render API with
  what GUP-268 needs to avoid duplicating infrastructure.

### Enables Stories

- GUP-268: PNG Export — the validated off-screen texture pipeline built here
  provides a reusable foundation for exporting charts to PNG.

## Testing Strategy

- **Unit tests**: coordinate-mapping logic (panel offset to chart space);
  dirty-flag transitions (clean → dirty → rendered → clean).
- **Integration tests**: headless render cycle — construct a `GupWidget`, call
  `mark_dirty()`, drive one render pass, assert no wgpu validation errors and
  that the returned texture handle is valid.
- **Visual validation**: run `cargo run --example egui_chart` and manually
  verify the scatter plot renders inside the egui panel and updates live.
- **Compilation check**: `cargo check --example egui_chart` must succeed in CI.

## Success Metrics

- [ ] `cargo check --example egui_chart` passes with no errors or warnings.
- [ ] All unit and integration tests pass:
      `cargo test -p gup-egui -- --test-threads=1`.
- [ ] No wgpu validation layer errors during the example run.
- [ ] The example visibly updates the scatter plot data at least once per
      second.
- [ ] `docs/EGUI_INTEGRATION.md` exists and covers the three integration steps
      (add dependency, construct widget, push updates).

## Risk Assessment

- **Medium**: egui's wgpu backend (`egui-wgpu`) requires sharing a
  `wgpu::Device` between egui and Gup. If `GupContext` does not expose its
  device/queue for external sharing, the integration will require a non-trivial
  refactor of the context construction API. _Mitigation_: Audit `GupContext`
  early; if sharing is not yet supported, add a
  `GupContext::from_device(device, queue, ...)` constructor as the first
  sub-task.

- **Medium**: egui texture IDs are invalidated when the egui context is
  recreated (e.g. on window resize in some backends). Stale texture IDs will
  cause rendering artifacts or panics. _Mitigation_: Store the texture ID inside
  `GupWidget` and re-register the texture whenever the widget detects the ID is
  no longer valid.

- **Low**: Headless CI environments may not have a GPU, making integration tests
  that exercise the full wgpu pipeline unreliable. _Mitigation_: Use wgpu's
  `Backends::all()` with fallback to the `wgpu` Vulkan software renderer
  (lavapipe) for CI, consistent with the approach used elsewhere in the test
  suite.

- **Low**: The egui/eframe version pinned by the workspace may diverge from the
  version required by host applications. _Mitigation_: Declare only a minimum
  compatible version in `gup-egui`'s `Cargo.toml` and document the tested
  version range in the integration guide.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-egui -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
