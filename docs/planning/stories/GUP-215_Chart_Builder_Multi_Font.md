# GUP-215: Chart Builder Multi-Font Integration

**Status**: ✅ Complete **Priority**: Medium **Complexity**: Low **Created**:
2025-08-21 **Completed**: 2025-07-22

## Overview

Update the chart builder layer (axes, titles, labels) to use `FontAtlasManager`
so that `TextStyle.font_family` works automatically when building charts,
without users managing font atlases manually.

## Context

GUP-202 added `FontAtlasManager` and multi-font rendering support to
`TextRenderer`, but the higher-level chart builders (from GUP-018, GUP-100,
GUP-092) still use a single `FontAtlas`. Users of the chart API cannot yet
benefit from multi-font rendering without dropping down to the low-level
`TextRenderer` API.

## User Story

As a developer using the chart builder API, I want to set `font_family` on axis
labels, titles, and annotations and have the correct fonts render automatically,
so I can create typographically rich charts without managing font atlases.

## Acceptance Criteria

- [x] Chart builders accept or internally create a `FontAtlasManager`
- [x] Axis label `TextStyle.font_family` is respected during chart rendering
- [x] Chart title `TextStyle.font_family` is respected during chart rendering
- [x] Existing chart examples continue to work without changes

## Technical Tasks

1. Integrate `FontAtlasManager` into the chart rendering pipeline
2. Update axis renderers to use `queue_text_with_fonts`
3. Ensure backward compatibility with existing single-font chart API

## Dependencies

- GUP-202 ✅ (Font-Aware Text Rendering Pipeline)
- GUP-100 ✅ (Visual Chart Axis Integration)

## Testing Strategy

- Integration tests with charts using multiple fonts
- Verify existing chart examples still compile and render

## Risk Assessment

- **Low**: Straightforward integration; the font manager API is designed to be a
  drop-in alongside existing `FontAtlas` usage.

## Definition of Done

- [x] Chart builder API supports multi-font rendering via `font_family`
- [x] All existing chart tests pass
- [x] Documentation updated with chart font customisation examples
- [x] At least one chart example uses multiple fonts

## Implementation Summary

### What was implemented

- **`ChartConfig` text style fields**: Added `label_style` and `title_style`
  (`TextStyle`) to `ChartConfig` with builder methods `with_label_style()`,
  `with_title_style()`, and `with_title()`.
- **`ComposedChart::queue_chart_text()`**: Queues axis labels and optional chart
  title through `FontAtlasManager` using `queue_text_with_fonts()` so that
  `TextStyle.font_family` is automatically resolved.
- **`ComposedChart::queue_chart_text_resolved()`**: Same as above but with label
  collision detection via `LabelPositioner`.
- **`ComposedChart::queue_title_text()`**: Private helper that positions the
  title centred at the top of the chart area.

### Key files changed

| File                                  | Change                                                   |
| ------------------------------------- | -------------------------------------------------------- |
| `src/chart_builder.rs`                | Added text style fields, queue methods, 11 tests         |
| `examples/multi_font_chart_demo.rs`   | New example: chart with DejaVu Serif title + Sans labels |
| `docs/text-rendering-architecture.md` | Added "Chart Builder Multi-Font Integration" section     |

### Tests

- 11 new tests (6 unit, 5 GPU integration) in `chart_builder::tests_multi_font`
- All 1435 existing lib tests pass with 0 failures

---

**Estimated Effort**: 3-5 days **Prerequisites**: GUP-202 ✅ **Blockers**: None
