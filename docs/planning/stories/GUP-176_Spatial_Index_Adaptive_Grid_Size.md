# GUP-176: Spatial Index Adaptive Grid Size

**Priority**: Low **Complexity**: Low **Created**: 2025-08-06 **Status**: 📋
Planned

## Overview

The basic grid spatial index uses a fixed 100×100 grid regardless of dataset
size. An adaptive strategy that adjusts grid resolution based on dataset size
and distribution would improve performance across a wider range of data scales.

## Context

GUP-078 focused on advanced spatial indexing algorithms (Morton, Hierarchical)
but the underlying basic grid still uses a hardcoded 100×100 layout. For very
small datasets this wastes memory on empty cells; for very large datasets the
cells may be too coarse.

## User Story

As a developer, I want the spatial grid resolution to adapt automatically to my
dataset size so that the grid provides good performance without manual tuning.

## Acceptance Criteria

- [ ] Grid size adapts based on element count (e.g., √N × √N)
- [ ] Minimum grid size (e.g., 4×4) for small datasets
- [ ] Maximum grid size capped at buffer limits
- [ ] No regression in existing spatial index tests
- [ ] Measurable improvement for datasets where 100×100 is suboptimal

## Technical Tasks

1. Add grid size calculation heuristic to
   `InteractionSystem::build_spatial_index`
2. Update `SpatialIndexConfig` grid_size dynamically
3. Ensure buffer allocation handles variable grid sizes
4. Add tests for various dataset scales

## Dependencies

- **Requires**: GUP-076 (spatial index infrastructure)
- **Benefits from**: GUP-078 (algorithm selection context)

## Testing Strategy

- Unit tests with datasets of 10, 100, 10K, 100K elements
- Verify grid resolution adapts appropriately at each scale

## Risk Assessment

- **Low**: Simple heuristic change with clear fallback to current behaviour.

## Definition of Done

- [ ] Adaptive grid size implemented
- [ ] All existing tests pass
- [ ] `mask all-fix` passes
