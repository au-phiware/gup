# GUP-360: GPU Fill Tessellation

## Story Overview

**Initiative**: Mark System **Status**: 💡 New **Created**: 2026-03-05

## Context

GUP-132 provides a GPU compute shader for path stroke tessellation (converting
Bezier path commands into triangle strip geometry for outlined paths). GUP-298
introduced the `FilledPolygon` mark which uses CPU-side ear-clipping to
tessellate polygon fills. For very large dynamic polygons (>100K vertices) that
change every frame, CPU tessellation becomes a bottleneck. A GPU compute shader
for polygon fill tessellation would enable per-frame updates without CPU
round-trips.

## User Story

> "As a visualisation developer working with large dynamic polygons, I want GPU
> fill tessellation so that polygon fills can be updated per-frame without CPU
> bottlenecks."

## Acceptance Criteria

- [ ] A GPU compute shader that converts a closed polygon vertex list into
      filled triangles
- [ ] Handles concave polygons correctly
- [ ] Supports polygons with up to 100,000 vertices
- [ ] Can be used as an alternative backend for `FilledPolygon` tessellation
- [ ] Performance: at least 10× faster than CPU ear-clipping for 100K vertices

## Dependencies

### Prerequisite Stories

- GUP-132: GPU Path Tessellation ✅ — compute shader infrastructure
- GUP-298: Filled Polygon Mark ✅ — defines the `TriangleInstance` output format

## Testing Strategy

- Correctness tests comparing GPU vs CPU tessellation output
- Performance benchmarks at 1K, 10K, 100K vertex counts
- Concave polygon edge cases

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint and format clean
- [ ] Documentation updated
