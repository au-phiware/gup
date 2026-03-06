# GUP-382: Extract Shared Segment NDC Mapping Helper

## Story Overview

**Initiative**: Chart Builders **Status**: 💡 New **Created**: 2025-07-27

## Context

GUP-364 integrated `scale_value()` → range → NDC mapping into both the
line and area builders. The mapping logic is nearly identical in both
files: it clones scales, computes range bounds, then applies a
`scale_value → normalize → NDC` pipeline in two attr closures (for
"start" and "end"). Extracting this into a shared helper would reduce
duplication and make it easier to add new segment-based mark types.

## User Story

> "As a chart builder maintainer, I want a reusable helper for mapping
> segment start/end positions through scales to NDC, so that new
> segment-based mark types don't duplicate the mapping logic."

## Acceptance Criteria

- [ ] A shared function or struct handles segment position → NDC mapping
      for both line and area builders.
- [ ] Line and area builders delegate to the shared helper.
- [ ] All existing tests continue to pass.
- [ ] Code duplication between line and area NDC mapping is eliminated.

## Technical Tasks

- [ ] Design a `SegmentNdcMapper` helper (closure-based or struct-based)
      that encapsulates the two-path mapping logic (scales vs auto-domain).
- [ ] Refactor line builder to use the helper.
- [ ] Refactor area builder to use the helper.
- [ ] Verify no regressions.

## Dependencies

### Prerequisite Stories

- GUP-364: Composite Line/Area Scale Integration ✅

## Testing Strategy

- Existing line/area render tests validate correctness.
- No new tests needed unless the helper API introduces new edge cases.

## Risk Assessment

- **Low**: Pure refactoring with no behaviour change.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
