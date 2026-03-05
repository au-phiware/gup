# GUP-286: Line Chart Builder Data-Mark Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-19 **Completed**: 2025-07-20

## Context

GUP-284 unified the scatter chart builder's data-mark rendering so that
`ComposedChart::render_to_png()` produces complete charts with visible circles.
However, the `LineChartBuilder` needs analogous treatment. The `Line` mark uses
a different vertex layout (line strips / connected segments) and requires a
different mapping strategy than the instanced-quad approach used for circles.

## User Story

> "As a Gup developer, I want `LineChartBuilder` charts to render visible line
> segments via `render_to_png()` without manually wiring up the Line mark
> pipeline."

## Acceptance Criteria

- [x] `LineChartBuilder::build_with_data()` produces a render-ready
      `ComposedChart` whose Selection draws line segments.
- [x] `render_to_png()` on a line chart shows connected line segments in the
      data region.
- [x] Line segments respect the chart area NDC bounds (same axis alignment as
      scatter charts).
- [x] At least one test validates visible line pixels in the data region.

## Technical Tasks

- [x] Update `LineChartBuilder::build_with_data()` to apply accessor-driven attr
      bindings using the `NdcBounds` approach from GUP-284.
- [x] Call `prepare_render_bound()` at build time for Line marks.
- [x] Add a visual regression test for line chart PNG export.

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

- [x] Line chart builder produces visible data marks via `render_to_png()`.
- [x] All tests pass: `cargo test -- --test-threads=1`.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### What Was Implemented

The `LineChartBuilder::build_with_data()` method was updated to produce
render-ready line charts by mapping data-space segment coordinates to NDC
chart-area coordinates and preparing the GPU pipeline at build time.

### Key Changes

| File                              | Change                                                                                                                                                                               |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/chart_builder/builders/line.rs` | Rewrote `build_with_data()`: compute data domain with 5% padding, create ComposedChart with axes first, compute NdcBounds, bind start/end/color/width attrs with NDC mapping, call `prepare_render_bound()`. Added `auto_domain_from_iter()` helper. Added 2 new tests. |

### Test Summary

- 2 new tests added (`test_line_chart_has_data_mark_data`,
  `test_line_chart_render_to_png_produces_visible_lines`)
- All 24 line chart tests pass
- All 3058 library tests pass
- All examples compile and produce correct output
- Visual verification: `03_line_chart` and `multi_series_line` examples export
  PNGs with visible line segments

## Retrospective

**Completed**: 2025-07-20

### Key Technical Learnings

#### Line Marks Require Different NDC Mapping Than Circles

- **Challenge**: The scatter chart's `apply_accessors_to_selection()` binds a
  single "center" attribute. Line marks need two position attributes ("start"
  and "end") mapped independently to NDC, so the shared helper could not be
  reused directly.
- **Solution**: Compute the data domain and NdcBounds inline, then bind separate
  "start" and "end" attr closures that each perform the domain→NDC linear
  mapping. The mapping logic is identical to the scatter chart's "center" mapping
  but applied independently to each endpoint.
- **Pattern**: When a mark type has different attribute names than the shared
  helper expects, inline the mapping rather than over-generalising the helper.
  Keep the shared helper focused on the most common case (Circle).

#### Width Units: Pixels vs NDC

- **Challenge**: `LineSegment.width` stores a user-facing pixel value (e.g. 2.0
  for "2px"). The Line mark vertex shader interprets width in clip-space units,
  where 2.0 would span the entire viewport.
- **Solution**: Convert pixel width to NDC with `width * 2.0 / config.width`.
  This gives consistent visual weight at the chart's configured logical width.
  The result scales naturally with render resolution (a 2px line at 800px
  logical width renders as 4px at 1600px resolution).
- **Pattern**: When mark shaders use clip-space units, convert user-facing pixel
  values at build time using the logical chart dimensions as the reference.

#### Build-Time Order: Axes Before NdcBounds

- **Challenge**: The chart area depends on axis margins. If NdcBounds is computed
  before axes are added, the data marks will be mis-positioned.
- **Solution**: Follow the same order established by GUP-284's scatter builder:
  (1) create ComposedChart with `.with_default_axes()`, (2) compute chart area,
  (3) compute NdcBounds, (4) bind attrs, (5) `prepare_render_bound()`.
- **Pattern**: Always add layout-affecting configuration (axes, titles, margins)
  before computing spatial mappings.

### Architectural Decisions

#### Inline Domain + NDC Mapping vs Extending apply_accessors_to_selection

- **Decision**: Implement the domain computation and NDC mapping directly in
  `LineChartBuilder::build_with_data()` rather than extending the shared
  `apply_accessors_to_selection()` to handle line marks.
- **Reasoning**: The line builder pre-computes segment endpoints from raw data
  before creating the Selection, so the mapping operates on `LineSegment.start_pos`
  and `LineSegment.end_pos` rather than accessor functions. Forcing this into the
  shared helper would require a different function signature.
- **Trade-off**: Slight duplication of the domain→NDC mapping formula. Acceptable
  because the formula is simple (4 lines) and the alternative would add
  complexity to the shared helper's API.
- **Future**: If more mark types need the same pattern, a `map_to_ndc(value,
  min, max, ndc_lo, ndc_hi)` utility could be extracted.

#### auto_domain_from_iter as a Private Helper

- **Decision**: Added `auto_domain_from_iter()` in the line module rather than
  extending the existing `auto_domain()` in builders.rs.
- **Reasoning**: The existing `auto_domain()` takes `&[T]` + `AccessorFunction`
  which doesn't fit here — the line builder has already evaluated accessors into
  `f32` values. A simple iterator-based helper avoids creating synthetic
  AccessorFunction wrappers.
- **Trade-off**: Two domain-padding functions exist in different modules.
- **Future**: Could unify into a single `auto_domain_from_iter()` that both call.

### Development Workflow Insights

- The change was remarkably small (~25 net lines of production code changed in
  `build_with_data()` plus a 20-line helper function) for the impact: all line
  chart builder outputs now render visible data marks.
- The pattern established by GUP-284 (axes first → chart area → NdcBounds →
  attr bindings → prepare_render_bound) transferred cleanly to the line mark
  case, validating the architectural approach.
- Visual verification via PNG export was essential. The test uses a 400×300
  render resolution and checks for >50 non-white pixels in the data region
  center, matching the scatter chart's test strategy.
- All 22 pre-existing line chart tests continued to pass without modification,
  confirming backward compatibility. The segment data (start_pos/end_pos in
  data space) is unchanged; only the GPU attr bindings now perform the NDC
  mapping.

### Follow-up Stories

1. **GUP-288: Area Chart Builder Data-Mark Rendering** — The
   `AreaChartBuilder` has the same gap: no NDC mapping, no
   `prepare_render_bound()`. It uses `Selection<AreaSegment<T>, Line>` and
   needs the same treatment applied here.

2. **GUP-289: Bar Chart Builder prepare_render_bound** — The `BarChartBuilder`
   computes NdcBounds and calls `apply_accessors_to_selection()` but does not
   call `prepare_render_bound()`, so bar chart data marks are not visible in
   `render_to_png()` output.
