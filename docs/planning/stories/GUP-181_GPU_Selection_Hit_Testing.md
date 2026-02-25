# GUP-181: GPU-Accelerated Selection Hit Testing

**Status**: 📋 Planned **Priority**: Medium **Effort**: 5 **Dependencies**:
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

1. `MarkSelectionSystem` can optionally hold a reference to `InteractionSystem`
2. Point hit tests are dispatched to GPU when `InteractionSystem` is available
3. Rectangle and lasso selections use spatial index for candidate filtering
4. Hit testing latency stays under 1ms for 100K points
5. Fallback to CPU hit testing when GPU is not available
6. Integration example demonstrating large-dataset selection

## Technical Tasks

- [ ] Add optional `InteractionSystem` integration to `MarkSelectionSystem`
- [ ] Wire up `query_point` for hover and click events
- [ ] Wire up `query_region` for rectangle selection
- [ ] Implement async hit test result handling in event loop
- [ ] Add benchmark test for 100K-point selection latency
- [ ] Create large-dataset selection example

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

- [ ] GPU-accelerated hit testing works for point/rect/lasso tools
- [ ] Performance target: <1ms for 100K marks
- [ ] CPU fallback works when no InteractionSystem is available
- [ ] All tests pass
- [ ] Example demonstrates large-dataset selection
