# Bevy Integration Guide

Embed a Gup chart in a [Bevy](https://bevyengine.org/) app as a sprite texture,
rendered on Bevy's own GPU device and updated as your data changes.

## Version Compatibility

| gup-bevy | Bevy | wgpu |
| -------- | ---- | ---- |
| 0.1      | 0.18 | 27.x |

Bevy's wgpu version must match `gup-core`'s; `Cargo.lock` holds one `wgpu`.

## Adding the Dependency

```toml
[dependencies]
gup-core = { git = "https://github.com/au-phiware/gup" }
gup-bevy = { git = "https://github.com/au-phiware/gup" }
bevy = { version = "0.18", default-features = false, features = [
    "2d_bevy_render", "bevy_asset", "bevy_log", "bevy_window", "bevy_winit",
    "x11", "png", # replace with your platform feature
] }
```

## Minimal Example

```rust,no_run
use bevy::prelude::*;
use gup_bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, GupPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let size = UVec2::new(480, 320);
    let image = images.add(chart_image(size, 1.0));
    let chart = gup_core::scatter(vec![(1.0, 2.0), (2.0, 3.0)])
        .x(|p| p.0)
        .y(|p| p.1)
        .title("Hello Bevy");
    commands.spawn((GupChart::new(chart, size, image.clone()), Sprite::from_image(image)));
}
```

`examples/bevy_scatter.rs` extends a scatter every frame through an `Appender`;
run it with `cargo run -p gup-bevy --example bevy_scatter`.

## How It Works

- **`GupPlugin`**: in `finish` it wraps Bevy's `RenderDevice` and queue with
  `gup_core::Context::from_wgpu`, stored as the `GupContext` resource. No second
  adapter is created.
- **`GupChart`** (component): the chart (any `gup_core::Chart + Send + Sync`;
  the builders and `Plot` are), its logical `size`, `dpr` and the `image`
  handle. `chart.as_any_mut().downcast_mut::<ScatterChart<_>>()` reaches the
  concrete type.
- **`chart_image(size, dpr)`**: a blank `Rgba8UnormSrgb` image of `size * dpr`
  pixels with no CPU data. The chart's texture is also `Rgba8UnormSrgb`, and Gup
  draws through its `Rgba8Unorm` view, so colours match `ChartExt::render` byte
  for byte.
- **Per frame**: a `PostUpdate` system resolves and draws each chart (a chart
  with no changes writes nothing new); extraction hands the texture to the
  render world, where a system in `RenderSystems::Queue` copies it into the
  image's `GpuImage` (`copy_texture_to_texture`).

## Updating Chart Data

Keep an `Appender` from the chart before spawning it (`chart.appender()`) and
`push` rows from any system or thread. Rows pushed between frames are uploaded
together at the next resolve.

## Known Limitations

- The copy runs in the render world while the main world draws the next frame
  into the same texture: a frame may show the newer chart, never a torn one
  (both are ordered on one queue).
- Charts are not drawn into Bevy's own pass, and Bevy input is not forwarded to
  `Chart::handle`; both are possible follow-ups.
- Bevy is a heavy dependency, so CI lints and tests the crate in a separate
  `integrations` job.
