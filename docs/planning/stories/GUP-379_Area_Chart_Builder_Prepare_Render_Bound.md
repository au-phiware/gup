# GUP-379: Area Chart Builder prepare_render_bound

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 📋 Planned **Created**:
2025-07-20

## Context

The `AreaChartBuilder::build_with_data()` computes `NdcBounds` and calls
`apply_accessors_to_selection()` to map data values to NDC chart-area
coordinates. However, like the bar chart builder before GUP-289, it does not
call `prepare_render_bound()`, so the Selection is never marked render-ready. As
a result, `render_to_png()` on an area chart may produce axes and grid but no
visible filled-area segments.

This is the same class of bug fixed in GUP-289 for the bar chart builder.

## User Story

> "As a Gup developer, I want `AreaChartBuilder` charts to render visible
> filled areas via `render_to_png()` without manually calling
> `prepare_render_bound()`."

## Acceptance Criteria

- [ ] `AreaChartBuilder::build_with_data()` calls `prepare_render_bound()` at
      build time so the Selection is render-ready.
- [ ] `render_to_png()` on an area chart shows visible filled segments in the
      data area.
- [ ] At least one test validates visible area pixels in the data region.

## Technical Tasks

- [ ] Clone `context` before passing to `Selection::new` in
      `AreaChartBuilder::build_with_data()`.
- [ ] Call `prepare_render_bound()` at the end of
      `AreaChartBuilder::build_with_data()` (after
      `apply_accessors_to_selection`).
- [ ] Add a visual regression test for area chart PNG export.

## Dependencies

### Prerequisite Stories

- GUP-287 ✅ (Area Chart Data Mark Rendering)
- GUP-289 ✅ (Bar Chart Builder prepare_render_bound — establishes the pattern)

## Testing Strategy

- Visual regression test: `render_to_png` for an area chart produces non-white
  pixels in the data region.

## Success Metrics

- Area chart `render_to_png()` produces visible filled-area marks.
- All existing area chart tests continue to pass.

## Risk Assessment

- **Very Low**: The fix is a single `prepare_render_bound()` call following the
  established pattern from GUP-289.

## Definition of Done

- [ ] Area chart builder produces visible data marks via `render_to_png()`.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
