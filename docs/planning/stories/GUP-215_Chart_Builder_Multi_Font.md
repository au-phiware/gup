# GUP-215: Chart Builder Multi-Font Integration

**Status**: 📋 Planned **Priority**: Medium **Complexity**: Low **Created**:
2025-08-21

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

- [ ] Chart builders accept or internally create a `FontAtlasManager`
- [ ] Axis label `TextStyle.font_family` is respected during chart rendering
- [ ] Chart title `TextStyle.font_family` is respected during chart rendering
- [ ] Existing chart examples continue to work without changes

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

- [ ] Chart builder API supports multi-font rendering via `font_family`
- [ ] All existing chart tests pass
- [ ] Documentation updated with chart font customisation examples
- [ ] At least one chart example uses multiple fonts

---

**Estimated Effort**: 3-5 days **Prerequisites**: GUP-202 ✅ **Blockers**: None
