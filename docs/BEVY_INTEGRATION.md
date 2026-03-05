# Bevy Integration Guide

This guide shows how to embed GPU-accelerated Gup data visualizations inside a
[Bevy](https://bevyengine.org/) application.

## Version Compatibility

| gup-bevy | Bevy | wgpu |
| -------- | ---- | ---- |
| 0.1      | 0.18 | 27.x |
| 0.1      | 0.17 | 26.x |

> **Note**: Bevy's wgpu version must match the version used by the `gup` crate.
> Bevy 0.18 ships with wgpu 27.0, which is compatible.

## Adding the Dependency

Add `gup-bevy` to your project:

```toml
# Cargo.toml
[dependencies]
gup = { git = "https://github.com/au-phiware/gup" }
gup-bevy = { git = "https://github.com/au-phiware/gup" }
bevy = { version = "0.18", default-features = false, features = [
    "bevy_render",
    "bevy_asset",
    "bevy_winit",
    "bevy_sprite",
    "png",
    "x11",          # Linux/X11 — replace with your platform feature
] }
```

`gup-bevy` re-exports the Bevy-specific items you need through a prelude:

```rust
use gup_bevy::prelude::*;
```

## Minimal Example

```rust
use bevy::prelude::*;
use gup::prelude::*;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{scatter, AccessorFunction};
use gup::chart_builder::ChartBuilder;
use gup_bevy::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct DataPoint { x: f32, y: f32 }

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GupPlugin)            // 1. Add the Gup plugin
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

    // 2. Build a chart with the Gup chart-builder API
    let context = Arc::new(
        pollster::block_on(RenderContext::new()).unwrap(),
    );
    let data = vec![
        DataPoint { x: 1.0, y: 2.0 },
        DataPoint { x: 2.0, y: 4.0 },
        DataPoint { x: 3.0, y: 1.0 },
    ];
    let chart = scatter()
        .x(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.y)))
        .build_with_data(data, context)
        .unwrap();

    // 3. Wrap in GupChart and spawn with a Sprite
    let placeholder = blank_chart_image(800, 600);
    let image_handle = images.add(placeholder);

    commands.spawn((
        GupChart::new(chart).with_size(800, 600),
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(800.0, 600.0)),
            ..default()
        },
    ));
}
```

That's it — about **20 lines of user-written code** to get a GPU-accelerated
scatter plot rendering inside a Bevy window.

## How It Works

### GupPlugin

`GupPlugin` performs two actions:

1. **`build()`** — registers `gup_render_system` in the `PostUpdate` schedule.
2. **`finish()`** — extracts Bevy's `RenderDevice`, `RenderQueue`,
   `RenderAdapter`, and `RenderInstance` from the render sub-app and constructs
   a `GupRenderContext` resource. No second GPU adapter is created; both Bevy
   and Gup share the same device.

### GupChart Component

`GupChart` is a Bevy `Component` that wraps any chart built with the Gup
chart-builder API. It uses an object-safe `DynChart` trait internally for type
erasure, so you can store scatter plots, line charts, bar charts, etc.

Key fields:

| Field         | Type   | Description                                        |
| ------------- | ------ | -------------------------------------------------- |
| `auto_update` | `bool` | Re-render every frame when `true` (default: true). |
| `width`       | `u32`  | Pixel width of the offscreen render target.        |
| `height`      | `u32`  | Pixel height of the offscreen render target.       |

### gup_render_system

Each frame, `gup_render_system`:

1. Queries all `(GupChart, Sprite)` entities.
2. Skips charts that are not dirty (and not `auto_update`).
3. Renders dirty charts to PNG via `GupChart::render_to_png`.
4. Decodes the PNG into a Bevy `Image` asset.
5. Replaces the `Sprite`'s image handle so the on-screen sprite updates.

## Updating Chart Data at Runtime

To animate or update data, rebuild the chart and replace the `GupChart`
component:

```rust
fn animate_system(time: Res<Time>, mut charts: Query<&mut GupChart>) {
    let t = time.elapsed_secs();
    for mut chart in &mut charts {
        let new_chart = build_my_chart(t);
        *chart = GupChart::new(new_chart).with_size(800, 600);
    }
}
```

For one-shot updates (e.g. in response to user interaction), set
`auto_update: false` and call `mark_dirty()` when the data changes:

```rust
let mut gup_chart = GupChart::with_auto_update(chart, false);
// … later …
gup_chart.mark_dirty(); // triggers a single re-render
```

## Known Limitations and Caveats

1. **Bevy version lock** — `gup-bevy` targets Bevy 0.18 (wgpu 27). Upgrading to
   a different Bevy version requires matching wgpu versions.

2. **Render-to-PNG overhead** — The current implementation renders charts to PNG
   bytes and then loads the PNG as a Bevy `Image`. This involves a GPU→CPU→GPU
   round-trip. A future story will add direct texture sharing to eliminate this
   overhead.

3. **Single-threaded chart rendering** — Charts are rendered sequentially in the
   render system. Parallel chart rendering is not yet supported.

4. **Platform features** — The `bevy` dependency in `gup-bevy` enables `x11` by
   default. On other platforms, adjust the feature flags accordingly (e.g.
   `wayland` on Wayland Linux, no extra feature on macOS/Windows).

5. **No 3-D support** — Charts are rendered to 2-D textures displayed as Bevy
   sprites. 3-D scene integration (e.g. billboarded charts in a 3-D world) is
   out of scope for this initial integration.
