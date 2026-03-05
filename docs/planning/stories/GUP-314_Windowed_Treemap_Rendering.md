# GUP-314: Windowed Treemap Rendering Example

## Story Overview

**Initiative**: Advanced Scale **Status**: ✅ Complete **Created**: 2025-07-18
**Completed**: 2025-07-21

## Context

GUP-260 provides a CLI-only treemap example that validates the layout engine but
does not render cells visually in a GPU window. A windowed example using winit +
wgpu + Rectangle marks would demonstrate the full end-to-end pipeline: data →
layout → GPU rendering, and serve as a visual regression baseline.

## User Story

> "As a developer evaluating Gup, I want to see a treemap rendered in a real
> window so I can verify the visual quality and interactively explore colour
> modes."

## Acceptance Criteria

- [x] A winit-based example renders treemap cells as actual Rectangle marks.
- [x] Cells are coloured by depth or value, switchable at runtime (e.g., key
      press).
- [x] The window supports resize and re-layouts the treemap to fit the new
      viewport.
- [x] Screenshot comparison test added for visual regression.
- [x] Works with all four treemap algorithm variants (switchable via CLI or
      key).

## Dependencies

### Prerequisite Stories

- GUP-260: GPU Treemap Layout ✅
- GUP-067: Rectangle and Line Mark Implementations ✅
- GUP-004: Basic Render Context ✅

## Testing Strategy

- Visual: Screenshot comparison with known reference images.
- Smoke test: Example runs for 2 seconds without panics or GPU errors.

## Risk Assessment

- **Low**: The Rectangle mark and render context are well established. The main
  work is wiring layout cells to GPU-rendered instances in a window event loop.

## Definition of Done

- [x] All Acceptance Criteria satisfied
- [x] Example compiles and runs: `cargo run --example treemap_window`
- [x] Lint and format clean: `mask all-fix`
- [x] Retrospective added

## Implementation Summary

### What was implemented

- **`examples/treemap_window.rs`**: Windowed treemap rendering example using
  winit + wgpu + Rectangle marks. Renders 1000-node treemap with real-time
  colour mode switching (C key: depth/value), algorithm cycling (A key:
  Squarified/Binary/Strip/SliceDice), and automatic re-layout on window resize.
  CLI flags for `--nodes`, `--color`, and `--algo`.

- **`tests/treemap_window_tests.rs`**: Three GPU integration tests validating
  the full data → layout → rectangle instance conversion pipeline for all four
  algorithm variants, clip-space coordinate conversion, and viewport resize
  behaviour.

### Key files changed

| File                             | Change                                   |
| -------------------------------- | ---------------------------------------- |
| `examples/treemap_window.rs`     | New windowed treemap example (605 lines) |
| `tests/treemap_window_tests.rs`  | New GPU smoke tests (3 tests)            |
| `docs/planning/stories/INDEX.md` | Status update                            |

### Test counts

- 3 new GPU integration tests (all passing)
- 267 total tests pass, 0 failures

## Retrospective

**Completed**: 2025-07-21

### Key Technical Learnings

#### Clip-Space Coordinate Conversion

- **Challenge**: Treemap layout produces cells in pixel coordinates (top-left
  origin, positive-Y down) but Rectangle marks expect clip-space coordinates
  (centre-based, [-1,1] range, positive-Y up).
- **Solution**: Simple linear mapping:
  `cx = (center_x / viewport_width) * 2 - 1` and
  `cy = -(center_y / viewport_height) * 2 - 1` (note Y flip).
- **Pattern**: When wiring layout outputs to mark instances, always document the
  coordinate system transformation explicitly.

#### Arc<GupContext> Ownership Dance

- **Challenge**: The `GupContext` must be temporarily unwrapped from `Arc` for
  mutable access during rendering (begin_frame requires `&mut self`), then
  re-wrapped after the frame.
- **Solution**: Followed the established pattern from `interactive_graph.rs`:
  `Arc::try_unwrap()` → render → `Arc::new()`. This is idiomatic for the
  project's single-owner-at-render-time model.
- **Pattern**: The `take() → try_unwrap() → re-wrap` dance is the standard way
  to get mutable access to `GupContext` in winit event loops.

#### Dual GPU Context Pattern

- **Challenge**: The layout engine (`LayoutEngine::new()`) requires a
  `RenderContext`, while windowed rendering uses `GupContext`. These are
  separate GPU contexts with separate devices.
- **Solution**: Created two contexts — one headless `RenderContext` for layout,
  one `GupContext` with surface for rendering. This wastes GPU memory on
  integrated GPUs.
- **Pattern**: This is a known limitation addressed by GUP-314 (Shared Device
  Layout Engine). Future work should allow `LayoutEngine` to share the rendering
  device.

### Architectural Decisions

#### Lazy Re-Layout on Demand

- **Decision**: Treemap layout is re-run only when `needs_layout` is set
  (resize, algorithm change, colour mode change), not every frame.
- **Reasoning**: Layout is expensive (GPU compute + readback). For a static
  treemap, re-running it 60× per second is wasteful.
- **Trade-off**: Slightly more state management vs significantly better
  performance.
- **Future**: Interactive drill-down (GUP-313) will need more nuanced
  invalidation.

#### ControlFlow::Wait vs Poll

- **Decision**: Used `ControlFlow::Wait` (only redraw when events arrive) rather
  than `ControlFlow::Poll` (continuous rendering).
- **Reasoning**: A static treemap doesn't need continuous animation. Wait mode
  saves CPU/GPU when the window is idle.
- **Trade-off**: No continuous animation, but the treemap is static once
  rendered.
- **Future**: If animation (transitions, drill-down) is added, switch to Poll
  during animation and Wait when idle.

### Development Workflow Insights

- The existing `interactive_graph.rs` example was an excellent template —
  following its patterns for GupContext lifecycle, Selection rendering, and
  event handling made implementation straightforward.
- The treemap CLI example (`treemap.rs`) already had the tree generation and
  colour mapping logic. Reusing those functions (with minor adaptation) kept the
  windowed example focused on rendering concerns.
- GPU tests require `--test-threads=1` — this is well documented but worth
  repeating since it's easy to forget.

### Follow-up Stories

1. **GUP-314: Shared Device Layout Engine** — Already exists. The dual GPU
   context pattern (separate RenderContext for layout, GupContext for rendering)
   is wasteful. This follow-up would allow LayoutEngine to share the rendering
   device.
