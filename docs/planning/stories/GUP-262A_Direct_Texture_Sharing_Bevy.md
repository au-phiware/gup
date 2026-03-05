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

| File                                | Change                                                   |
| ----------------------------------- | -------------------------------------------------------- |
| `src/chart_builder.rs`              | Added `render_to_texture_view` method to `ComposedChart` |
| `gup-bevy/src/chart.rs`             | Extended `DynChart` trait with `render_to_texture_view`  |
| `gup-bevy/src/texture_target.rs`    | New — `ChartTextureTarget` offscreen texture manager     |
| `gup-bevy/src/render_node.rs`       | New — render-world extract + GPU copy systems            |
| `gup-bevy/src/render_system.rs`     | Replaced PNG render with texture render                  |
| `gup-bevy/src/plugin.rs`            | Added render-world system registration                   |
| `gup-bevy/src/lib.rs`               | Updated exports                                          |
| `gup-bevy/Cargo.toml`               | Removed `image` and `png` dependencies                   |
| `gup-bevy/examples/bevy_scatter.rs` | Uses shared `GupRenderContext` device                    |
| `gup-bevy/README.md`                | Architecture documentation                               |
| `gup-bevy/tests/integration.rs`     | 5 new tests (13 total)                                   |

### Test Counts

- 13 gup-bevy integration tests (5 new)
- All workspace tests pass (241+ passing)

## Retrospective

**Completed**: 2025-07-25

### Key Technical Learnings

#### Bevy Render World Architecture

- **Challenge**: Bevy 0.17 splits the ECS into a main world and a render world.
  GPU resources like `GpuImage` textures live exclusively in the render world
  and cannot be accessed from main-world systems.
- **Solution**: Two-phase approach: main world renders charts to an offscreen
  `wgpu::Texture`, then a render-world system uses `copy_texture_to_texture` to
  copy the result into the `GpuImage` backing the sprite.
- **Pattern**: When integrating GPU rendering libraries with Bevy, always use
  the Extract schedule to bridge between worlds. Store wgpu handles (which are
  internally Arc-referenced) in components for cheap cross-world transfer.

#### wgpu Device Identity and Texture Sharing

- **Challenge**: Charts built with `RenderContext::new()` allocate a fresh wgpu
  device. Textures created on one device cannot be used in render passes or
  copies targeting textures on a different device — wgpu panics with "does not
  exist" errors.
- **Solution**: The `bevy_scatter` example (and any user code) must build charts
  using `GupRenderContext::render_context()` so every GPU resource lives on
  Bevy's shared device.
- **Pattern**: In Bevy integrations, always propagate the shared
  `Device`/`Queue` through the chart-builder API. Never create a second adapter
  for chart rendering.

#### Bevy Image Without CPU Data

- **Challenge**: `Image::new_fill()` creates CPU pixel data that gets uploaded
  to the GPU. For texture-sharing we never need CPU data — the chart writes
  directly to the GPU texture.
- **Solution**: `Image::new_uninit()` (Bevy 0.17) creates an `Image` with
  `data: None`. Bevy's `GpuImage::prepare_asset` creates the GPU texture without
  uploading anything. The texture is then ready for `copy_texture_to_texture`.
- **Pattern**: Use `new_uninit` + `COPY_DST` usage for render targets that are
  only ever written to by the GPU.

### Architectural Decisions

#### Offscreen Texture + GPU Copy vs. Direct GpuImage Rendering

- **Decision**: Render to a `ChartTextureTarget` then `copy_texture_to_texture`
  into the `GpuImage`, rather than rendering directly into the GpuImage texture.
- **Reasoning**: Direct rendering into the GpuImage requires either (a) access
  to the GpuImage from the main world (impossible) or (b) extracting the entire
  chart's GPU resources (buffers, pipelines) to the render world (very complex).
  The copy approach requires only a single GPU blit per chart per frame.
- **Trade-off**: One extra GPU copy per chart per frame (sub-millisecond for
  800×600). The simplicity gain is substantial.
- **Future**: If profiling shows the extra copy is a bottleneck, a render-graph
  node could render directly into the GpuImage by extracting pre-prepared
  buffer/pipeline handles.

#### Removing image/png Dependencies

- **Decision**: Removed the `image` crate and Bevy's `png` feature from gup-bevy
  dependencies.
- **Reasoning**: The PNG path is no longer used in the render system. The
  `render_to_png` method still exists on `DynChart`/`GupChart` for screenshot
  exports, but it uses the main `gup` crate's PNG infrastructure directly.
- **Trade-off**: If users relied on Bevy's PNG feature transitively through
  gup-bevy, they would need to enable it themselves. Unlikely since gup-bevy is
  a niche integration crate.
- **Future**: Could re-add as an optional feature if users request it.

### Development Workflow Insights

- **Bevy 0.17 API discovery**: Without local docs, examining the crate source
  directly in `~/.cargo/registry/src/` was the fastest way to understand
  `GpuImage`, `RenderAssets`, and `Extract<Query<>>`.
- **wgpu device identity**: The cross-device "TextureView does not exist" panic
  appeared early in testing and was resolved by ensuring the test helper shared
  the same `RenderContext` between chart and texture creation.
- **Pre-existing lint**: The `mask all-fix` check surface pre-existing markdown
  lint in other story files. These were correctly ignored since they are not
  related to this story's changes.

### Follow-up Stories

1. **GUP-262C: Bevy Render-Graph Node for Direct GpuImage Rendering** — Replace
   the current offscreen texture + copy approach with a Bevy render-graph node
   that renders Gup charts directly into GpuImage textures, eliminating the per-
   frame GPU copy. Requires extracting chart GPU resources (buffers, pipelines)
   to the render world.
2. **GUP-262D: Multi-Chart Batched Rendering** — When multiple `GupChart`
   entities exist, batch their render commands into a single command encoder
   submission rather than submitting separately per chart. Reduces GPU
   synchronisation overhead for dashboards with many charts.
