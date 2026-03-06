# GUP-364: Composite Line/Area Scale Integration

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**:
2025-07-27

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

- [x] Line builder produces correctly scaled segment positions when given
      explicit x_scale/y_scale in config.
- [x] Area builder produces correctly scaled segment positions when given
      explicit x_scale/y_scale in config.
- [x] The override-by-append for line and area in composite `build_layer()` is
      removed.
- [x] Standalone line and area charts render correctly.
- [x] Composite charts with line/area layers continue to render correctly.
- [x] All existing tests pass.

## Technical Tasks

- [x] Refactor line builder to map segment start/end positions through config
      scales during build.
- [x] Refactor area builder similarly.
- [x] Remove line/area overrides from composite `build_layer()`.
- [x] Add standalone line and area render tests.

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

- [x] All Acceptance Criteria are satisfied.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

Integrated `AxisScale::scale_value()` → range → NDC mapping into the
line and area builders so that composite `build_layer()` no longer needs
override-by-append for any layer type. All four layer types (scatter,
line, bar, area) now follow the same uniform pattern: inject unified
scales into the builder's config, call `build_with_data()`, and use the
resulting visualization directly.

### Key Files Changed

| File | Change |
| --- | --- |
| `src/chart_builder/builders/line.rs` | Added scale-aware NDC mapping path; clones scales before config is moved |
| `src/chart_builder/builders/area.rs` | Added full NDC mapping (previously absent); computes domain from segments or uses explicit scales |
| `src/chart_builder/builders/composite.rs` | Removed all override-by-append logic for line/area layers; uniform 4-arm match |
| `tests/composite_chart_integration.rs` | Updated test to reflect layers are render-ready after build |

### Test Counts

- 2 new render tests (line with explicit scales, area with explicit scales)
- All 3081 existing lib tests pass
- All 15 composite integration tests pass
- All examples compile
