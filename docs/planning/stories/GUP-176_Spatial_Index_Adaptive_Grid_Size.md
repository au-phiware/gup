# GUP-176: Spatial Index Adaptive Grid Size

**Priority**: Low **Complexity**: Low **Created**: 2025-08-06 **Status**: ✅
Complete (2025-08-07)

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

- [x] Grid size adapts based on element count (e.g., √N × √N)
- [x] Minimum grid size (e.g., 4×4) for small datasets
- [x] Maximum grid size capped at buffer limits
- [x] No regression in existing spatial index tests
- [x] Measurable improvement for datasets where 100×100 is suboptimal

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

- [x] Adaptive grid size implemented
- [x] All existing tests pass
- [x] `mask all-fix` passes

## Implementation Summary

### What was implemented

Added an `adaptive_grid_side` static method to `InteractionSystem` that computes
grid resolution as `ceil(√N)`, clamped between a minimum of 4 and the maximum
side that fits within `max_spatial_cells` (√10000 = 100). This is called at the
start of `build_spatial_index` before any cell computations.

### Key files changed

- `src/interaction.rs` — Added `adaptive_grid_side()` method and integrated it
  into `build_spatial_index()`; added 6 unit tests
- `tests/spatial_index_tests.rs` — Added 4 GPU integration tests for adaptive
  grid behaviour

### Test counts

- 6 unit tests for the heuristic function
- 4 GPU integration tests for end-to-end adaptive behaviour
- 51 total spatial index tests pass (0 regressions)
