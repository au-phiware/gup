# GUP-384: Area Builder Stroke Width NDC Conversion

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2026-10-04

## Context

While verifying GUP-382 with a headless render of `area_chart_demo`, the
single-series area chart rendered as large fan-shaped stripes covering most of
the canvas instead of an outlined area. The output is identical before and after
GUP-382, so the bug is pre-existing.

Root cause: `AreaChartBuilder::build_with_data()` binds the Line mark's `width`
attribute directly to `AreaSegment::width` (1.5, intended as logical pixels):

```rust
selection.attr("width", |seg: &AreaSegment<T>| seg.width);
```

The Line mark interprets `width` in NDC units, so each stroke is 1.5 NDC
(roughly 75% of the viewport) wide. `LineChartBuilder` already converts pixels
to NDC with `seg.width * (2.0 / chart_width)`. A throwaway experiment applying
the same conversion in the area builder produced a clean area outline.

The existing GUP-379 regression test only asserts that non-white pixels appear
in the data region, which the bug satisfies, so it went unnoticed.

## User Story

> "As a chart author, I want `area()` charts built with `build_with_data()` to
> render thin outline strokes at the configured pixel width, so that the chart
> shows the data shape rather than overlapping wide stripes."

## Acceptance Criteria

- [ ] `AreaChartBuilder::build_with_data()` converts segment stroke width from
      logical pixels to NDC, matching `LineChartBuilder`.
- [ ] The pixel→NDC width conversion is shared between the line and area
      builders (e.g. a helper next to `SegmentNdcMapper`), not duplicated.
- [ ] A regression test fails on the current behaviour: e.g. pixels well outside
      the area polygon (such as the top-left corner of the data region above the
      curve) remain background-coloured.
- [ ] `area_chart_demo` headless render shows the area outline without stripes.

## Technical Tasks

- [ ] Move the `width` binding in `area.rs` after the `ComposedChart` is built
      so the chart width is available, and apply the conversion.
- [ ] Add a shared helper (e.g.
      `SegmentNdcMapper::stroke_width_to_ndc(px,     chart_width)` or a
      `bind_width` method) and use it from both builders.
- [ ] Add a pixel-level regression test in `area.rs` tests.
- [ ] Re-render `area_chart_demo` via `GUP_SCREENSHOT_PATH` and inspect.

## Dependencies

### Prerequisite Stories

- GUP-379: Area Chart Builder prepare_render_bound ✅
- GUP-382: Extract Shared Segment NDC Mapping Helper ✅

## Testing Strategy

- Unit test for the width conversion helper.
- GPU pixel test on `render_to_png()` asserting background pixels outside the
  polygon and stroke pixels on the outline.
- Visual check of `area_chart_demo` output.

## Success Metrics

- Area chart strokes are ~1.5 px wide in exported PNGs.
- No change to line chart output (byte-identical PNG before/after).

## Risk Assessment

- **Low**: Single-attribute fix with an established pattern in the line builder.
  Gallery thumbnails for area charts will change (intended).

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
