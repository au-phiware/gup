# GUP-373: Billboard Text Labels for 3D Axes

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-22

## Context

GUP-315 delivered `Axis3D` with tick marks and a `generate_tick_labels()` API
that produces `TickLabel3D` structs with world-space positions and formatted
text. However, the labels are data-only — they are not yet rendered as visible
text in the 3D scene. The project already has an SDF text rendering pipeline
(`TextRenderer`, `FontAtlas`) that can render text quads. This story bridges the
gap by projecting `TickLabel3D` positions into screen-space (or using billboard
quads) and feeding them to `TextRenderer`.

## User Story

> "As a visualization developer, I want axis tick values rendered as readable
> text in my 3D scatter plots so that viewers can interpret the scale of each
> axis."

## Acceptance Criteria

- [ ] `TickLabel3D` positions are projected to screen-space or rendered as
      camera-facing billboard quads
- [ ] Labels are readable at the default camera distance
- [ ] Labels do not overlap axis lines or each other at the default view
- [ ] A convenience function or builder method connects `Axis3D` tick labels to
      `TextRenderer`
- [ ] Performance: adding labels does not drop FPS below 30 for a 3-axis setup
      with 8 ticks per axis

## Technical Tasks

- [ ] Implement world-to-screen projection using the Camera view/projection
      matrices
- [ ] Batch `TickLabel3D` text through `TextRenderer` or create billboard text
      quads in the 3D pipeline
- [ ] Add label rendering to the `scatter_3d_with_axes` example
- [ ] Write tests for projection accuracy and label batch generation

## Dependencies

### Prerequisite Stories

- GUP-315: 3D Axis and Grid ✅ — provides `Axis3D`, `TickLabel3D`
- GUP-261: 3D Visualization Support ✅ — provides Camera, CameraUniform

## Testing Strategy

- Unit tests for world-to-screen projection
- Visual validation via updated example
- Performance measurement with label rendering on/off

## Risk Assessment

- **Medium**: Billboard text may require additional shader work (camera-facing
  quads with text atlas sampling). Screen-space overlay is simpler but may not
  track 3D positions correctly during rotation.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Example compiles and runs with visible labels
- [ ] Story status updated in INDEX.md
