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

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Labels lose axis provenance after generation

- **Challenge**: `generate_axis_geometry()` merges labels from all axes into a
  flat `Vec<AxisLabel>`. Once merged, there is no way to determine which axis
  produced which label, making it impossible to apply per-axis styles after the
  fact.
- **Solution**: Refactored `queue_chart_text` and `queue_chart_text_resolved` to
  iterate per-axis directly (inlining the label generation loop), rather than
  delegating to `generate_axis_geometry()` first. This keeps axis context
  available when resolving styles.
- **Pattern**: When per-element metadata (like axis origin) matters for
  downstream processing, either enrich the element struct or keep the processing
  co-located with the iteration that has context. Avoid flattening data when
  context is needed later.

#### Option-based style override is ergonomic and backward-compatible

- **Challenge**: Adding a required `TextStyle` field to `AxisConfiguration`
  would break every existing construction site.
- **Solution**: `Option<TextStyle>` with `None` default. The resolution pattern
  `config.label_style.as_ref().unwrap_or(&chart_config.label_style)` is concise
  and clear.
- **Pattern**: Use `Option<T>` for override fields that should fall back to a
  parent-level default. This follows the same pattern as CSS specificity —
  element-level overrides parent-level.

### Architectural Decisions

#### Per-axis style on AxisConfiguration rather than on ComposedChart

- **Decision**: Added `label_style: Option<TextStyle>` directly to
  `AxisConfiguration`, not as a per-axis wrapper on `ComposedChart`.
- **Reasoning**: `AxisConfiguration` is the natural home for per-axis visual
  configuration. Users already interact with it when customising tick
  appearance. Adding the style there keeps the API consistent and discoverable.
- **Trade-off**: Requires `AxisConfiguration` to import `TextStyle`, adding a
  coupling between the axis and text modules. This is acceptable since axis
  labels inherently involve text rendering.
- **Future**: If additional per-axis overrides are needed (e.g., tick label
  formatters), they can follow the same `Option<T>` pattern on
  `AxisConfiguration`.

#### Inlined per-axis iteration in queue methods

- **Decision**: `queue_chart_text` and `queue_chart_text_resolved` now inline
  the per-axis iteration instead of calling `generate_axis_geometry()`.
- **Reasoning**: The existing `generate_axis_geometry()` returns a flat list
  that discards axis provenance. Rather than changing its return type (which
  would affect external callers), the queue methods were refactored to iterate
  directly.
- **Trade-off**: Slight code duplication between `generate_axis_geometry` and
  the queue methods (both iterate the same axis list and call
  `generate_label_data`). The queue methods now have ~15 more lines each.
- **Future**: If more axis-aware processing is needed, a private helper
  `for_each_axis_labels()` could reduce duplication.

### Development Workflow Insights

- The story was straightforward — a clean additive change. The main design
  question was where to place the per-axis style (AxisConfiguration vs
  ComposedChart wrapper) and how to handle the flat label list from
  `generate_axis_geometry`. Both resolved quickly.
- GPU tests with `FontAtlasManager.get_atlas(Some("FontName"))` assertions
  provided strong verification that per-axis font families are actually resolved
  to separate atlases.
- Pre-existing failures in `mark::renderer::tests` are a noise source — not
  caused by these changes but should be investigated separately.

### Follow-up Stories

No new follow-up stories identified. The per-axis label style override is
self-contained and complete.
