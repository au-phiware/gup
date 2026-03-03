# GUP-286: Spherical Polygon Simplification

## Story Overview

**Initiative**: Mark System **Status**: 📋 Planned **Created**: 2025-07-17

## Context

GUP-274 (Map Mark Rendering) uses Ramer–Douglas–Peucker (RDP) simplification in
planar (longitude, latitude) space. This works well for world-scale maps but
introduces geometric inaccuracies near the poles where one degree of longitude
covers much less distance than at the equator. For visualizations of polar
regions (Arctic, Antarctic) or projections centred on the poles, a spherical-
aware simplification algorithm would produce more accurate results.

## User Story

> "As a visualization developer creating polar-region maps, I want polygon
> simplification to respect great-circle distances so that coastlines near the
> poles are not distorted by the simplification pass."

## Acceptance Criteria

- [ ] A `simplify_ring_spherical()` function uses great-circle distance
      (Haversine or Vincenty) instead of planar Euclidean distance.
- [ ] The `GeoPathMark` builder supports a `simplification_method` option to
      choose between planar RDP and spherical simplification.
- [ ] A visual comparison test shows that polar coastlines (e.g., Svalbard,
      Antarctica) are better preserved with spherical simplification than planar
      RDP at the same tolerance.
- [ ] Performance overhead of spherical simplification is within 2× of planar
      RDP for the same dataset.

## Dependencies

### Prerequisite Stories

- GUP-274: Map Mark Rendering ✅ — provides the simplification infrastructure

## Testing Strategy

- Unit tests: spherical distance calculations match known great-circle
  distances.
- Comparison tests: spherical vs planar simplification at same tolerance for
  polar polygons.
- Performance test: measure simplification time for both methods on the bundled
  world GeoJSON.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated in INDEX.md
