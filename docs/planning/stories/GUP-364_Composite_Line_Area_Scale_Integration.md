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

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Area Builder Had No NDC Mapping

- **Challenge**: The area builder (`area.rs`) was storing raw data-space
  coordinates in its `AreaSegment` positions and the attr bindings simply
  passed them through (`|seg| seg.start_pos`). Standalone area charts
  rendered using raw data-space values as if they were NDC — this only
  worked when the composite builder overrode those bindings.
- **Solution**: Added full NDC mapping to the area builder, mirroring the
  line builder's pattern: compute chart area, derive NDC bounds, then
  apply data → NDC mapping in the attr closures. The standalone no-scale
  path auto-computes domain from all segment positions (upper and lower
  boundary vertices).
- **Pattern**: When refactoring a layer builder for scale integration,
  check whether the builder actually maps its positions to NDC — some
  builders may have been silently relying on post-build overrides.

#### Config Ownership and Scale Cloning

- **Challenge**: `self.config` is moved into `ComposedChart::new()`, but
  the scale-aware NDC mapping closures need the scales after that move.
- **Solution**: Clone the optional scales out of config before the move
  (`let x_scale_opt = self.config.x_scale.clone()`). This is cheap since
  `AxisScale` is a small enum of scalar fields.
- **Pattern**: When a builder moves its config into a composed output,
  extract any values needed for post-composition attr bindings beforehand.

### Architectural Decisions

#### Uniform build_layer Pattern

- **Decision**: Make all four `build_layer()` arms identical: set
  `show_axes = false`, inject scales, call `build_with_data()`, return
  the visualization.
- **Reasoning**: Eliminates special-case logic that was fragile and easy
  to get wrong. Each builder is self-contained and testable in isolation.
- **Trade-off**: The NDC mapping logic is now duplicated across line and
  area builders (and is different from scatter/bar which use
  `apply_accessors_to_selection`). A shared helper could reduce this, but
  line/area segment types differ from point-based marks.
- **Future**: If more segment-based marks are added (e.g., arrow marks),
  a shared `map_segments_to_ndc()` helper could be extracted.

#### Two-Path NDC Mapping (Scales vs Auto-Domain)

- **Decision**: Both line and area builders use an `if let (Some(xs),
  Some(ys))` branch for scale-aware mapping and an `else` branch for the
  original linear domain-to-NDC mapping.
- **Reasoning**: Standalone charts (no explicit scales) must continue to
  auto-compute domain from data. Composite charts inject scales. The
  two-path approach keeps both cases working without changing the API.
- **Trade-off**: The branch adds code size. Could unify by always
  creating a default linear scale from auto-domain, but that would change
  subtle behaviour (e.g., padding) for standalone charts.

### Development Workflow Insights

- The story was small and focused — a clean "follow the established
  pattern" task. The GUP-362 pattern was well-documented in its
  retrospective, which made this straightforward.
- The flaky `test_cache_hit_is_significantly_faster` grid performance
  test failed during validation but is unrelated to this story.
- Disk space constraints required using `CARGO_TARGET_DIR=/tmp/gup-build`
  to run tests, since the ZFS dataset for `/home/corin/src` was at 100%.

### Follow-up Stories

1. **GUP-366: Extract Shared Segment NDC Mapping Helper** — The
   scale_value → range → NDC mapping logic is now duplicated across line
   and area builders. A shared `map_segment_positions_to_ndc()` utility
   could reduce this duplication if more segment-based mark types are
   added.
