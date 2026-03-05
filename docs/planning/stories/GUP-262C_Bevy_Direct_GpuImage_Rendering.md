# GUP-262C: Bevy Render-Graph Node for Direct GpuImage Rendering

**Initiative**: Ecosystem Integration **Status**: 💡 New **Created**: 2025-07-25

## Overview

Replace the current offscreen texture + `copy_texture_to_texture` approach with
a Bevy render-graph node that renders Gup charts directly into `GpuImage`
textures. This eliminates the per-frame GPU copy overhead.

## Context

GUP-262A introduced direct texture sharing but still uses an intermediate
offscreen texture followed by a GPU blit. For maximum performance, the chart
should render directly into the sprite's `GpuImage` texture from within the
render world.

## User Story

As a game developer with performance-critical chart rendering, I want charts to
render directly into the Bevy sprite texture without any intermediate copies so
that per-chart overhead is minimised.

## Acceptance Criteria

- [ ] Chart draw commands execute directly against the `GpuImage` texture view —
      no intermediate texture or copy.
- [ ] The render-graph node handles pipeline/buffer extraction from the main
      world correctly.
- [ ] Performance improvement is measurable (< 1ms per chart per frame for an
      800×600 chart).
- [ ] Backward-compatible with the existing `GupChart` API.

## Technical Tasks

1. Design an `ExtractedChartResources` struct to hold Arc-wrapped wgpu buffers,
   pipelines, and bind groups.
2. Implement extraction in `ExtractSchedule` to copy references from the main
   world.
3. Create a Bevy render-graph node that:
   - Looks up the `GpuImage` for the chart's sprite.
   - Creates a render pass targeting the `GpuImage.texture_view`.
   - Issues chart draw commands from `ExtractedChartResources`.
4. Wire the node into the render graph (after `PrepareAssets`).
5. Remove the offscreen `ChartTextureTarget` and `copy_texture_to_texture`
   system.

## Dependencies

- GUP-262A ✅ (extends the texture sharing architecture)

## Testing Strategy

- Benchmark comparison: measure per-frame time with and without intermediate
  copy.
- Visual regression: output must match GUP-262A's output exactly.
- GPU validation layer: no errors.

## Risk Assessment

- **High**: Extracting chart GPU resources (buffers, pipelines, bind groups) to
  the render world requires exposing internal `ComposedChart` state or creating
  a new abstraction layer. This may require changes to the core `gup` crate.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] Tests pass
- [ ] Benchmark shows measurable improvement over GUP-262A's copy approach
- [ ] Documentation updated
