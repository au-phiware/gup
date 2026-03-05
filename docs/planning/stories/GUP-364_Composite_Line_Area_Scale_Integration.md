# GUP-364: Composite Line/Area Scale Integration

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-07-27

## Context

GUP-362 removed the override-by-append workaround for scatter and bar layers in
the composite builder's `build_layer()`, since `apply_accessors_to_selection`
now correctly integrates with scales. However, line and area layers still use
override-by-append to remap segment start/end positions through the composite's
unified scales. This asymmetry can be eliminated by integrating scale
transformations into the line/area builder's segment creation.

## User Story

> "As a chart builder maintainer, I want line and area layers to handle their
> own scale-to-NDC mapping during build, so that the composite builder's
> `build_layer()` is uniform and override-free for all layer types."

## Acceptance Criteria

- [ ] Line builder produces correctly scaled segment positions when given
      explicit x_scale/y_scale in config.
- [ ] Area builder produces correctly scaled segment positions when given
      explicit x_scale/y_scale in config.
- [ ] The override-by-append for line and area in composite `build_layer()` is
      removed.
- [ ] Standalone line and area charts render correctly.
- [ ] Composite charts with line/area layers continue to render correctly.
- [ ] All existing tests pass.

## Technical Tasks

- [ ] Refactor line builder to map segment start/end positions through config
      scales during build.
- [ ] Refactor area builder similarly.
- [ ] Remove line/area overrides from composite `build_layer()`.
- [ ] Add standalone line and area render tests.

## Dependencies

### Prerequisite Stories

- GUP-362: Accessor-to-GPU Position Pipeline ✅

## Testing Strategy

- Standalone line/area rendering tests (headless render, check for visible
  marks).
- Composite charts with line/area layers (existing integration tests).

## Risk Assessment

- **Low**: The pattern established by GUP-362 for scatter/bar can be directly
  applied to line/area segments.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
