# gup-bevy

Bevy integration for the [Gup](https://github.com/au-phiware/gup) GPU-accelerated
data visualization library.

## Version Compatibility

| gup-bevy | Bevy  | wgpu  |
| -------- | ----- | ----- |
| 0.1      | 0.17  | 26.x  |

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
