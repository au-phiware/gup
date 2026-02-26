# GUP-202: Font-Aware Text Rendering Pipeline

**Status**: 🚧 In Progress **Priority**: Medium **Complexity**: Medium
**Created**: 2025-08-20

## Overview

Connect the `TextStyle.font_family` field to actual font atlas creation in the
text rendering pipeline, enabling per-text-element font selection end-to-end.

## Context

GUP-106 added `FontSpec`, `FontDatabase`, and `FontAtlas::with_font()` for
system font loading, plus a `font_family` field on `TextStyle`. However, the
rendering pipeline does not yet use the `font_family` field to select different
font atlases. Currently, all text renders using a single pre-created
`FontAtlas`.

## User Story

As a developer building data visualizations, I want to specify different fonts
for different text elements (e.g., "Arial" for axis labels, "Times New Roman"
for titles) so that my charts have typographically rich text.

## Acceptance Criteria

- [ ] `TextStyle.font_family` is used by the rendering pipeline to select the
      appropriate `FontAtlas`
- [ ] Multiple fonts can be rendered in a single frame
- [ ] Font atlas is lazily created on first use for each unique font family
- [ ] Fallback to default font when requested font is unavailable
- [ ] Examples updated to demonstrate multi-font rendering

## Technical Tasks

1. Modify the text rendering pipeline to check `TextStyle.font_family`
2. Create a font atlas registry/manager that maps family names to atlases
3. Handle on-the-fly atlas creation for new font families
4. Update examples to showcase multi-font text

## Dependencies

- GUP-106 ✅ (System Font Loading)

## Testing Strategy

- Unit tests for font atlas registry
- Integration tests with multiple fonts in one frame
- Visual verification of mixed-font text rendering

## Risk Assessment

- **Medium**: Multiple font atlases increase GPU memory usage; need to consider
  atlas size limits and eviction strategies.

## Definition of Done

- [ ] TextStyle.font_family drives font selection in rendering
- [ ] Multiple fonts render correctly in a single frame
- [ ] All existing tests pass
- [ ] Documentation updated
- [ ] At least one example demonstrates multi-font rendering

---

**Estimated Effort**: 1-2 weeks **Prerequisites**: GUP-106 ✅ **Blockers**: None
