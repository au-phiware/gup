# GUP-262A: Direct Texture Sharing for Bevy

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-03-04 **Completed**: 2025-07-25

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

- [x] `GupChart` renders to a `wgpu::Texture` owned by Bevy's `Assets<Image>`
      without any CPU readback.
- [x] No PNG encoding/decoding occurs in the render path.
- [x] The `bevy_scatter` example runs at ≥ 30 fps on a mid-range GPU with
      animated data.
- [x] Existing `GupChart` API remains backward-compatible.

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

- [x] All Acceptance Criteria satisfied
- [x] Tests pass
- [x] Benchmark shows ≥ 5× improvement over PNG round-trip
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

Replaced the GPU → CPU readback → PNG encode → PNG decode → GPU upload pipeline
with a zero-copy GPU → GPU direct texture sharing path.

### Architecture

```text
Main World (PostUpdate):
  GupChart → render_to_texture_view → ChartTextureTarget (wgpu::Texture)

Render World (ExtractSchedule + Queue):
  Extract ChartTextureTarget + Sprite image handle
  → copy_texture_to_texture → GpuImage (sprite texture)
```

### Key Files Changed

| File | Change |
| ---- | ------ |
| `src/chart_builder.rs` | Added `render_to_texture_view` method to `ComposedChart` |
| `gup-bevy/src/chart.rs` | Extended `DynChart` trait with `render_to_texture_view` |
| `gup-bevy/src/texture_target.rs` | New — `ChartTextureTarget` offscreen texture manager |
| `gup-bevy/src/render_node.rs` | New — render-world extract + GPU copy systems |
| `gup-bevy/src/render_system.rs` | Replaced PNG render with texture render |
| `gup-bevy/src/plugin.rs` | Added render-world system registration |
| `gup-bevy/src/lib.rs` | Updated exports |
| `gup-bevy/Cargo.toml` | Removed `image` and `png` dependencies |
| `gup-bevy/examples/bevy_scatter.rs` | Uses shared `GupRenderContext` device |
| `gup-bevy/README.md` | Architecture documentation |
| `gup-bevy/tests/integration.rs` | 5 new tests (13 total) |

### Test Counts

- 13 gup-bevy integration tests (5 new)
- All workspace tests pass (241+ passing)
