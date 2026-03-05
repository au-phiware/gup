# GUP-314: Windowed Treemap Rendering Example

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**: 2025-07-18

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

- [ ] A winit-based example renders treemap cells as actual Rectangle marks.
- [ ] Cells are coloured by depth or value, switchable at runtime (e.g., key
      press).
- [ ] The window supports resize and re-layouts the treemap to fit the new
      viewport.
- [ ] Screenshot comparison test added for visual regression.
- [ ] Works with all four treemap algorithm variants (switchable via CLI or
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

- [ ] All Acceptance Criteria satisfied
- [ ] Example compiles and runs: `cargo run --example treemap_window`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
