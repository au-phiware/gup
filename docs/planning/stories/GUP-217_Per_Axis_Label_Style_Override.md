# GUP-217: Per-Axis Label Style Override

**Status**: 📋 Planned **Priority**: Low **Complexity**: Low **Created**:
2025-07-22

## Overview

Allow individual axes to override the chart-level `label_style` from
`ChartConfig`, enabling different fonts, sizes, or colours for each axis.

## Context

GUP-215 added a single `label_style` on `ChartConfig` that applies to all axes.
Some charts need the X-axis labels in one font (e.g., for dates) and Y-axis
labels in another (e.g., for currency values), or different font sizes for
primary vs secondary axes.

## User Story

As a chart author, I want to set a different text style on individual axes so
that I can optimise readability for each axis's data domain.

## Acceptance Criteria

- [ ] `AxisConfiguration` or a new wrapper accepts an optional `TextStyle`
      override
- [ ] When set, the per-axis style is used instead of `ChartConfig.label_style`
- [ ] `font_family` is respected per-axis via `FontAtlasManager`
- [ ] Backward compatible — omitting the override uses the chart-level style

## Technical Tasks

1. Add `label_style: Option<TextStyle>` to `AxisConfiguration` (or a chart-
   level axis wrapper)
2. Update `queue_chart_text` to merge per-axis styles
3. Add tests and documentation

## Dependencies

- GUP-215 ✅ (Chart Builder Multi-Font Integration)

## Testing Strategy

- GPU test with two axes using different `font_family` values
- Verify correct atlas count matches unique fonts used

## Risk Assessment

- **Low**: Additive change; `queue_chart_text` already iterates over axes.

## Definition of Done

- [ ] Per-axis label style override works
- [ ] All existing chart tests pass
- [ ] Documentation shows per-axis styling example

---

**Estimated Effort**: 1-2 days **Prerequisites**: GUP-215 ✅ **Blockers**: None
