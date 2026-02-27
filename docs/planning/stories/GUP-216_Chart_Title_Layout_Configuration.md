# GUP-216: Chart Title Layout Configuration

**Status**: 🚧 In Progress **Priority**: Low **Complexity**: Low **Created**:
2025-07-22

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

- [ ] `TitleConfig` struct with alignment (left/center/right), y-offset, and
      optional subtitle
- [ ] Multi-line titles render correctly with configurable line spacing
- [ ] Subtitle has its own `TextStyle` (typically smaller, lighter)
- [ ] Backward compatible — omitting `TitleConfig` uses the current default

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

- [ ] `TitleConfig` struct is part of `ChartConfig`
- [ ] All existing chart tests pass
- [ ] At least one example demonstrates subtitle or alignment

---

**Estimated Effort**: 1-2 days **Prerequisites**: GUP-215 ✅ **Blockers**: None
