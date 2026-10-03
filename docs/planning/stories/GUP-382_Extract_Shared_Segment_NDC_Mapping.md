# GUP-382: Extract Shared Segment NDC Mapping Helper

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-27
**Completed**: 2026-10-04

## Context

GUP-364 integrated `scale_value()` → range → NDC mapping into both the line and
area builders. The mapping logic is nearly identical in both files: it clones
scales, computes range bounds, then applies a `scale_value → normalize → NDC`
pipeline in two attr closures (for "start" and "end"). Extracting this into a
shared helper would reduce duplication and make it easier to add new
segment-based mark types.

## User Story

> "As a chart builder maintainer, I want a reusable helper for mapping segment
> start/end positions through scales to NDC, so that new segment-based mark
> types don't duplicate the mapping logic."

## Acceptance Criteria

- [x] A shared function or struct handles segment position → NDC mapping for
      both line and area builders.
- [x] Line and area builders delegate to the shared helper.
- [x] All existing tests continue to pass.
- [x] Code duplication between line and area NDC mapping is eliminated.

## Technical Tasks

- [x] Design a `SegmentNdcMapper` helper (closure-based or struct-based) that
      encapsulates the two-path mapping logic (scales vs auto-domain).
- [x] Refactor line builder to use the helper.
- [x] Refactor area builder to use the helper.
- [x] Verify no regressions.

## Dependencies

### Prerequisite Stories

- GUP-364: Composite Line/Area Scale Integration ✅

## Testing Strategy

- Existing line/area render tests validate correctness.
- No new tests needed unless the helper API introduces new edge cases.

## Risk Assessment

- **Low**: Pure refactoring with no behaviour change.

## Definition of Done

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

A shared `SegmentNdcMapper` struct and `SegmentEndpoints` trait were added to
`src/chart_builder/builders.rs`:

- `SegmentEndpoints` exposes `start_pos()` / `end_pos()` for segment data items;
  implemented for `LineSegment<T>` and `AreaSegment<T>`.
- `SegmentNdcMapper` holds one private per-axis mapping (`AxisUnitMapping` enum:
  `Scaled` via `AxisScale::scale_value()` + output range, or `Linear` over a
  data domain) plus the chart's `NdcBounds`.
- Constructors: `scaled(x_scale, y_scale, ndc)`,
  `linear(x_domain, y_domain, ndc)`, and
  `from_scales_or_else(x_scale, y_scale, ndc, fallback_domain)`, which selects
  the scaled path only when both scales are present and evaluates the fallback
  domain lazily.
- `map([x, y]) -> [x, y]` maps a single position; `bind_positions(selection)`
  attaches the `start` and `end` attribute bindings to any
  `Selection<S: SegmentEndpoints, M: Mark>`.

`LineChartBuilder` and `AreaChartBuilder` now build a mapper with
`from_scales_or_else()` and call `bind_positions()`, replacing four
near-identical attr closures (~110–140 lines) in each builder. Each builder
keeps its own fallback-domain strategy (line: scale domain or padded raw-point
domain; area: padded domain over segment endpoints), passed in as a closure.

### Key Files Changed

| File                                 | Change                                                                  |
| ------------------------------------ | ----------------------------------------------------------------------- |
| `src/chart_builder/builders.rs`      | Added `SegmentEndpoints`, `SegmentNdcMapper`, `AxisUnitMapping` + tests |
| `src/chart_builder/builders/line.rs` | Delegates start/end NDC mapping to the helper; implements the trait     |
| `src/chart_builder/builders/area.rs` | Delegates start/end NDC mapping to the helper; implements the trait     |

### Test Results

- 7 new unit tests for `SegmentNdcMapper` (linear corners, custom chart-area
  bounds, degenerate spans, inverted pixel-range scales, log scale, path
  selection/lazy fallback, and `bind_positions` binding `start`/`end`).
- Full suite: 4,664 passed, 0 failed, 170 ignored (lib: 3,102 passed).
- `cargo check --examples` passes.
- Headless PNG renders of `area_chart_demo` and `line_chart_demo`
  (`GUP_SCREENSHOT_PATH`) are byte-identical before and after the refactor.
