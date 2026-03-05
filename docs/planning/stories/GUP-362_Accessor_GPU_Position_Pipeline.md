# GUP-362: Accessor-to-GPU Position Pipeline

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-18
**Completed**: 2025-07-27

## Context

The `apply_accessors_to_selection` function in the chart builder pipeline
currently uses placeholder closures for position mapping — returning
`[0.0, 0.0]` for all data points. GUP-303 worked around this by appending
override attr bindings in `build_layer()`, but the proper fix is to connect the
accessor functions directly to GPU-side scale transformations. This would enable
scatter/bar charts built outside the composite builder to render data-driven
positions automatically.

## User Story

> "As a chart builder user, I want the x/y accessors I provide to
> ScatterPlotBuilder and BarChartBuilder to produce correctly positioned marks
> on screen without needing manual attr binding overrides."

## Acceptance Criteria

- [x] `apply_accessors_to_selection` evaluates the accessor functions against
      each data item and passes the resulting values to the position attr
      binding.
- [x] The x_scale and y_scale from ChartConfig are applied to map data positions
      to NDC.
- [x] ScatterPlotBuilder produces visible scatter points when built standalone
      (not via CompositeChartBuilder).
- [x] BarChartBuilder produces visible bars when built standalone.
- [x] Existing composite builder tests continue to pass.

## Technical Tasks

- [x] Refactor `apply_accessors_to_selection` to evaluate accessors on each data
      point and produce proper `AttrValue::Vec2` positions.
- [x] Integrate the x_scale/y_scale from ChartConfig to transform data positions
      to NDC during attr binding evaluation.
- [x] Remove or simplify the override-by-append workaround in composite
      `build_layer()` if it becomes redundant.
- [x] Add standalone scatter and bar rendering tests.

## Dependencies

### Prerequisite Stories

- GUP-251: Custom Composite Chart Support ✅
- GUP-303: Composite Chart GPU Render Pipeline ✅

## Testing Strategy

- Unit tests for `apply_accessors_to_selection` with real accessor functions.
- Integration tests rendering standalone scatter/bar charts.

## Risk Assessment

- **Low**: The accessor and scale infrastructure already exist; this is
  primarily a wiring task connecting existing components.

## Definition of Done

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

1. **`AxisScale::range_min()` / `range_max()` methods** — expose the output range
   bounds for all scale types (Linear, Log, Band, Point), enabling range-to-NDC
   conversion.

2. **Refactored `apply_accessors_to_selection`** — added a scale-aware code path:
   when `config.x_scale` and `config.y_scale` are both present, accessor values
   are mapped through `AxisScale::scale_value()` and then linearly converted from
   the scale's output range to NDC via `NdcBounds`. When scales are absent, the
   existing auto-domain + linear interpolation path is preserved.

3. **Bar-specific position and size bindings** — the `BarChartBuilder` now
   converts category strings to ordinal indices via `OrdinalScale`, computes bar
   center as the midpoint between baseline and bar top, and sets the `"size"`
   attribute with `[bandwidth, bar_height]` in NDC. Both vertical and horizontal
   orientations are handled.

4. **Simplified composite `build_layer()`** — removed the override-by-append
   workaround for scatter and bar layers. These overrides were already broken
   (due to `AccessorFunction::clone()` losing the closure) and are now redundant
   since `apply_accessors_to_selection` correctly integrates with scales. Line
   and area overrides are retained (they use segment-based data models).

5. **`range_to_ndc()` helper** — a small utility for linear range-to-NDC
   conversion, used by both the generic accessor pipeline and bar builder.

### Key Files Changed

| File | Change |
|------|--------|
| `src/chart_builder.rs` | Added `range_min()` / `range_max()` to `AxisScale` |
| `src/chart_builder/builders.rs` | Refactored `apply_accessors_to_selection`, added `range_to_ndc` |
| `src/chart_builder/builders/bar.rs` | Bar-specific center/size bindings with ordinal index mapping |
| `src/chart_builder/builders/composite.rs` | Removed scatter/bar override-by-append workaround |
| `tests/composite_chart_integration.rs` | Updated render-ready assertion |

### Test Counts

- **3075** lib tests pass (8 new)
- **15** composite integration tests pass
- All other integration tests pass
- All examples compile
