# GUP-076: GPU Occlusion Culling for Dense Datasets

**Story ID**: GUP-076 **Title**: GPU Occlusion Culling for Dense Datasets
**Status**: 🚧 In Progress **Priority**: Low **Effort**: — **Created**: 2026-02-25
**Dependencies**: GUP-074 (Mark Performance Optimization)

## Overview

Implement compute-shader-based occlusion culling using a hierarchical Z-buffer
for dense point clouds where frustum culling alone is insufficient. GUP-074
provides frustum culling and LOD selection, but overlapping marks in dense
datasets still waste GPU fill rate.

## Context

GUP-074 implemented frustum culling and LOD selection, which eliminates
off-screen and sub-pixel marks. However, for dense datasets (>100K points in a
small viewport area), many visible marks are fully occluded by marks in front of
them. A compute-shader approach to occlusion culling would be more practical
than hardware occlusion queries (which require async readback and multi-pass
rendering).

## User Story

As a developer rendering dense point clouds with >100K overlapping marks, I want
occluded marks to be automatically culled so that GPU fill rate is not wasted on
invisible geometry.

## Acceptance Criteria

- [ ] Compute shader generates hierarchical Z-buffer from front-to-back marks
- [ ] Instance culling pass tests mark bounds against the Z-buffer
- [ ] At least 30% reduction in draw calls for typical dense scatter plots
- [ ] No visual artifacts from incorrect culling
- [ ] Configurable toggle to enable/disable occlusion culling

## Technical Tasks

1. Implement depth-only pre-pass for front-to-back mark rendering
2. Create compute shader that generates hierarchical Z-buffer mip chain
3. Create compute shader that tests instance bounding boxes against Z-buffer
4. Integrate with `InstancedBatchRenderer` from GUP-074
5. Add benchmarks comparing with/without occlusion culling

## Dependencies

- GUP-074: Mark Performance Optimization (provides `InstancedBatchRenderer`,
  `CullingManager`, `InstanceAttributes`)

## Testing Strategy

- Benchmark with 100K, 500K, 1M overlapping circles in a dense cluster
- Visual regression tests ensure no marks are incorrectly culled
- Compare draw call counts with/without occlusion culling

## Success Metrics

- 30-50% reduction in draw calls for dense datasets
- No visual regressions
- <1ms compute shader overhead for 1M instances

## Risk Assessment

- **Risk**: Hierarchical Z-buffer generation may be too expensive for small
  datasets
  - **Mitigation**: Only enable for datasets above a configurable threshold
- **Risk**: False positives (culling visible marks) due to Z-buffer resolution
  - **Mitigation**: Conservative bounds testing with configurable margin

## Definition of Done

- [ ] Compute shader implementation compiles and runs
- [ ] Integration tests verify correct culling behavior
- [ ] Performance benchmarks show improvement for dense datasets
- [ ] No visual regressions in existing tests
- [ ] Documentation updated
