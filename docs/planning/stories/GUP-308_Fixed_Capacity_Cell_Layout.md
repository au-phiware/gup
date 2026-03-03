# GUP-308: Fixed-Capacity Cell Layout for Per-Cell GPU Uploads

## Story Overview

**Initiative**: Advanced Scale **Status**: 💡 New **Created**: 2025-07-17

## Context

The `StreamingLodManager` (GUP-258) currently re-uploads the entire level buffer
whenever any cell in that level is dirty. This is O(total_points) per flush
cycle, which becomes a bottleneck at >100K points. By laying out cells in the
GPU buffer with a fixed capacity per cell, each dirty cell can be updated via
`upload_range()` independently — O(dirty_cells × cell_capacity) instead.

## User Story

As a developer building a streaming visualization with >100K live data points, I
want per-cell GPU uploads so that the incremental update cost is proportional to
the number of dirty cells, not the total dataset size.

## Acceptance Criteria

- [ ] Each LOD level's GPU buffer is organized as fixed-capacity cell slots.
- [ ] `flush_dirty_cells()` uses `upload_range()` for each dirty cell.
- [ ] When a cell overflows its capacity, the level buffer is reallocated with
      larger slots and all cells are re-uploaded.
- [ ] A benchmark shows >2× improvement in flush latency at 500K total points
      compared to the current full-level upload.

## Dependencies

- GUP-258 ✅ (StreamingLodManager)

## Testing Strategy

- Unit tests for cell slot layout calculations.
- Benchmark comparing full-level vs. per-cell upload at 100K, 500K, 1M points.

## Risk Assessment

- **Medium**: Fixed-capacity cells waste memory when cell utilisation is uneven.
  Mitigation: use a growth strategy (start small, double on overflow).

## Definition of Done

- [ ] All acceptance criteria satisfied
- [ ] Tests pass: `cargo test -- --test-threads=1`
- [ ] Lint clean: `mask all-fix`
- [ ] Benchmark results documented
