# GUP-170: BoxPlot Notch Rendering

**Status**: 📋 Planned

## Story Overview

**Title**: Notched Box Plot Rendering in SDF Shader **Epic**: Phase 1 Initiative
4 - Advanced Data Mapping **Priority**: Low **Story Points**: 2

## Context

GUP-166 implemented a unified BoxPlot mark with SDF-based rendering. The
`BoxPlotAttributes` struct already includes `notched: bool` and
`notch_width: f32` fields, but the fragment shader does not render notches. A
notched box plot shows a narrowing at the median to indicate the confidence
interval, which is a common statistical visualisation technique.

## User Story

**As a** data analyst **I want** to render notched box plots **So that** I can
visually compare medians with confidence interval indicators

## Acceptance Criteria

- [ ] When `notched` is `true`, the box SDF narrows symmetrically at the median
      position by `notch_width` fraction of the box width
- [ ] Notch shape is smooth (trapezoidal or curved)
- [ ] Existing non-notched rendering is unaffected (notched defaults to false)
- [ ] Demo or test exercises both notched and non-notched box plots

## Dependencies

- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- Unit test: verify `BoxPlotInstance` packs notch fields correctly
- GPU test: render notched box plot to headless texture without errors

## Definition of Done

- [ ] All acceptance criteria met
- [ ] All tests pass (`cargo test -- --test-threads=1`)
- [ ] `mask all-fix` clean

---

_Identified during GUP-166 retrospective (2025-07-17)._
