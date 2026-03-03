# GUP-303: Composite Chart GPU Render Pipeline

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2026-03-03

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

- [ ] `CompositeChart::render()` records draw commands for every layer selection
      into a single render pass.
- [ ] Layers appear in declaration order (first layer at the bottom).
- [ ] A single set of axes and grid lines is drawn — not one per layer.
- [ ] The `composite_scatter_regression` and `composite_bar_trend` examples
      produce a visible window with all layers when run.
- [ ] No wgpu validation errors or panics.

## Technical Tasks

- [ ] Implement `CompositeChart::render(&mut RenderContext)` that iterates
      `additional_layers`, calling `Selection::render` or the equivalent draw
      method for each mark type.
- [ ] Ensure axis/tick/grid draw commands from the primary `ComposedChart` are
      issued once.
- [ ] Update examples to open a winit event loop and present the rendered
      surface.
- [ ] Write integration tests that verify all layers produce non-zero draw calls.

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

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
