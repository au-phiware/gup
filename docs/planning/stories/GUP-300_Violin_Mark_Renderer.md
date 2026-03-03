# GUP-300: Violin Mark Renderer

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2026-03-04

## Context

GUP-249 delivered the `ViolinPlotBuilder` that computes mirrored density
polygons from KDE output and lays out multi-category violins with half-violin
variants. However, the current rendering reuses the `BoxPlot` mark for the
embedded five-number summary without a dedicated renderer for the violin body
polygon itself.

This story adds a purpose-built `ViolinMark` that renders the mirrored density
polygon as a GPU-filled shape with optional gradient fill, stroke outline, and
anti-aliasing — enabling full visual fidelity for violin plots.

## User Story

> "As a data visualisation developer, I want the violin body to render as a
> smooth filled polygon with configurable gradient and outline, so that violin
> plots look polished and publication-ready at any zoom level."

## Acceptance Criteria

- [ ] `ViolinMark` implements the `Mark` trait with custom vertex/fragment
      shaders for filled polygon rendering
- [ ] Violin body polygon vertices (from `ViolinPath`) are uploaded to a GPU
      vertex buffer and rendered via triangle fan or tessellation
- [ ] Optional gradient fill (e.g., density-mapped colour gradient) is
      configurable
- [ ] Optional stroke outline along the outer contour with configurable colour
      and width
- [ ] Anti-aliased edges at all zoom levels
- [ ] `ViolinPlotBuilder` is updated to use `ViolinMark` instead of `BoxPlot`
      for the body, with `BoxPlot` retained only for the embedded overlay
- [ ] Performance: 20 violins × 128 grid points render at ≥ 60 FPS

## Dependencies

### Prerequisite Stories

- GUP-249: Violin Plot Builder ✅ — provides the builder, KDE integration, and
  polygon construction
- GUP-132: GPU Path Tessellation ✅ — provides the tessellation pipeline for
  converting polygon paths into triangle geometry

## Testing Strategy

- Unit tests: ViolinMark shader generation, vertex buffer layout validation
- Integration tests: render 3-category violins headlessly, verify no GPU
  validation errors
- Visual validation: side-by-side comparison with GUP-249's output

## Risk Assessment

- **Medium**: Polygon tessellation for concave shapes (possible with bimodal
  distributions) may produce degenerate triangles. _Mitigation_: use ear-
  clipping or fan triangulation with robust degenerate-case handling.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Retrospective added
