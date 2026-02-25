# GUP-181: GPU-Accelerated Selection Hit Testing

**Status**: ✅ Complete **Priority**: Medium **Effort**: 5 **Dependencies**:
GUP-075 (Interactive Mark Selection), GUP-012 (GPU Interaction System)

## Overview

Integrate the `MarkSelectionSystem` from GUP-075 with the GPU-based
`InteractionSystem` from GUP-012 to enable high-performance hit testing for
datasets with 10K+ marks. Currently, the selection system accepts hit IDs from
any source; this story wires up the GPU compute shader path for sub-millisecond
hit testing on large datasets.

## Context

GUP-075 delivered a complete selection state management system with undo/redo,
visual styles, and selection tools. Its demo uses CPU-side distance checks which
work well for small datasets (200 points) but won't scale.

The `InteractionSystem` from GUP-012 already has GPU compute shaders for hit
testing and spatial indexing. This story bridges the two systems so that
`MarkSelectionSystem` can use GPU-accelerated hit IDs when available.

## User Story

As a developer building interactive visualizations with 10K+ data points, I want
the selection system to use GPU-accelerated hit testing so that hover and click
interactions remain responsive at <1ms.

## Acceptance Criteria

1. [x] `MarkSelectionSystem` can optionally hold a reference to
       `InteractionSystem`
2. [x] Point hit tests are dispatched to GPU when `InteractionSystem` is
       available
3. [x] Rectangle and lasso selections use spatial index for candidate filtering
4. [x] Hit testing latency stays under 1ms for 100K points
5. [x] Fallback to CPU hit testing when GPU is not available
6. [x] Integration example demonstrating large-dataset selection

## Technical Tasks

- [x] Add optional `InteractionSystem` integration to `MarkSelectionSystem`
- [x] Wire up `query_point` for hover and click events
- [x] Wire up `query_region` for rectangle selection
- [x] Implement async hit test result handling in event loop
- [x] Add benchmark test for 100K-point selection latency
- [x] Create large-dataset selection example

## Testing Strategy

- Unit tests for GPU/CPU fallback logic
- Integration tests with `InteractionSystem`
- Performance benchmark: 100K points, measure hit test latency
- Visual example with 50K+ marks

## Risk Assessment

- **Medium**: Async buffer readback in synchronous event loops requires careful
  design (polling vs futures)
- **Low**: The two systems are already designed to work independently

## Definition of Done

- [x] GPU-accelerated hit testing works for point/rect/lasso tools
- [x] Performance target: <1ms for 100K marks
- [x] CPU fallback works when no InteractionSystem is available
- [x] All tests pass
- [x] Example demonstrates large-dataset selection

## Implementation Summary

### What Was Implemented

- **GPU hit testing integration**: `MarkSelectionSystem` now supports
  GPU-accelerated point, rectangle, and lasso hit testing via
  `InteractionSystem`
- **Position tracking**: `set_positions()` and `set_positions_with_sizes()`
  register mark positions for both CPU and GPU hit testing
- **CPU fallback**: `hit_test()`, `rect_hit_test()`, and `lasso_hit_test()`
  provide CPU-based alternatives
- **GPU methods**: `hit_test_gpu()`, `rect_hit_test_gpu()`, and
  `lasso_hit_test_gpu()` dispatch to GPU compute shaders
- **Auto-fallback**: `hit_test_auto()`, `rect_hit_test_auto()`, and
  `lasso_hit_test_auto()` select GPU or CPU automatically
- **Lasso optimisation**: GPU lasso queries use bounding-rect candidate
  filtering then CPU point-in-polygon refinement
- **ElementDataRenderable adapter**: Bridges `MarkSelectionSystem` position data
  to the `Renderable` trait required by `InteractionSystem`

### Key Files Changed

| File                                 | Changes                                                                                                                           |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `src/mark_selection.rs`              | +483 lines: position tracking, CPU/GPU hit testing, `ElementDataRenderable` adapter, 10 new unit tests                            |
| `tests/gpu_selection_hit_testing.rs` | New file: 13 GPU integration tests covering point/rect/lasso, CPU fallback, auto-switching, 100K benchmarks, CPU↔GPU consistency |
| `examples/gpu_selection_demo.rs`     | New file: 50K-mark interactive demo with GPU hit testing, all three selection tools, timing display                               |

### Test Counts

- 10 new unit tests in `mark_selection.rs` (56 total in module)
- 13 new GPU integration tests in `gpu_selection_hit_testing.rs`
- All 1207+ existing tests continue to pass
