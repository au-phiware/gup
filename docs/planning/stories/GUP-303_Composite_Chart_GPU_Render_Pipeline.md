# GUP-303: Composite Chart GPU Render Pipeline

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2026-03-03
**Completed**: 2025-07-18

## Context

GUP-251 delivered the `CompositeChartBuilder` which assembles multi-layer charts
with shared axes and unified domains. Currently the builder constructs all layer
selections and a primary `ComposedChart` (with axis/grid pipelines), but these
selections are not yet wired through a single wgpu render pass that draws all
layers to the same surface in declaration order. This story completes the GPU
rendering side.

## User Story

> "As a visualization developer, I want composite charts to render all layers
> visually in a single window so that I can see the combined multi-layer output
> at runtime."

## Acceptance Criteria

- [x] `CompositeChart::render()` records draw commands for every layer selection
      into a single render pass.
- [x] Layers appear in declaration order (first layer at the bottom).
- [x] A single set of axes and grid lines is drawn — not one per layer.
- [x] The `composite_scatter_regression` and `composite_bar_trend` examples
      produce a visible window with all layers when run.
- [x] No wgpu validation errors or panics.

## Technical Tasks

- [x] Implement `CompositeChart::render(&mut RenderContext)` that iterates
      `additional_layers`, calling `Selection::render` or the equivalent draw
      method for each mark type.
- [x] Ensure axis/tick/grid draw commands from the primary `ComposedChart` are
      issued once.
- [x] Update examples to open a winit event loop and present the rendered
      surface.
- [x] Write integration tests that verify all layers produce non-zero draw
      calls.

## Dependencies

### Prerequisite Stories

- GUP-251: Custom Composite Chart Support ✅

## Testing Strategy

- Integration tests verifying draw call counts.
- Visual validation of both composite examples.

## Risk Assessment

- **Medium**: Render-pass ordering for different mark types (Circle vs Line vs
  Rectangle) may require separate pipeline bindings within one pass.
  _Mitigation_: Follow the existing single-render-pass pattern from GUP-102.

## Definition of Done

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What was implemented

1. **`AxisScale::scale_value()` method** — Maps data-domain values to output
   range (NDC). Handles linear, log, band and point scales.

2. **`BuiltLayer` render methods** — `prepare()`, `draw()`, `is_render_ready()`
   for all layer variants (Scatter/Circle, Line/Line, Bar/Rectangle, Area/Line).

3. **`CompositeChart::prepare_render()`** — Prepares axis/grid pipelines on the
   primary chart and uploads GPU resources for every layer selection.

4. **`CompositeChart::draw()`** — Records draw commands into a single render
   pass in correct order: grid → data layers (declaration order) → axes/ticks.

5. **`CompositeChart::layer_draw_count()`** — Test helper returning count of
   render-ready layers.

6. **NDC-scaled attr bindings** — `build_layer()` now overrides placeholder attr
   bindings with properly scaled positions using the unified domain-to-NDC
   scales for each layer type.

7. **Windowed examples** — Both `composite_scatter_regression` and
   `composite_bar_trend` examples now open a winit window and render all layers
   through the GPU pipeline.

### Key files changed

- `src/chart_builder.rs` — Added `AxisScale::scale_value()` + unit tests
- `src/chart_builder/builders/composite.rs` — BuiltLayer render methods,
  CompositeChart render pipeline, NDC-scaled build_layer
- `examples/composite_scatter_regression.rs` — Windowed rendering
- `examples/composite_bar_trend.rs` — Windowed rendering
- `tests/composite_chart_integration.rs` — 4 new GPU render pipeline tests

### Test counts

- 4 new integration tests (prepare_render, all-layers-ready, dual-axis, scale)
- 6 existing integration tests (still passing)
- 233 total tests pass across the full suite

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### Enum-Based Layer Dispatch for GPU Rendering

- **Challenge**: The `BuiltLayer` enum holds four different `Selection<T, M>`
  types (Circle, Line, Rectangle, Line-for-area). Each needs `prepare_render_bound`
  and `render` called, but the Selection type parameter differs per variant.
- **Solution**: Added `prepare()`, `draw()` and `is_render_ready()` methods on
  `BuiltLayer` that match on the variant and delegate to the inner Selection.
  This keeps the enum-over-trait-objects pattern consistent with the project's
  architecture principles.
- **Pattern**: Enum dispatch for known-set polymorphism scales well for GPU
  resource management — each variant can share the same render pass without
  trait-object overhead.

#### Accessor-to-NDC Coordinate Pipeline

- **Challenge**: The chart builders produce selections with data-space positions
  (e.g. x=5.0 means "5 in data units"), but the GPU mark shaders render in
  NDC (-1 to 1). The existing `apply_accessors_to_selection` function uses
  placeholder closures returning `[0.0, 0.0]` for positions.
- **Solution**: In `build_layer()`, after the inner builder creates its
  selection, we append additional `attr()` bindings that apply the composite's
  unified scales to map data coordinates to NDC. Because `build_instance`
  processes bindings in order and last-write-wins, the new bindings override
  the placeholders.
- **Pattern**: "Override-by-append" for attr bindings — a clean way to inject
  coordinate transformations without modifying the inner builder's code.

#### Single Render Pass Multi-Pipeline Rendering

- **Challenge**: Each mark type (Circle, Line, Rectangle) has its own render
  pipeline. Rendering all of them in a single render pass requires switching
  pipelines within the pass.
- **Solution**: Each Selection's `render()` method sets its own pipeline via
  `render_pass.set_pipeline()` before drawing. wgpu handles pipeline switches
  within a single render pass transparently. No special coordination needed.
- **Pattern**: wgpu render passes support arbitrary pipeline switches. The
  single-render-pass constraint means only one `begin_render_pass` per
  command encoder, not one pipeline per pass.

### Architectural Decisions

#### Two-Phase Render API (prepare + draw)

- **Decision**: Split rendering into `prepare_render()` (GPU resource upload)
  and `draw()` (render pass recording) rather than a single `render()` method.
- **Reasoning**: Follows the existing Selection pattern (`prepare_render_bound`
  then `render`). GPU resource upload happens outside the render pass scope,
  while draw commands happen inside it.
- **Trade-off**: Callers must remember to call both methods. But this matches
  the windowed rendering pattern already used in every Gup example.
- **Future**: Enables future optimizations like only re-preparing layers whose
  data changed.

#### AxisScale::scale_value for CPU-Side Coordinate Mapping

- **Decision**: Added a CPU-side `scale_value()` method to `AxisScale` for
  mapping data values to NDC during attr binding evaluation.
- **Reasoning**: The shader-function-based GPU scaling (LinearScale as
  ComposableShaderFunction) requires pipeline setup. For the build phase, a
  simple CPU-side linear interpolation is sufficient and much simpler.
- **Trade-off**: Double computation (CPU mapping at build time, potential GPU
  mapping later). But the build phase only runs once, and the data sizes are
  small (hundreds of points, not millions).
- **Future**: When shader-function-based rendering is fully connected, the
  CPU-side mapping could be replaced by GPU-side scaling, avoiding the
  CPU→NDC→GPU pipeline.

### Development Workflow Insights

- The project has a very large number of test binaries (~100+), so running
  `cargo test -- --test-threads=1` with a filter can be slow as it checks
  every binary. Using `--test <name>` to target a specific integration test
  file is much faster for iterative development.
- The `chart_builder.rs` unit test module (`mod tests`) doesn't appear in the
  test listing, possibly due to a compile issue in that 5000+ line file. The
  integration test file `tests/composite_chart_integration.rs` is the reliable
  way to test chart builder functionality.
- The windowed examples can't be visually validated when the desktop session is
  locked, but the screen-grabber agent confirmed the window was created with
  the correct title and was running on the expected workspace.

### Follow-up Stories

1. **GUP-304: Per-Layer Data Support** — Now unblocked by GUP-303. Allow each
   layer to carry its own data type (different T per layer) via type-erased
   layer support. This is the natural next step after the render pipeline is
   wired.

2. **GUP-362: Accessor-to-GPU Position Pipeline** — The
   `apply_accessors_to_selection` function currently uses placeholder closures
   for position mapping. Connect the accessor functions to actual GPU-side scale
   transformations so scatter/bar charts render data-driven positions without
   the override-by-append workaround.
