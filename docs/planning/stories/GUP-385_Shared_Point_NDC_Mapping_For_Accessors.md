# GUP-385: Shared Point NDC Mapping for Accessor Pipeline

## Story Overview

**Initiative**: Chart Builders **Status**: ⏸ Parked **Created**: 2026-10-04

**Parked 2026-10-04**: pending strategic review tracks T2/T3/T5. See
[STRATEGIC_REVIEW_2026-10.md](../STRATEGIC_REVIEW_2026-10.md#backlog-triage).

## Context

GUP-382 introduced `SegmentNdcMapper` in `src/chart_builder/builders.rs`, which
models each axis as either a scaled mapping (`AxisScale::scale_value()` then
normalise over the scale's output range) or a linear mapping over a data domain,
and maps `[x, y]` positions into the chart's `NdcBounds`.

`apply_accessors_to_selection()` (used by scatter and bar builders for the
`center` attribute) still contains its own copy of the same two-path logic:
range bounds, span checks, unit normalisation and NDC interpolation, written out
separately for the scaled and auto-domain branches. This is the last remaining
duplicate of the data→NDC position pipeline in the builders module.

## User Story

> "As a chart builder maintainer, I want point-based and segment-based builders
> to share one data→NDC position mapper, so that scale handling fixes apply to
> every mark type at once."

## Acceptance Criteria

- [ ] A single position mapper type handles `[x, y]` data→NDC mapping for both
      point (`center`) and segment (`start`/`end`) bindings.
- [ ] `apply_accessors_to_selection()` delegates its position mapping to the
      shared mapper.
- [ ] Scatter and bar PNG output is unchanged (byte-identical headless renders
      before and after).
- [ ] All existing tests continue to pass.

## Technical Tasks

- [ ] Decide naming: generalise `SegmentNdcMapper` (e.g. `PositionNdcMapper`
      with a `bind_positions` method for segments and a `bind_center` style
      method or plain `map()` for points), keeping a type alias if useful.
- [ ] Refactor `apply_accessors_to_selection()` to construct the mapper via
      `from_scales_or_else()` with its existing fallback (scale domain or
      `auto_domain()` per axis).
- [ ] Consider moving NDC helpers (`NdcBounds`, mapper, `range_to_ndc`,
      `boxplot_ndc_mapper`) into a dedicated `ndc` submodule.
- [ ] Compare headless renders of scatter/bar examples before and after.

## Dependencies

### Prerequisite Stories

- GUP-362: Accessor GPU Position Pipeline ✅
- GUP-382: Extract Shared Segment NDC Mapping Helper ✅

## Testing Strategy

- Existing `apply_accessors_to_selection` tests and scatter/bar render tests.
- Before/after PNG `cmp` using `GUP_SCREENSHOT_PATH` on scatter and bar
  examples.

## Success Metrics

- One implementation of the data→NDC position pipeline in `builders.rs`.

## Risk Assessment

- **Low**: Behaviour-preserving refactor; preserving floating-point operation
  order keeps output byte-identical.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
