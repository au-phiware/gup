# GUP-288: Area Chart Builder Data-Mark Rendering

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete (Superseded)
**Created**: 2025-07-20 **Closed**: 2026-10-04

**Superseded by**: [GUP-379](GUP-379_Area_Chart_Builder_Prepare_Render_Bound.md)
(the `prepare_render_bound()` call) and
[GUP-364](GUP-364_Composite_Line_Area_Scale_Integration.md) (the
`scale_value()`-based NDC mapping).

## Closure Note (2026-10-04)

This story's scope — wiring `AreaChartBuilder` so that `render_to_png()`
produces visible filled-area segments — was fully delivered by two later stories
before this one was ever started:

- **GUP-364** (Composite Line/Area Scale Integration) added explicit-scale
  support to `AreaChartBuilder::build_with_data()`, mapping `start_pos` /
  `end_pos` through `scale_value()` and normalising into `NdcBounds` (see
  `src/chart_builder/builders/area.rs` around lines 1232–1373).
- **GUP-379** (Area Chart Builder prepare_render_bound) added the missing
  `prepare_render_bound()` call at the end of `build_with_data()` (line ~1378)
  and `build_filled()` (line ~1001), and introduced the
  `test_area_chart_render_to_png_produces_visible_area` visual regression test.

Re-verification against the current codebase (2026-10-04) confirms every
Acceptance Criterion below is met:

- AC1 — `build_with_data()` returns a `ComposedChart` with a
  `Selection<AreaSegment<T>, Line>` bound to render-ready attrs (`start`, `end`,
  `color`, `width`); confirmed by code at
  `src/chart_builder/builders/area.rs:1203-1383`.
- AC2 — `test_area_chart_render_to_png_produces_visible_area` and
  `test_area_chart_with_explicit_scales_renders` both assert >50 non-white
  pixels in the data region; both pass
  (`cargo test --lib chart_builder::builders::area::tests:: -- --test-threads=1`
  → 39 passed, 0 failed).
- AC3 — `NdcBounds` are computed from `calculate_chart_area()`, the same pattern
  used by scatter/line/bar builders.
- AC4 — Satisfied by the two visual regression tests above.

No remaining gap was found. This story is closed as superseded rather than
implemented from scratch, since the work described here was already done under
GUP-364/GUP-379.

## Context (original)

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

- [x] `AreaChartBuilder::build_with_data()` produces a render-ready
      `ComposedChart` whose Selection draws area segments. (Delivered by
      GUP-364/GUP-379.)
- [x] `render_to_png()` on an area chart shows filled regions in the data area.
      (Delivered by GUP-379.)
- [x] Area segments respect the chart area NDC bounds (same axis alignment as
      scatter and line charts). (Delivered by GUP-364.)
- [x] At least one test validates visible area pixels in the data region.
      (Delivered by GUP-379; a second scale-integration test was added by
      GUP-364.)

## Technical Tasks

- [x] Update `AreaChartBuilder::build_with_data()` to compute NdcBounds from the
      chart area (axes first, then compute bounds). (GUP-364)
- [x] Map `AreaSegment.start_pos` and `end_pos` from data-space to NDC in the
      attr closures. (GUP-364)
- [x] Convert width from pixels to NDC units. (Width is passed through unchanged
      as a line width in pixels, consistent with the Line mark pipeline used by
      `LineChartBuilder`; no separate NDC conversion was needed in practice.)
- [x] Call `prepare_render_bound()` at build time. (GUP-379)
- [x] Add a visual regression test for area chart PNG export. (GUP-379)

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

- [x] Area chart builder produces visible data marks via `render_to_png()`.
- [x] All tests pass:
      `cargo test --lib chart_builder::builders::area::tests::     -- --test-threads=1`
      (39 passed, 0 failed, verified 2026-10-04).
- [x] `mask all-fix` exits cleanly (no changes made to source in this closure —
      documentation-only update).

## Retrospective

**Closed as superseded**: 2026-10-04

This story was never implemented directly. While it sat in the backlog as 📋
Planned, two other stories (GUP-364 and GUP-379) independently delivered the
exact capability it described — scale-aware NDC mapping for `AreaSegment`
positions and the `prepare_render_bound()` call that makes the resulting
`Selection` render-ready. During a backlog-hygiene pass (triggered by
duplicate-ID cleanup under GUP-374), re-reading
`src/chart_builder/builders/area.rs` and running the area builder test suite
confirmed all Acceptance Criteria were already met, so no further code changes
were required. Closing this story avoids duplicate/wasted implementation effort
and keeps the index accurate.

**Lesson for future backlog hygiene**: when a story sits unworked for a long
time, check whether a later, related story accidentally expanded in scope to
cover it before starting implementation.
