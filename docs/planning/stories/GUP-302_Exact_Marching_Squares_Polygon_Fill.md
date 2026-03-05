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
- **`cell_band_polygons()`**: Per-cell entry point that returns zero or more
  band polygons, handling both normal and saddle configurations.
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

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Boundary-Walking Algorithm for Band Polygons

- **Challenge**: Standard marching squares extracts iso-contour _lines_ at a
  single threshold. Filled contour bands require the polygon _area_ between two
  thresholds, which is a fundamentally different geometric operation.
- **Solution**: Classify each cell corner as Below/Inside/Above relative to the
  band, then walk the cell boundary collecting vertices: cell corners that are
  Inside, plus interpolated crossing points where thresholds intersect cell
  edges. The resulting vertex list traces the band polygon in winding order
  because straight-line connections between consecutive vertices automatically
  follow either cell edges or interior iso-contour segments.
- **Pattern**: For any two-threshold polygon extraction on a grid cell, the
  boundary-walk approach produces correct results for non-saddle cells with
  minimal per-edge logic (9 state-transition cases).

#### Saddle Disambiguation via Triangle Subdivision

- **Challenge**: Saddle cells (where diagonally opposite corners share a state)
  can produce disconnected band regions that the simple boundary walk merges
  into a single incorrect polygon.
- **Solution**: Subdivide saddle cells into 4 triangles through the cell centre
  (whose value is the bilinear average of the 4 corners). Triangles have no
  saddle ambiguity, so the same boundary-walk logic applies to each sub-triangle
  independently.
- **Pattern**: When a grid algorithm encounters topological ambiguity, splitting
  cells into triangles is a robust escape hatch that works with the same
  per-edge processing logic.

#### Top-Boundary Nudge for Inclusive Last Band

- **Challenge**: The original `filled_contour_bands` used `[low, high)`
  half-open intervals with `high = max_val` for the last band, causing cells at
  exactly `max_val` to be classified as Above and excluded from all bands.
- **Solution**: Nudge the top boundary to `max_val + epsilon` so the last band
  includes peak-density cells. Store the original `max_val` in the `ContourBand`
  struct so downstream consumers see the correct threshold.
- **Pattern**: When partitioning a continuous range into half-open intervals,
  always nudge the final boundary slightly beyond the data maximum.

### Architectural Decisions

#### Reuse `safe_lerp_t` for Band Threshold Interpolation

- **Decision**: The new `lerp_point` function delegates to the existing
  `safe_lerp_t` for interpolation, ensuring band polygon boundaries use
  identical crossing-point calculations as the contour-line extraction.
- **Reasoning**: Using the same interpolation guarantees that band boundaries
  align perfectly with iso-contour lines, satisfying the sub-pixel accuracy
  acceptance criterion.
- **Trade-off**: None — this is strictly better than reimplementing
  interpolation.
- **Future**: If the interpolation changes (e.g. to account for bilinear
  curvature), both line contours and filled bands update consistently.

#### In-Place Replacement of `filled_contour_bands`

- **Decision**: Replaced the function's implementation in-place rather than
  creating a new function and deprecating the old one.
- **Reasoning**: The function is public API but the output format
  (`Vec<ContourBand>`) is unchanged. The only behavioural difference is that
  band polygon boundaries now follow iso-contours rather than cell edges, which
  is strictly more accurate. The `density_scatter_overlay` example and all
  existing tests pass without modification.
- **Trade-off**: No option to toggle between the old cell-average and new exact
  mode. If a user relied on the blocky cell-level behaviour, they would need to
  adapt.
- **Future**: If performance profiling shows the exact mode is too slow for very
  large grids, a fallback to the simpler approach could be added via a
  `DensityConfig` flag.

### Development Workflow Insights

- The algorithm design required careful reasoning about 9 edge-transition cases
  and saddle topology before writing code. Spending time on paper analysis
  prevented iteration-heavy debugging later.
- The topology test (band areas summing to grid area) was the most powerful
  correctness check — it caught boundary accounting bugs that per-case vertex
  count tests missed.
- The existing `density_scatter_overlay` example served as an integration
  validation, confirming the new implementation works end-to-end with the KDE
  pipeline.

### Follow-up Stories

No new stories identified. The GPU compute shader path
(`density_marching_squares.compute.wgsl`) only extracts contour _lines_, not
filled bands, so it does not need a parallel update for this change. If GPU-side
filled band extraction is needed in the future, GUP-301's infrastructure would
support it.
