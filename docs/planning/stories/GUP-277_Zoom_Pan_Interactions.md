# GUP-277: Zoom and Pan Interactions

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress
**Created**: 2025-07-14

## Context

Zoom and pan are the most fundamental navigational interactions for exploring
large datasets. D3.js's `d3-zoom` behavior is one of the most widely-used
components in its ecosystem precisely because navigation is a universal need:
scatter plots with millions of points, geographic maps, time-series charts, and
network graphs all require the user to zoom in for detail and pan to explore.

The naive approach — rebuilding the scene geometry on every frame as the user
moves — is prohibitively expensive at the data scales Gup targets (1M+ points).
This story avoids that by representing the view as a `ViewportTransform` uniform
(`scale_x`, `scale_y`, `translate_x`, `translate_y`) that is uploaded to the GPU
once per frame. The vertex shader applies this transform directly in clip space,
so the underlying geometry buffers are never touched during navigation. This
makes smooth 60 FPS panning and zooming tractable even for very large datasets.

GUP-012 delivered the foundational GPU interaction infrastructure, and GUP-013
is building the event dispatch layer (mouse wheel, drag). This story sits one
layer above both: it consumes raw wheel and drag events and translates them into
viewport transform state, then exposes a clean `ZoomBehavior` API inspired by
D3's design.

## User Story

> "As a visualization developer, I want to attach zoom and pan behaviour to a
> chart with a single API call so that my users can navigate large datasets
> smoothly at 60 FPS without any scene rebuild cost."
>
> "As an end user exploring a chart, I want panning to have natural inertia
> after I release the mouse so that the interaction feels fluid and responsive."

## Acceptance Criteria

### AC1: ZoomBehavior API

- [ ] `ZoomBehavior` is a public struct in the `gup` crate with a builder API
- [ ] `ZoomBehavior::new()` constructs a default behaviour with scale extent
      `(0.01, 10_000.0)` and no initial transform
- [ ] `.scale_extent(min: f64, max: f64)` constrains the zoom range; panicking
      if `min <= 0.0` or `min >= max`
- [ ] `.translate_extent(x0, y0, x1, y1)` optionally constrains the pan range to
      a world-space rectangle
- [ ] A chart can attach a `ZoomBehavior` via
      `chart.zoom(ZoomBehavior::new() .scale_extent(0.1, 100.0))` and remove it
      by passing `None`
- [ ] Attaching a `ZoomBehavior` twice replaces the previous one without
      panicking

### AC2: ViewportTransform Uniform

- [ ] A `ViewportTransform` struct (`scale_x: f32`, `scale_y: f32`,
      `translate_x: f32`, `translate_y: f32`) implements `bytemuck::Pod` +
      `bytemuck::Zeroable`
- [ ] The uniform is uploaded to a dedicated wgpu buffer bound at a stable bind
      group slot before each render pass
- [ ] The vertex shader for marks reads and applies `ViewportTransform` to
      clip-space positions
- [ ] When no zoom behaviour is attached the default transform is the identity
      (`scale_x = 1`, `scale_y = 1`, `translate_x = 0`, `translate_y = 0`)
- [ ] GPU validation layers emit no errors during zoom/pan interaction in the
      example

### AC3: Zoom to Cursor

- [ ] When a mouse-wheel event is handled, the chart computes the pointer
      position in data space and adjusts `translate_x`/`translate_y` so that the
      point under the cursor remains fixed after the scale change
- [ ] The behaviour is verified via a unit test that checks the
      `ViewportTransform` output for a known scroll event at a known pointer
      position

### AC4: Smooth Inertia Panning

- [ ] After a drag-release event the viewport continues to move with the release
      velocity, decaying exponentially until it falls below a minimum threshold
      (default `< 0.5 px/frame`)
- [ ] The decay coefficient is configurable via `.inertia_decay(alpha: f64)`
      where `0.0` means no inertia and `1.0` means no decay (clamped to
      `[0.0, 1.0)`)
- [ ] Initiating a new drag while inertia is active immediately cancels the
      inertia
- [ ] Inertia can be disabled entirely with `.inertia_decay(0.0)` — verified via
      unit test

### AC5: Scale and Translate Constraints

- [ ] The current scale is always clamped to `[scale_min, scale_max]` after
      every wheel event
- [ ] When a translate extent is set, `translate_x` and `translate_y` are
      clamped after every interaction event so the viewport cannot scroll
      outside the configured world-space rectangle
- [ ] Unit tests verify both clamp behaviours with boundary inputs

### AC6: Example and Documentation

- [ ] A new example `examples/zoom_pan.rs` demonstrates `ZoomBehavior` attached
      to a scatter plot with 500 000 data points
- [ ] The example compiles successfully with `cargo check --examples`
- [ ] Public API items carry `///` doc comments; `cargo doc --no-deps` produces
      no warnings for this module

## Technical Tasks

- [ ] Define `ViewportTransform` struct with `bytemuck` derives in
      `src/viewport.rs` (or appropriate module)
- [ ] Create and manage a `wgpu::Buffer` for the `ViewportTransform` uniform;
      add its bind group layout entry to the shared bind group used by all mark
      shaders
- [ ] Update the common vertex shader preamble (or per-mark WGSL) to apply
      `viewport_transform` to the output clip position
- [ ] Define `ZoomState` (internal): `scale: f64`, `translate: [f64; 2]`,
      `velocity: [f64; 2]`
- [ ] Define `ZoomBehavior` (public) with builder methods: `scale_extent`,
      `translate_extent`, `inertia_decay`
- [ ] Implement wheel-event handler: compute scale delta from `deltaY`, apply
      zoom-to-cursor arithmetic, clamp, write `ZoomState`
- [ ] Implement drag-start, drag-move, drag-end handlers: accumulate translation
      delta, record release velocity for inertia
- [ ] Implement per-frame inertia tick: multiply velocity by `(1 - decay)`, add
      to `translate`, stop when velocity magnitude is below threshold
- [ ] Wire `ZoomBehavior` into the event dispatch path provided by GUP-013
- [ ] Upload `ViewportTransform` (f32 downcast from f64 state) to GPU before
      each render pass
- [ ] Write unit tests for: zoom-to-cursor arithmetic, scale clamping, translate
      clamping, inertia decay, inertia cancellation
- [ ] Write `examples/zoom_pan.rs` with 500K point scatter, `ZoomBehavior`
      attached, on-screen scale readout in title bar

## Dependencies

### Prerequisite Stories

- GUP-004: Basic Render Context ✅ — provides `RenderContext` and wgpu
  device/queue
- GUP-012: GPU Interaction System ✅ — provides hit-test infrastructure and
  pointer-event delivery into the render loop
- GUP-013: Event Handling System 📋 — provides the `Selection::on()` event
  dispatch layer that `ZoomBehavior` hooks into for wheel and drag events

### Enables Stories

- GUP-275: Choropleth Chart Builder — geographic maps require zoom/pan to be
  usable at any meaningful data scale
- GUP-278: Brush Mark — the brush interaction must be aware of the current
  viewport transform so that brush extents are correctly mapped to data
  coordinates

## Testing Strategy

- **Unit tests**: Zoom-to-cursor arithmetic (fixed-point numeric tests),
  scale/translate clamping (boundary values), inertia velocity decay curve,
  inertia cancellation on drag start
- **Integration tests**: Attach `ZoomBehavior` to a minimal chart, simulate
  wheel and drag events, assert `ViewportTransform` buffer contents match
  expected values
- **Visual validation**: Run `examples/zoom_pan.rs` and visually confirm smooth
  navigation; GPU validation layers must report no errors
- **Performance**: Verify that 500K-point scatter achieves ≥ 60 FPS during
  continuous pan on the reference hardware documented in `perf-thresholds.toml`;
  frame time must not regress compared to a static (non-interactive) render of
  the same data

## Success Metrics

- [ ] `examples/zoom_pan.rs` renders 500 000 points at ≥ 60 FPS during
      continuous pan on reference hardware
- [ ] All unit tests for zoom-to-cursor, clamping, and inertia pass under
      `cargo test -- --test-threads=1`
- [ ] GPU validation layers emit zero errors or warnings during zoom/pan in the
      example
- [ ] `cargo doc --no-deps` emits zero warnings for the `zoom` / `viewport`
      modules
- [ ] `mask all-fix` reports a clean lint and format pass

## Risk Assessment

- **Medium**: WGSL bind group slot collision — adding a new `ViewportTransform`
  uniform bind group entry must not clash with existing bind group layouts used
  by mark shaders. A wrong binding index will silently produce incorrect
  rendering. _Mitigation_: Audit all existing `@binding(N)` assignments in WGSL
  shaders before choosing the slot; add an integration test that validates the
  rendered position of a known data point under a known transform.

- **Low**: `f64` → `f32` precision loss — zoom state is maintained in `f64` for
  numerical stability at extreme zoom levels, but the GPU uniform is `f32`. Loss
  of precision at very high zoom (> 10 000×) may produce visible jitter.
  _Mitigation_: Document the practical limit in `///` doc comments; consider a
  centre-relative transform decomposition if high-zoom use cases emerge.

- **Low**: GUP-013 not yet complete — this story depends on GUP-013's event
  dispatch layer. If GUP-013 is delayed, the event wiring cannot be completed.
  _Mitigation_: The `ViewportTransform` uniform plumbing and all arithmetic
  logic can be implemented and unit-tested independently of GUP-013; only the
  final integration step requires it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
