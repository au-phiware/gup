# GUP-302: Exact Marching-Squares Polygon Fill

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-07-15

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

- [ ] Each filled-contour band's polygon boundary matches the interpolated
      iso-contour line to sub-pixel accuracy
- [ ] Bands tile seamlessly with no visible gaps or overlaps at any grid
      resolution ≥ 4
- [ ] The exact decomposition handles all 16 marching-squares cases plus both
      saddle-point configurations
- [ ] Visual comparison of cell-average vs exact fill at 16×16 shows clearly
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

- [ ] All acceptance criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
