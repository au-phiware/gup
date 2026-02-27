# GUP-216: Chart Title Layout Configuration

**Status**: ✅ Complete **Priority**: Low **Complexity**: Low **Created**:
2025-07-22 **Completed**: 2025-07-23

## Overview

Add a dedicated `TitleConfig` struct to `ChartConfig` supporting title
alignment, vertical offset, subtitle text, and multi-line titles.

## Context

GUP-215 added basic title rendering (centred at top margin, single line).
Real-world charts often need more control: left-aligned titles, subtitles below
the main title, or a configurable offset from the top edge.

## User Story

As a chart author, I want to control the position, alignment, and appearance of
the chart title so I can match my application's design system.

## Acceptance Criteria

- [x] `TitleConfig` struct with alignment (left/center/right), y-offset, and
      optional subtitle
- [x] Multi-line titles render correctly with configurable line spacing
- [x] Subtitle has its own `TextStyle` (typically smaller, lighter)
- [x] Backward compatible — omitting `TitleConfig` uses the current default

## Technical Tasks

1. Add `TitleConfig` struct with alignment, offset, subtitle fields
2. Update `ComposedChart::queue_title_text` to use `TitleConfig`
3. Add tests and update documentation

## Dependencies

- GUP-215 ✅ (Chart Builder Multi-Font Integration)

## Testing Strategy

- Unit tests for title positioning logic
- GPU integration test verifying subtitle is queued

## Risk Assessment

- **Low**: Small, additive change to the existing title rendering path.

## Definition of Done

- [x] `TitleConfig` struct is part of `ChartConfig`
- [x] All existing chart tests pass
- [x] At least one example demonstrates subtitle or alignment

## Implementation Summary

### What was implemented

- **`TitleAlignment` enum** (`Left`, `Center`, `Right`) with `Default` deriving
  `Center`
- **`TitleConfig` struct** with fields: `text`, `alignment`, `y_offset`,
  `subtitle`, `subtitle_style`, `line_spacing` — plus fluent builder methods
- **`ChartConfig` refactor**: replaced `title: Option<String>` with
  `title_config: Option<TitleConfig>`; kept `with_title()` backward-compatible;
  added `with_title_config()` and `title()` accessor
- **`queue_title_text` update**: renders title at alignment-derived x position
  and anchor; renders subtitle below title using `line_spacing`
- **6 chart builders** updated to use `TitleConfig::new()`
- **`multi_font_chart_demo` example** updated to showcase left-aligned title
  with a subtitle

### Key files changed

| File | Change |
|------|--------|
| `src/chart_builder.rs` | `TitleAlignment`, `TitleConfig`, `ChartConfig` refactor, `queue_title_text` rewrite, 11 new tests |
| `src/chart_builder/builders/{area,bar,boxplot,heatmap,line,scatter}.rs` | Migrate `config.title = …` to `config.title_config = …` |
| `src/lib.rs` | Export `TitleConfig` and `TitleAlignment` |
| `examples/multi_font_chart_demo.rs` | Demonstrate subtitle and left alignment |

### Test counts

- 8 new unit tests for `TitleConfig` builder methods, defaults, and edge cases
- 3 new GPU integration tests for subtitle rendering, left alignment, and
  no-title edge case
- All 22 chart_builder tests pass; 1727 total tests pass (3 pre-existing flaky
  mark renderer tests)

---

**Estimated Effort**: 1-2 days **Prerequisites**: GUP-215 ✅ **Blockers**: None
