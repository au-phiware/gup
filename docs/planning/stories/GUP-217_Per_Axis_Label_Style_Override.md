# GUP-217: Per-Axis Label Style Override

**Status**: ✅ Complete **Priority**: Low **Complexity**: Low **Created**:
2025-07-22 **Completed**: 2025-07-27

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

- [x] `AxisConfiguration` or a new wrapper accepts an optional `TextStyle`
      override
- [x] When set, the per-axis style is used instead of `ChartConfig.label_style`
- [x] `font_family` is respected per-axis via `FontAtlasManager`
- [x] Backward compatible — omitting the override uses the chart-level style

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

- [x] Per-axis label style override works
- [x] All existing chart tests pass
- [x] Documentation shows per-axis styling example

## Implementation Summary

### What was implemented

- **`AxisConfiguration.label_style`**: Added `label_style: Option<TextStyle>`
  field to `AxisConfiguration` with `with_label_style()` builder method.
- **`queue_chart_text` refactor**: Refactored from using
  `generate_axis_geometry` (which merges labels from all axes) to iterating
  per-axis directly, resolving per-axis style override vs chart-level
  `label_style` fallback.
- **`queue_chart_text_resolved` refactor**: Same per-axis iteration approach,
  with collision detection via `LabelPositioner`.
- **Documentation**: Added "Per-Axis Label Style Overrides (GUP-217)" section to
  `docs/text-rendering-architecture.md`.

### Key files changed

| File                                  | Change                                       |
| ------------------------------------- | -------------------------------------------- |
| `src/axis.rs`                         | Added `label_style` field and builder method |
| `src/chart_builder.rs`                | Refactored queue methods, added 7 tests      |
| `docs/text-rendering-architecture.md` | Added per-axis style override documentation  |

### Tests

- 7 new tests (3 unit, 4 GPU integration) in `chart_builder::tests_multi_font`
- All 1733 existing lib tests pass (3 pre-existing failures in unrelated
  `mark::renderer` module)

---

**Estimated Effort**: 1-2 days **Prerequisites**: GUP-215 ✅ **Blockers**: None
