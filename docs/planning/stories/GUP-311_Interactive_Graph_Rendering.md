# GUP-311: Interactive Force-Directed Graph Rendering

## Story Overview

**Initiative**: Advanced Scale **Status**: ✅ Complete **Created**: 2025-07-19
**Completed**: 2025-07-20

## Context

GUP-259 provides an async layout engine that computes final positions but does
not render the graph interactively. This story adds real-time rendering of
force-directed layouts where nodes and edges are drawn each frame as the layout
converges, with support for node dragging, zooming, and streaming layout
updates.

## User Story

> "As a visualization user, I want to see the graph layout animate in real-time
> and drag nodes to manually adjust positions."

## Acceptance Criteria

- [x] A windowed example renders nodes (circles) and edges (lines) updating each
      frame as the layout converges
- [x] Node dragging with mouse allows pinning a node to a fixed position
- [x] Pan and zoom with mouse wheel / drag
- [x] The layout runs incrementally (a few iterations per frame) so the UI
      remains responsive
- [x] Node and edge counts are displayed as an overlay

## Dependencies

### Prerequisite Stories

- GUP-259: GPU Force-Directed Graph Layout ✅

## Testing Strategy

- Visual validation: run the interactive example and verify smooth animation
- Unit tests for incremental layout stepping
- Integration test for node pinning

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass
- [x] Lint clean
- [x] Retrospective added

## Implementation Summary

### What Was Implemented

1. **Incremental layout session API** (`src/layout/engine.rs`):
   - `LayoutSession` struct holding GPU buffers for a running simulation
   - `LayoutEngine::create_session()` to initialise buffers from graph data
   - `LayoutEngine::step()` to advance N iterations on the GPU
   - `LayoutEngine::read_positions()` to read back current node coordinates
   - `LayoutEngine::pin_node()` to write a node's position and zero velocity

2. **Interactive graph example** (`examples/interactive_graph.rs`):
   - 200-node random graph rendered with Circle (nodes) and Line (edges) marks
   - Incremental layout: 3 iterations per frame via `LayoutSession`
   - Node dragging via `pin_node()` on mouse drag
   - Pan/zoom via `ZoomBehavior` on background drag and scroll wheel
   - Node/edge count and iteration status in window title
   - Space to restart, R to reset view, Q/Escape to quit

### Key Files Changed

| File | Change |
|------|--------|
| `src/layout/engine.rs` | Added `LayoutSession`, `create_session()`, `step()`, `read_positions()`, `pin_node()` |
| `src/layout.rs` | Exported `LayoutSession` |
| `examples/interactive_graph.rs` | New: 710-line interactive windowed example |
| `tests/layout_integration.rs` | Added 3 new GPU integration tests |

### Test Counts

- 3 new integration tests: `session_create_and_step`, `session_incremental_stepping`, `session_pin_node`
- All 21 layout tests pass
- Full suite: 3006+ tests pass
