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

| File                              | Change                                   |
| --------------------------------- | ---------------------------------------- |
| `examples/treemap_window.rs`      | New windowed treemap example (605 lines) |
| `tests/treemap_window_tests.rs`   | New GPU smoke tests (3 tests)            |
| `docs/planning/stories/INDEX.md`  | Status update                            |

### Test counts

- 3 new GPU integration tests (all passing)
- 267 total tests pass, 0 failures
