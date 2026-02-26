# GUP-171: BoxPlot Pixel-Space Stroke Widths

**Status**: 🚧 In Progress

## Story Overview

**Title**: Viewport-Aware Stroke Widths for Box Plot Marks **Epic**: Phase 1
Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**: 3

## Context

GUP-166's SDF box plot shader specifies stroke widths and outlier radii in
clip-space units. This means a 0.004 stroke width looks different on a 400px
window vs a 2000px window. A viewport-dimensions uniform would allow the shader
to convert clip-space values to pixel-space, producing visually consistent
strokes regardless of resolution.

This issue applies to all SDF-based marks (Circle, Rectangle, BoxPlot) and could
be generalised into a shared viewport uniform pattern.

## User Story

**As a** developer rendering box plots at varying resolutions **I want** stroke
widths and outlier radii to remain pixel-consistent **So that** visualisations
look the same on different screen sizes

## Acceptance Criteria

- [ ] A viewport-dimensions uniform is passed to the box plot shader
- [ ] Stroke width and outlier radius are interpreted as pixel values in the
      fragment shader, converted from clip space using the viewport dimensions
- [ ] Visual appearance is consistent across different canvas sizes
- [ ] The same pattern is applicable to Circle and Rectangle marks (but need not
      be implemented for those in this story)

## Dependencies

- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- GPU test: render same box plot to 400×400 and 800×800 textures, verify stroke
  pixel widths are equivalent

## Risk Assessment

**Low Risk**: Adding a uniform buffer requires a second bind group entry. The
existing bind group layout extension is straightforward.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] All tests pass (`cargo test -- --test-threads=1`)
- [ ] `mask all-fix` clean

---

_Identified during GUP-166 retrospective (2025-07-17)._
