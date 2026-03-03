# GUP-298: Filled Polygon Mark

## Story Overview

**Initiative**: Mark System **Status**: 📋 Planned **Created**: 2025-07-28

## Context

Area charts, choropleth maps, and other filled-region visualisations currently
render their polygon outlines using `Line` mark segments. This produces an
outlined shape rather than a truly filled region. A dedicated `FilledPolygon`
mark type would use compute-shader tessellation (building on the GUP-132 path
tessellation pipeline) to produce GPU-side triangle geometry from closed polygon
outlines, enabling correct filled rendering.

## User Story

> "As a visualisation developer, I want a `FilledPolygon` mark type so that area
> charts and other polygon-based visualisations render as filled shapes rather
> than outlines."

## Acceptance Criteria

- [ ] A `FilledPolygon` mark type is available in the mark system
- [ ] It accepts a closed polygon (list of vertices) and produces filled
      triangles via GPU tessellation
- [ ] The `AreaChartBuilder` can use `FilledPolygon` instead of `Line` segments
      for true filled rendering
- [ ] Per-vertex colour interpolation is supported for gradient fills
- [ ] Performance is comparable to Line-based rendering for polygon outlines up
      to 10,000 vertices

## Dependencies

### Prerequisite Stories

- GUP-132: GPU Path Tessellation ✅ — provides the compute-shader tessellation
  pipeline
- GUP-247: Area Chart Builder ✅ — provides the polygon outline data that needs
  filled rendering

## Testing Strategy

- Unit tests for polygon triangulation correctness
- GPU integration test rendering a filled polygon without validation errors
- Visual comparison between outline and filled rendering

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint and format clean
- [ ] Documentation updated
