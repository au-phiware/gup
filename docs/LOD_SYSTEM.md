# LOD System

The Level-of-Detail (LOD) system enables rendering of billion-point datasets at
interactive frame rates by maintaining a pyramid of progressively coarser
representations and adaptively selecting the appropriate tier each frame.

## Architecture

### LOD Pyramid (`gup::lod`)

The `LodPyramid` stores multiple levels of a point dataset:

- **Level 0**: full-resolution source data.
- **Level N** (coarsest): fewest points, produced by grid-based spatial
  aggregation.

Each level is stored in a `GpuBuffer<VertexData>` and has metadata (point count,
cell size, spatial bounds).

Build a pyramid:

```rust
let pyramid = LodPyramidBuilder::new()
    .levels(5)
    .max_gpu_bytes(512 * 1024 * 1024)
    .build_cpu(&device, &queue, &data)?;
```

### Adaptive Renderer (`gup::renderer`)

The `AdaptiveRenderer` orchestrates per-frame tier selection:

1. **Tier selection** — a density heuristic picks the finest tier whose on-screen
   point density is ≤ 1 point per pixel. Finer tiers are preferred when zoomed
   in; coarser tiers are used when zoomed out.
2. **Blend transitions** — `LodBlendState` cross-fades between tiers over a
   configurable number of frames (default: 8) to avoid visual popping.
3. **Viewport culling** — `ViewportCuller` runs a GPU compute shader pipeline
   that discards off-screen points and produces a compacted output buffer with an
   indirect draw argument buffer. No CPU readback occurs.

### Viewport Culler (`gup::renderer::ViewportCuller`)

A five-pass GPU compute pipeline:

1. **Cull** — mark each point as visible or culled based on viewport bounds.
2. **Per-workgroup prefix sum** — Hillis-Steele scan within each 256-thread
   workgroup.
3. **Block scan** — scan the per-workgroup totals (two-level for > 65K points).
4. **Add offsets** — propagate block offsets to per-element prefix sums.
5. **Compact** — write visible points to a dense output buffer.

Supports up to 16.7M points (256³) in a single dispatch.

## Data Flow

```text
LodPyramid
   │
   ├─ Level 0 (finest) ──┐
   ├─ Level 1             │
   ├─ Level 2             ├── AdaptiveRenderer.select_tier(viewport)
   ├─ Level 3             │        ↓
   └─ Level 4 (coarsest) ─┘   selected tier
                                   ↓
                          ViewportCuller.dispatch(tier_buffer, bounds)
                                   ↓
                          compacted output + draw_indirect
                                   ↓
                          render pass (indirect draw)
```

## Usage

```rust
use gup::renderer::{AdaptiveRenderer, AdaptiveRendererConfig, AdaptiveViewport, ViewportCuller};

let config = AdaptiveRendererConfig {
    blend_frames: 8,
    heuristic_scale: 1.0,
};
let mut renderer = AdaptiveRenderer::new(&pyramid, config);
let culler = ViewportCuller::new(&device)?;

// Per frame:
let viewport = AdaptiveViewport::new(zoom, pan, screen_size);
let frame = renderer.update(&viewport);

let tier = frame.tier;
let bounds = viewport.world_bounds();
let result = culler.dispatch(
    &device, &queue,
    pyramid.buffer(tier).buffer(),
    pyramid.level_point_count(tier) as u32,
    1, // vertex_count
    [bounds[0], bounds[2], bounds[1], bounds[3]],
).await?;

// Use result.draw_indirect_buffer for indirect draw.
```

## Debug Overlay

Enable with `renderer.set_debug_overlay(true)` to collect per-frame info:

- Current LOD tier (e.g., "LOD 3/6")
- Visible point count after culling
- Total points in tier
- Blend state

## Related Stories

- **GUP-256**: LOD Pyramid (data structure and build pipeline)
- **GUP-257**: Adaptive Viewport Renderer (this system)
- **GUP-258**: Streaming Data Manager (incremental LOD updates)
