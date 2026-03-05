# GUP-366: Choropleth GPU Render Pipeline Integration

## Story Overview

**Initiative**: Chart Builders
**Status**: 📋 Planned
**Created**: 2025-07-18

## Context

GUP-287 (GPU-Side Choropleth Recolouring) delivered the CPU-side data structures
for dynamic choropleth recolouring: `RegionColorBuffer`,
`IndexedChoroplethVertex`, and WGSL shaders that read per-region colours from a
GPU storage buffer. However, these components are not yet wired into a live wgpu
render pipeline.

This story creates the GPU execution path: bind group layouts, pipeline layouts,
buffer creation, and a render method that draws choropleth regions using the
storage-buffer colour lookup path rather than per-vertex colours.

## User Story

> "As a visualization developer, I want the GPU-side choropleth recolouring to
> work end-to-end with a real wgpu render pipeline, so that dynamic recolouring
> actually renders to screen at interactive frame rates."

## Acceptance Criteria

- [ ] A wgpu render pipeline is created using the `choropleth_recolor` vertex
      and fragment shaders.
- [ ] A bind group layout includes the uniform buffer at `@binding(0)` and the
      region colour storage buffer at `@binding(1)`.
- [ ] The `RegionColorBuffer` is uploaded to a GPU storage buffer and bound to
      the pipeline.
- [ ] Calling `update_colors()` followed by `queue.write_buffer()` dynamically
      recolours the rendered choropleth without re-creating any GPU resources.
- [ ] A visual example renders a choropleth and dynamically switches between
      two datasets.

## Dependencies

### Prerequisite Stories

- GUP-287: GPU-Side Choropleth Recolouring ✅

### Enables Stories

- GUP-288: Choropleth Tooltip and Hover Interaction

## Testing Strategy

- Integration test creating a headless wgpu pipeline with the recolour shaders
  and verifying no GPU validation errors.
- Visual test rendering a choropleth, updating colours, and comparing the output
  framebuffer before and after.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
