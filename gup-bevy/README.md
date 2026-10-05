# gup-bevy

Bevy integration for the [Gup](https://github.com/au-phiware/gup)
GPU-accelerated data visualization library.

> **Parked (GUP-390).** This crate is excluded from the Gup workspace and CI
> until RFC-001 step S13 re-wires it onto `gup-core`. Build it with
> `CARGO_TARGET_DIR=target cargo check --manifest-path gup-bevy/Cargo.toml` from
> the repository root. See the root README, "Parked integration crates".

## Version Compatibility

| gup-bevy | Bevy | wgpu |
| -------- | ---- | ---- |
| 0.1      | 0.18 | 27.x |

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
