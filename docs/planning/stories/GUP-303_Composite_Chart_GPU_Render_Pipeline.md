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
