# GUP-315: 3D Axis and Grid

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2026-03-04

## Context

GUP-261 delivered the camera, depth buffer, and 3D mark types (Sphere3D, Box3D,
Line3D). A natural next step is to render 3D axis lines, tick labels, and a
ground-plane grid so that users can orient themselves within the 3D scene. The
`Line3D` mark and camera uniform from GUP-261 provide the foundation; this story
adds the higher-level axis/grid layout logic.

## User Story

> "As a visualization developer, I want 3D axis lines and a ground grid so that
> my 3D scatter plots have spatial reference and viewers can judge depth and
> scale at a glance."

## Acceptance Criteria

- [ ] An `Axis3D` struct generates axis-line `Line3D` instances along X, Y, Z
      with configurable length and colour
- [ ] Tick marks are placed at regular intervals with optional labels (using the
      existing `Text` mark projected into 3D space)
- [ ] A `Grid3D` struct generates a ground-plane grid as `Line3D` instances
- [ ] Both integrate with the `Camera` uniform from GUP-261

## Technical Tasks

- [ ] Add `src/axis3d.rs` with `Axis3D` and `Grid3D` structs
- [ ] Generate `Line3D` instances for axis lines and grid lines
- [ ] Add `examples/scatter_3d_with_axes.rs` extending scatter_3d with axes
- [ ] Write unit tests for axis and grid line generation

## Dependencies

### Prerequisite Stories

- GUP-261: 3D Visualization Support ✅ — provides Camera, Line3D, DepthBuffer

## Testing Strategy

- Unit tests for axis/grid line generation and placement
- Visual validation via example

## Risk Assessment

- **Low**: Projecting text labels into 3D space may require billboard text or
  screen-space text overlay. Start with simple axis lines; labels can follow.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Examples compile and run
- [ ] Story status updated in INDEX.md
