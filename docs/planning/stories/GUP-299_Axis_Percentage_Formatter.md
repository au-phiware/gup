# GUP-299: Axis Percentage Formatter

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-28
**Completed**: 2025-07-28

## Context

When `AreaChartBuilder` uses `.stack_normalized()` mode, the y-axis values range
from 0.0 to 1.0 representing proportions. Users expect these to be displayed as
percentages (e.g., "50%" instead of "0.5"). The current axis system does not
support pluggable tick formatters, so the `StackMode::Normalized` flag stored on
the builder is not consumed during rendering.

## User Story

> "As a chart user, I want normalised stacked area charts to automatically show
> y-axis labels as percentages so that the chart is immediately understandable
> without mental arithmetic."

## Acceptance Criteria

- [x] The axis system supports pluggable tick label formatters
- [x] A `PercentFormatter` is available that formats 0.0–1.0 as "0%"–"100%"
- [x] `AreaChartBuilder` in `Normalized` mode automatically applies percentage
      formatting to the y-axis
- [x] Other chart builders can also use custom tick formatters via a fluent API
- [x] Existing default formatting behaviour is unchanged

## Dependencies

### Prerequisite Stories

- GUP-247: Area Chart Builder ✅ — stores `StackMode::Normalized` flag

## Testing Strategy

- Unit tests for the `PercentFormatter` with edge cases (0.0, 0.5, 1.0, >1.0)
- Integration test verifying axis labels in normalised mode

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass
- [x] Lint and format clean
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

1. **`PercentFormatter`** (`src/label/formatter.rs`): A dedicated formatter that
   multiplies by 100 and appends `%`. Configurable precision via
   `PercentFormatter::with_precision(n)`.

2. **Pluggable tick label formatters on `ChartConfig`**: Added
   `x_label_formatter` and `y_label_formatter` fields
   (`Option<Arc<dyn LabelFormatter>>`) to `ChartConfig` with builder methods
   `with_x_label_formatter()` / `with_y_label_formatter()`.

3. **`ConfigurableBuilder` trait extension**: Added `x_tick_format()` and
   `y_tick_format()` methods to the `ConfigurableBuilder` trait, implemented for
   all 9 chart builders.

4. **Axis rendering integration**: All 4 `generate_label_data` call sites in
   `ComposedChart` now pass through the configured formatter via the helper
   `ChartConfig::label_formatter_for(position)`.

5. **Auto-percentage for normalised mode**: Both
   `AreaChartBuilder::build_with_data` and `build_filled` automatically set the
   y-axis formatter to `PercentFormatter::new()` when
   `stack_mode == StackMode::Normalized` and no custom formatter was explicitly
   set.

### Key Files Changed

- `src/label/formatter.rs` — `PercentFormatter` struct + 4 unit tests
- `src/chart_builder.rs` — `ChartConfig` fields, helper, render integration
- `src/chart_builder/builders.rs` — `ConfigurableBuilder` trait extension
- `src/chart_builder/builders/area.rs` — auto-percent logic + 6 new tests
- `src/chart_builder/builders/{bar,boxplot,composite,density,heatmap/mod,line,scatter,violin}.rs`
  — trait implementations
- `src/chart_builder/builders/scatter.rs` — cross-builder formatter test
- `src/chart_builder/builders/line.rs` — cross-builder formatter test

### Test Counts

- 4 new `PercentFormatter` unit tests
- 6 new area chart builder tests (4 unit, 2 GPU integration)
- 1 new scatter builder test
- 1 new line builder test
- **12 new tests total**, all passing

## Retrospective

**Completed**: 2025-07-28

### Key Technical Learnings

#### Arc vs Box for Clonable Trait Objects

- **Challenge**: `ChartConfig` derives `Clone`, but `Box<dyn LabelFormatter>`
  does not implement `Clone`. Adding formatter fields required a compatible
  smart pointer.
- **Solution**: Used `Option<Arc<dyn LabelFormatter>>` instead of
  `Option<Box<dyn LabelFormatter>>`. `Arc` implements `Clone` via reference
  counting, which works naturally with derive macros.
- **Pattern**: When a struct derives `Clone` and needs to hold trait objects,
  use `Arc<dyn Trait>` instead of `Box<dyn Trait>`. This also enables zero-cost
  sharing of formatters across cloned configs.

#### Centralising Formatter Lookup

- **Challenge**: There were 4 separate `generate_label_data` call sites in
  `ComposedChart` that all needed the formatter plumbed through.
- **Solution**: Added a `ChartConfig::label_formatter_for(position)` helper that
  maps `AxisPosition` → `Option<&dyn LabelFormatter>`, keeping each call site to
  a single-line change.
- **Pattern**: When a configuration value needs to be routed to multiple
  consumers based on context (e.g. axis position), add a lookup helper on the
  config struct rather than duplicating the match logic.

### Architectural Decisions

#### Trait Extension over New Trait

- **Decision**: Extended `ConfigurableBuilder` with `x_tick_format` /
  `y_tick_format` rather than creating a separate `FormatterCapableBuilder`
  trait.
- **Reasoning**: All 9 builders already implement `ConfigurableBuilder` and have
  a `config: ChartConfig`. Tick formatting is a universal chart concern, not a
  specialised capability.
- **Trade-off**: The `ConfigurableBuilder` trait grows by 2 methods. If the
  number of axis-specific methods continues to grow, a separate trait may become
  worthwhile.
- **Future**: This pattern is straightforward to extend for additional per-axis
  configuration (e.g., axis title formatters, tick mark styles).

#### Auto-Apply with Override

- **Decision**: `AreaChartBuilder` automatically sets `PercentFormatter` for
  normalised mode, but only when no custom formatter was explicitly set.
- **Reasoning**: Sensible defaults without removing user control. The check
  `config.y_label_formatter.is_none()` preserves explicit overrides.
- **Trade-off**: The auto-apply happens at build time, not at
  `stack_normalized()` call time. This means calling `.y_tick_format(...)` after
  `.stack_normalized()` correctly overrides the auto-formatter.
- **Future**: Other builders could adopt similar auto-apply patterns (e.g., log
  scale auto-selecting scientific notation).

### Development Workflow Insights

- The story was very well-scoped — all 5 acceptance criteria mapped cleanly to
  discrete implementation steps.
- The existing `LabelFormatter` trait and `NumericFormatter::percentage()`
  method provided good prior art. `PercentFormatter` was purpose-built for axis
  labels with simpler semantics (always multiply by 100, integer precision by
  default).
- Running `mask all-fix` revealed no new warnings in the gup lib — only
  pre-existing markdown lint issues in other story files and pre-existing dead
  code warnings in `gup-macros`.

### Follow-up Stories

No follow-up stories are needed. The implementation is self-contained and all
acceptance criteria are fully met.
