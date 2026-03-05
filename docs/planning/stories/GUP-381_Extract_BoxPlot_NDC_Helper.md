# GUP-381: Extract shared NDC transformation helper for BoxPlot builders

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 🚧 In Progress **Created**:
2025-07-22

## Context

GUP-380 added NDC coordinate transformation to both `BoxPlotBuilder` and
`ViolinPlotBuilder`. The two builders share ~60 lines of identical logic:
computing data domain from `BoxPlotAttributes`, mapping positions, statistical
values, width, and outliers from data space to NDC coordinates. This duplication
should be extracted into a shared helper.

## User Story

> "As a Gup developer, I want a shared helper for transforming BoxPlotAttributes
> into NDC-space BoxPlotInstance values so that new BoxPlot-based builders don't
> need to duplicate the coordinate mapping logic."

## Acceptance Criteria

- [ ] A shared helper function exists that transforms `BoxPlotAttributes` into
      NDC-space `BoxPlotInstance` values given NDC bounds.
- [ ] `BoxPlotBuilder::build_with_data()` uses the shared helper.
- [ ] `ViolinPlotBuilder::build_with_data()` uses the shared helper.
- [ ] All existing tests continue to pass.

## Technical Tasks

- [ ] Extract the NDC domain computation (x/y min/max from BoxPlotAttributes
      slice) into a shared function.
- [ ] Extract the BoxPlotAttributes → BoxPlotInstance NDC mapper into a shared
      function or closure factory.
- [ ] Update both builders to use the shared functions.

## Dependencies

### Prerequisite Stories

- GUP-380 ✅ (Remaining Builders prepare_render_bound)

## Testing Strategy

- All existing boxplot and violin tests should continue to pass unchanged.
- No new tests needed — this is a pure refactor.

## Success Metrics

- Duplication between boxplot and violin builders reduced to a single call site.
- All existing tests pass.

## Risk Assessment

- **Very Low**: Pure refactoring with no behaviour change.

## Definition of Done

- [ ] Shared helper function extracts common logic.
- [ ] Both builders use the helper.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-fix` exits cleanly.
