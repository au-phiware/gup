# GUP-272: iOS Chart Rendering Integration

## Story Overview

**Initiative**: Mobile **Status**: 📋 Planned **Created**: 2025-07-24

## Context

GUP-270 delivered the iOS platform foundation: Metal surface creation, the
`gup-ios` C ABI shim, the GupSwift Swift package, and the touch event
translation pipeline. However, the `gup_render_frame()` function currently
renders a clear frame — it does not yet wire chart builder output into the iOS
render path.

This story connects the chart builder system (from GUP-018) and the event
handling system (from GUP-013) to the iOS render loop, enabling actual data
visualisation on iPhone and iPad.

## User Story

> "As an iOS developer using Gup, I want to pass chart configuration and data to
> the iOS embedding layer so that my GupChartView actually renders a scatter
> plot (or other chart type) rather than a blank frame."

## Acceptance Criteria

- [ ] `gup_render_frame()` renders the configured chart (scatter, line, bar) on
      the attached Metal surface
- [ ] A new C ABI function `gup_configure_scatter()` (or equivalent) accepts
      point data and chart configuration
- [ ] The Swift wrapper exposes a `GupChartView.configure(data:)` method
- [ ] The iOS scatter example renders 10 000 random points at ≥ 30 fps
- [ ] Touch selection works end-to-end: tap → hit-test → highlight

## Dependencies

### Prerequisite Stories

- GUP-270: iOS Platform Support ✅ — provides the surface, shim, and Swift
  wrapper
- GUP-013: Event Handling System 📋 — provides the `.on()` dispatch API and
  `InteractionEvent` routing
- GUP-018: Chart Builders ✅ — provides `ScatterPlotBuilder` and chart
  configuration

## Testing Strategy

- Unit tests for chart configuration serialisation across FFI
- iOS Simulator integration test rendering a configured scatter plot
- Visual screenshot comparison against reference image

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] iOS scatter example renders actual data points
- [ ] Touch selection pipeline verified end-to-end
