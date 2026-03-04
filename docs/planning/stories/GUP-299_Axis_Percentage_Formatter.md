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
   `y_tick_format()` methods to the `ConfigurableBuilder` trait, implemented
   for all 9 chart builders.

4. **Axis rendering integration**: All 4 `generate_label_data` call sites in
   `ComposedChart` now pass through the configured formatter via the helper
   `ChartConfig::label_formatter_for(position)`.

5. **Auto-percentage for normalised mode**: Both `AreaChartBuilder::build_with_data`
   and `build_filled` automatically set the y-axis formatter to
   `PercentFormatter::new()` when `stack_mode == StackMode::Normalized` and no
   custom formatter was explicitly set.

### Key Files Changed

- `src/label/formatter.rs` — `PercentFormatter` struct + 4 unit tests
- `src/chart_builder.rs` — `ChartConfig` fields, helper, render integration
- `src/chart_builder/builders.rs` — `ConfigurableBuilder` trait extension
- `src/chart_builder/builders/area.rs` — auto-percent logic + 6 new tests
- `src/chart_builder/builders/{bar,boxplot,composite,density,heatmap/mod,line,scatter,violin}.rs` — trait implementations
- `src/chart_builder/builders/scatter.rs` — cross-builder formatter test
- `src/chart_builder/builders/line.rs` — cross-builder formatter test

### Test Counts

- 4 new `PercentFormatter` unit tests
- 6 new area chart builder tests (4 unit, 2 GPU integration)
- 1 new scatter builder test
- 1 new line builder test
- **12 new tests total**, all passing
