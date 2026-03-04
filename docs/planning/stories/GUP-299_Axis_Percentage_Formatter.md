# GUP-299: Axis Percentage Formatter

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-07-28

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

- [ ] The axis system supports pluggable tick label formatters
- [ ] A `PercentFormatter` is available that formats 0.0–1.0 as "0%"–"100%"
- [ ] `AreaChartBuilder` in `Normalized` mode automatically applies percentage
      formatting to the y-axis
- [ ] Other chart builders can also use custom tick formatters via a fluent API
- [ ] Existing default formatting behaviour is unchanged

## Dependencies

### Prerequisite Stories

- GUP-247: Area Chart Builder ✅ — stores `StackMode::Normalized` flag

## Testing Strategy

- Unit tests for the `PercentFormatter` with edge cases (0.0, 0.5, 1.0, >1.0)
- Integration test verifying axis labels in normalised mode

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint and format clean
- [ ] Documentation updated
