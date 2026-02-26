# GUP-170: BoxPlot Notch Rendering

**Status**: ✅ Complete (2025-07-19)

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

- [x] When `notched` is `true`, the box SDF narrows symmetrically at the median
      position by `notch_width` fraction of the box width
- [x] Notch shape is smooth (trapezoidal or curved)
- [x] Existing non-notched rendering is unaffected (notched defaults to false)
- [x] Demo or test exercises both notched and non-notched box plots

## Dependencies

- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- Unit test: verify `BoxPlotInstance` packs notch fields correctly
- GPU test: render notched box plot to headless texture without errors

## Definition of Done

- [x] All acceptance criteria met
- [x] All tests pass (`cargo test -- --test-threads=1`)
- [x] `mask all-fix` clean

## Implementation Summary

### What Was Implemented

Notched box plot rendering via the existing SDF-based fragment shader. When
`notched == true`, the box narrows symmetrically at the median using a linear
interpolation from full width at Q1/Q3 to `(1 - notch_width) * width` at the
median, producing a smooth trapezoidal notch shape.

### Key Files Changed

- **`src/mark/boxplot.rs`**: Added `notched: u32`, `notch_width: f32`, and
  `_pad_notch: [f32; 2]` fields to `BoxPlotInstance` (256 → 272 bytes). Updated
  `From<BoxPlotAttributes>`, `build_instance`, and generated shader strings.
- **`src/mark/shaders/boxplot.frag.wgsl`**: Added notch SDF logic — computes
  `effective_hw` that varies with position along the value axis.
- **`src/mark/shaders/boxplot.vert.wgsl`**: Updated WGSL struct to match.
- **`src/mark/shaders/boxplot_pattern.frag.wgsl`**: Same notch logic for
  pattern-enabled rendering.
- **`src/selection.rs`**: Added `gpu_render_notched_boxplot` GPU test.
- **`examples/boxplot_rendering_demo.rs`**: Updated demo to alternate notched
  and non-notched box plots.

### Test Counts

- 4 new unit tests (notch field packing and attribute builder)
- 1 new GPU integration test (notched + non-notched rendering)
- All 1589 existing tests continue to pass

---

_Identified during GUP-166 retrospective (2025-07-17)._
