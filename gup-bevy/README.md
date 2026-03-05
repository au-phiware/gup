# gup-bevy

Bevy integration for the [Gup](https://github.com/au-phiware/gup)
GPU-accelerated data visualization library.

## Version Compatibility

| gup-bevy | Bevy | wgpu |
| -------- | ---- | ---- |
| 0.1      | 0.17 | 26.x |

## Architecture

`GupPlugin` shares Bevy's wgpu `Device`/`Queue` with Gup — no second GPU adapter
is created. Charts render into offscreen textures which are GPU-copied directly
into Bevy's `GpuImage` for sprites. The render path involves **zero CPU
readback** and no PNG encoding.

```text
GupChart  ──render──▶  ChartTextureTarget  ──GPU copy──▶  GpuImage (Sprite)
              (main world)                        (render world)
```

## Quick Start

```rust
use bevy::prelude::*;
use gup_bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GupPlugin)
        .add_systems(Startup, setup)
        .run();
}
```

See [`docs/BEVY_INTEGRATION.md`](../docs/BEVY_INTEGRATION.md) for the full
integration guide.

## License

GPL-3.0-or-later
