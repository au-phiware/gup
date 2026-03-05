# GUP-315: 3D Axis and Grid

## Story Overview

**Initiative**: Advanced Scale **Status**: ✅ Complete **Created**: 2026-03-04

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

- [x] An `Axis3D` struct generates axis-line `Line3D` instances along X, Y, Z
      with configurable length and colour
- [x] Tick marks are placed at regular intervals with optional labels (using the
      existing `Text` mark projected into 3D space)
- [x] A `Grid3D` struct generates a ground-plane grid as `Line3D` instances
- [x] Both integrate with the `Camera` uniform from GUP-261

## Technical Tasks

- [x] Add `src/axis3d.rs` with `Axis3D` and `Grid3D` structs
- [x] Generate `Line3D` instances for axis lines and grid lines
- [x] Add `examples/scatter_3d_with_axes.rs` extending scatter_3d with axes
- [x] Write unit tests for axis and grid line generation

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

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass
- [x] Examples compile and run
- [x] Story status updated in INDEX.md

## Implementation Summary

### What was implemented

- **`src/axis3d.rs`**: New module with `Axis3D`, `Axis3DConfig`, `Grid3D`,
  `Grid3DConfig`, and `TickLabel3D` types
- **`Axis3D`**: Generates coloured X/Y/Z axis lines as `Line3DInstance` data
  with configurable origin, length, width, and per-axis colours. Includes
  perpendicular tick marks at regular intervals.
- **`Grid3D`**: Generates a ground-plane (XZ) grid as `Line3DInstance` data with
  configurable extent, spacing, colour, and Y offset.
- **`TickLabel3D`**: Data struct for optional tick labels — provides world-space
  position, formatted text, and axis index for use with text rendering.
- **`examples/scatter_3d_with_axes.rs`**: Full interactive example combining
  Sphere3D data points with Axis3D and Grid3D, sharing a camera uniform for
  orbit animation. Renders at 1000+ FPS with 800 spheres + 45 line instances.

### Key files changed

| File                               | Change                                    |
| ---------------------------------- | ----------------------------------------- |
| `src/axis3d.rs`                    | New — Axis3D, Grid3D, TickLabel3D structs |
| `src/lib.rs`                       | Added `pub mod axis3d` declaration        |
| `examples/scatter_3d_with_axes.rs` | New — interactive 3D scatter + axes demo  |

### Test counts

- 15 unit tests (axis geometry, colours, origin, ticks, grid lines, edge cases,
  tick labels)
- 1 doc test

## Retrospective

**Completed**: 2025-07-22

### Key Technical Learnings

#### Line3D Pipeline Integration

- **Challenge**: Setting up a second render pipeline (Line3D) alongside the
  existing Sphere3D pipeline in a single render pass, sharing the camera uniform
  buffer.
- **Solution**: The Line3D shader uses a simpler bind group layout (no light
  uniform at group 1 binding 1), so a separate pipeline layout is needed. Both
  pipelines share the same camera buffer via their respective bind groups.
- **Pattern**: When multiple 3D mark types share a scene, each gets its own
  pipeline but they share uniform buffers. Draw calls are sequenced within one
  render pass.

#### Perpendicular Tick Mark Generation

- **Challenge**: Generating tick marks perpendicular to arbitrary axis
  directions in 3D space.
- **Solution**: Used cross-product with a non-parallel reference vector to find
  a perpendicular pair. For axes aligned with X, the reference swaps to Y to
  avoid degenerate cross products.
- **Pattern**: `perpendicular_pair()` is a reusable helper for any scenario
  needing an orthonormal frame from a single direction vector.

### Architectural Decisions

#### Data Generation vs GPU Generation

- **Decision**: Generate axis/grid `Line3DInstance` data on the CPU and upload
  once, rather than using a compute shader.
- **Reasoning**: Axis and grid geometry is static (or changes only on
  configuration change), so GPU compute would add complexity for no benefit. CPU
  generation is trivial and the data is small (tens of instances).
- **Trade-off**: If axes needed to update every frame (e.g. dynamic range), GPU
  generation would be more efficient.
- **Future**: Dynamic axis ranges (e.g. auto-scaling to data bounds) could be
  added by regenerating instances when the data range changes.

#### TickLabel3D as Data-Only Struct

- **Decision**: Provide `TickLabel3D` as a data struct with world-space position
  and text, rather than integrating directly with the `TextRenderer`.
- **Reasoning**: The story's risk assessment noted that 3D text projection
  (billboard text or screen-space overlay) is a separate concern. Providing the
  data decouples axis logic from text rendering specifics.
- **Trade-off**: Users must handle text rendering themselves. A future story
  could provide a convenience method that projects labels and feeds them to
  `TextRenderer`.
- **Future**: Enables GUP-373 (Billboard Text Labels) to consume this data.

### Development Workflow Insights

- The existing `scatter_3d` example was an excellent template — following its
  pattern for pipeline setup, bind groups, and the orbit camera made the new
  example straightforward.
- Duplicate story ID (GUP-315) existed for both "Graph Node Label Rendering" and
  "3D Axis and Grid". The 3D Axis and Grid variant was implemented here. The
  duplicate should be renumbered.
- Running the example produced 1000+ FPS, confirming that 45 additional line
  instances have negligible performance impact alongside 800 sphere instances.

### Follow-up Stories

1. **GUP-373: Billboard Text Labels for 3D Axes** — Integrate `TextRenderer`
   with `TickLabel3D` data to render axis value labels as camera-facing text in
   3D scenes. Would consume the `generate_tick_labels()` API from this story.
2. **GUP-374: Duplicate Story ID Cleanup** — Renumber the second GUP-315 (Graph
   Node Label Rendering) to avoid ID collisions in the story index.
