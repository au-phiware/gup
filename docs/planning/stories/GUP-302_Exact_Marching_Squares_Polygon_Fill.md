# GUP-302: Exact Marching-Squares Polygon Fill

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-15
**Completed**: 2025-07-27

## Context

GUP-250's filled-contour mode uses a cell-average approximation: each cell whose
average density falls within a contour band is filled with two triangles
covering the entire cell. This produces visually correct results at grid
resolutions ≥ 64 but at lower resolutions the band boundaries appear blocky
rather than smooth.

This story replaces the cell-average approach with exact marching-squares
polygon decomposition. Each cell would emit precisely the polygon region where
density lies within the band boundaries, producing smooth contour fills even at
grid resolutions as low as 8 × 8.

## User Story

> "As a data journalist producing publication-quality density plots, I want
> smooth contour band boundaries even at low grid resolutions so that my
> visualisations look polished without requiring a 256×256 grid."

## Acceptance Criteria

- [x] Each filled-contour band's polygon boundary matches the interpolated
      iso-contour line to sub-pixel accuracy
- [x] Bands tile seamlessly with no visible gaps or overlaps at any grid
      resolution ≥ 4
- [x] The exact decomposition handles all 16 marching-squares cases plus both
      saddle-point configurations
- [x] Visual comparison of cell-average vs exact fill at 16×16 shows clearly
      smoother boundaries in the exact version

## Dependencies

### Prerequisite Stories

- GUP-250: Density Plot Builder ✅ — provides the marching-squares
  implementation and filled contour band infrastructure

## Testing Strategy

- Unit test: verify polygon vertex counts for all 16 marching-squares cases
- Visual test: render at 16×16 and 64×64, compare screenshots
- Topology test: no gaps between adjacent bands (sum of band areas ≈ total grid
  area)

## Definition of Done

- [x] All acceptance criteria satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`

## Implementation Summary

### What Was Implemented

Replaced the cell-average filled contour approach in `filled_contour_bands()`
with exact marching-squares polygon decomposition. Each cell now emits precisely
the polygon region where the scalar field lies within band boundaries, producing
smooth contour fills even at grid resolutions as low as 4×4.

### Key Components

- **`BandState` enum**: Classifies each cell corner as Below, Inside, or Above
  relative to a band `[low, high)`.
- **`cell_band_polygons()`**: Per-cell entry point that returns zero or more band
  polygons, handling both normal and saddle configurations.
- **`boundary_walk_quad()`**: Walks the 4 cell edges collecting band polygon
  vertices in winding order for non-saddle cells.
- **`triangle_band_polygon()`**: Handles triangle sub-cells produced by saddle
  subdivision; triangles have no saddle ambiguity.
- **`emit_edge_vertices()`**: Core edge-processing logic: emits the starting
  corner (if Inside) plus any threshold crossing points in traversal order.
  Covers all 9 state-transition combinations (B↔B, B↔I, B↔A, etc.).
- **`fan_triangulate()`**: Converts convex polygons to triangle strips via fan
  from the first vertex.
- **Saddle handling**: Cells with alternating diagonal corner states are
  subdivided into 4 triangles through the cell centre, eliminating topological
  ambiguity.

### Files Changed

- `src/chart_builder/builders/density.rs` — Replaced `filled_contour_bands`
  implementation (+733 lines, −27 lines); added 23 new tests.

### Test Counts

- 46 density module tests (23 existing + 23 new), all passing.
- Full test suite: 233+ tests pass, 0 failures.
