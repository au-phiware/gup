# GUP-262A: Direct Texture Sharing for Bevy

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2025-03-04

## Overview

Eliminate the render-to-PNG round-trip in the Bevy integration by rendering
charts directly to a `wgpu::Texture` that is wrapped as a Bevy `Image` handle
with zero-copy.

## Context

GUP-262 implemented the initial Bevy integration using a render-to-PNG approach
(GPU → CPU readback → PNG encode → PNG decode → GPU upload). While correct and
simple, this round-trip is too expensive for animated charts at 60 fps.

## User Story

As a game developer embedding Gup charts in a Bevy application, I want charts to
render directly to GPU textures shared with Bevy so that animated chart updates
are performant (< 2 ms per chart per frame).

## Acceptance Criteria

- [ ] `GupChart` renders to a `wgpu::Texture` owned by Bevy's `Assets<Image>`
      without any CPU readback.
- [ ] No PNG encoding/decoding occurs in the render path.
- [ ] The `bevy_scatter` example runs at ≥ 30 fps on a mid-range GPU with
      animated data.
- [ ] Existing `GupChart` API remains backward-compatible.

## Dependencies

- GUP-262 ✅ (this story extends the initial integration)

## Testing Strategy

- Benchmark: render 10 charts at 60 fps, measure per-frame time.
- Visual regression: compare rendered output to PNG-based reference.
- GPU validation: no errors under wgpu validation layer.

## Risk Assessment

- **Medium**: Bevy's `Image` asset and GPU texture management may not expose a
  clean way to write to the underlying texture from outside the render graph.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] Tests pass
- [ ] Benchmark shows ≥ 5× improvement over PNG round-trip
- [ ] Documentation updated
