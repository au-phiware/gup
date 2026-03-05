# GUP-288: Area Chart Builder Data-Mark Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 📋 Planned **Created**:
2025-07-20

## Context

GUP-286 wired the `LineChartBuilder` to produce render-ready line segments via
NDC mapping and `prepare_render_bound()`. The `AreaChartBuilder` uses the same
`Selection<AreaSegment<T>, Line>` pattern but still passes raw data-space
coordinates to the attr bindings and never calls `prepare_render_bound()`. As a
result, `render_to_png()` on an area chart produces no visible data marks.

## User Story

> "As a Gup developer, I want `AreaChartBuilder` charts to render visible
> filled-area segments via `render_to_png()` without manually wiring up the Line
> mark pipeline."

## Acceptance Criteria

- [ ] `AreaChartBuilder::build_with_data()` produces a render-ready
      `ComposedChart` whose Selection draws area segments.
- [ ] `render_to_png()` on an area chart shows filled regions in the data area.
- [ ] Area segments respect the chart area NDC bounds (same axis alignment as
      scatter and line charts).
- [ ] At least one test validates visible area pixels in the data region.

## Technical Tasks

- [ ] Update `AreaChartBuilder::build_with_data()` to compute NdcBounds from the
      chart area (axes first, then compute bounds).
- [ ] Map `AreaSegment.start_pos` and `end_pos` from data-space to NDC in the
      attr closures.
- [ ] Convert width from pixels to NDC units.
- [ ] Call `prepare_render_bound()` at build time.
- [ ] Add a visual regression test for area chart PNG export.

## Dependencies

### Prerequisite Stories

- GUP-286 ✅ (Line Chart Data-Mark Rendering)

## Testing Strategy

- Visual regression test: `render_to_png` for an area chart produces non-white
  pixels in the data region.

## Risk Assessment

- **Low**: The pattern is identical to GUP-286's line chart changes. The area
  chart uses `Selection<AreaSegment<T>, Line>` with the same "start", "end",
  "color", "width" attributes.

## Definition of Done

- [ ] Area chart builder produces visible data marks via `render_to_png()`.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
