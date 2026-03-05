# GUP-286: Line Chart Builder Data-Mark Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 🚧 In Progress **Created**:
2025-07-19

## Context

GUP-284 unified the scatter chart builder's data-mark rendering so that
`ComposedChart::render_to_png()` produces complete charts with visible circles.
However, the `LineChartBuilder` needs analogous treatment. The `Line` mark uses a
different vertex layout (line strips / connected segments) and requires a
different mapping strategy than the instanced-quad approach used for circles.

## User Story

> "As a Gup developer, I want `LineChartBuilder` charts to render visible line
> segments via `render_to_png()` without manually wiring up the Line mark
> pipeline."

## Acceptance Criteria

- [ ] `LineChartBuilder::build_with_data()` produces a render-ready
      `ComposedChart` whose Selection draws line segments.
- [ ] `render_to_png()` on a line chart shows connected line segments in the data
      region.
- [ ] Line segments respect the chart area NDC bounds (same axis alignment as
      scatter charts).
- [ ] At least one test validates visible line pixels in the data region.

## Technical Tasks

- [ ] Update `LineChartBuilder::build_with_data()` to apply accessor-driven attr
      bindings using the `NdcBounds` approach from GUP-284.
- [ ] Call `prepare_render_bound()` at build time for Line marks.
- [ ] Add a visual regression test for line chart PNG export.

## Dependencies

### Prerequisite Stories

- GUP-284 ✅ (Unify Chart Builder Data Layer)

## Testing Strategy

- Visual regression test: `render_to_png` for a line chart produces non-white
  pixels in the data region forming connected segments.

## Risk Assessment

- **Low**: The `Line` mark already implements `MarkInstanceBuilder`. The main
  work is adapting the accessor-to-NDC mapping for line segment endpoints rather
  than circle centres.

## Definition of Done

- [ ] Line chart builder produces visible data marks via `render_to_png()`.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
