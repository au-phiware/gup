# GUP-362: Accessor-to-GPU Position Pipeline

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2025-07-18

## Context

The `apply_accessors_to_selection` function in the chart builder pipeline
currently uses placeholder closures for position mapping — returning `[0.0, 0.0]`
for all data points. GUP-303 worked around this by appending override attr
bindings in `build_layer()`, but the proper fix is to connect the accessor
functions directly to GPU-side scale transformations. This would enable
scatter/bar charts built outside the composite builder to render data-driven
positions automatically.

## User Story

> "As a chart builder user, I want the x/y accessors I provide to ScatterPlotBuilder
> and BarChartBuilder to produce correctly positioned marks on screen without needing
> manual attr binding overrides."

## Acceptance Criteria

- [ ] `apply_accessors_to_selection` evaluates the accessor functions against each
      data item and passes the resulting values to the position attr binding.
- [ ] The x_scale and y_scale from ChartConfig are applied to map data positions
      to NDC.
- [ ] ScatterPlotBuilder produces visible scatter points when built standalone
      (not via CompositeChartBuilder).
- [ ] BarChartBuilder produces visible bars when built standalone.
- [ ] Existing composite builder tests continue to pass.

## Technical Tasks

- [ ] Refactor `apply_accessors_to_selection` to evaluate accessors on each data
      point and produce proper `AttrValue::Vec2` positions.
- [ ] Integrate the x_scale/y_scale from ChartConfig to transform data positions
      to NDC during attr binding evaluation.
- [ ] Remove or simplify the override-by-append workaround in composite
      `build_layer()` if it becomes redundant.
- [ ] Add standalone scatter and bar rendering tests.

## Dependencies

### Prerequisite Stories

- GUP-251: Custom Composite Chart Support ✅
- GUP-303: Composite Chart GPU Render Pipeline ✅

## Testing Strategy

- Unit tests for `apply_accessors_to_selection` with real accessor functions.
- Integration tests rendering standalone scatter/bar charts.

## Risk Assessment

- **Low**: The accessor and scale infrastructure already exist; this is primarily
  a wiring task connecting existing components.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
