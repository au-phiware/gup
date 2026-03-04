# egui Integration Guide

This guide covers embedding GPU-accelerated Gup charts inside an
[egui](https://github.com/emilk/egui) application using the `gup-egui` crate.

## Adding the Dependency

Add `gup-egui` alongside `gup` in your `Cargo.toml`:

```toml
[dependencies]
gup      = { git = "https://github.com/au-phiware/gup" }
gup-egui = { git = "https://github.com/au-phiware/gup" }
eframe   = "0.33"
pollster = "0.3"
```

`gup-egui` re-exports `egui` so you can use its types directly; `eframe`
provides the native and web application shell.

## Minimal Code: Embedding a Chart

Three steps are needed to display a Gup chart in an egui panel:

### Step 1 — Build a chart

Use Gup's chart-builder API to construct a `ComposedChart`:

```rust
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::render::RenderContext;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct DataPoint { x: f32, y: f32 }

fn build_chart(data: Vec<DataPoint>)
    -> gup::chart_builder::ComposedChart<DataPoint, gup::mark::Circle>
{
    let ctx = Arc::new(
        pollster::block_on(RenderContext::new())
            .expect("GPU context"),
    );

    scatter()
        .x(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.y)))
        .point_size(6.0)
        .build_with_data(data, ctx)
        .expect("chart")
}
```

### Step 2 — Wrap in a GupWidget

```rust
use gup_egui::GupWidget;

let mut widget = GupWidget::new(build_chart(initial_data));
```

`GupWidget` takes ownership of the chart and manages the off-screen GPU texture,
dirty tracking, and egui texture upload internally.

### Step 3 — Display in a panel

Inside your `eframe::App::update` implementation:

```rust
egui::CentralPanel::default().show(ctx, |ui| {
    ui.add(&mut widget);
});
```

`GupWidget` implements `egui::Widget` for `&mut GupWidget`, so it can be passed
directly to `ui.add()`.

## Pushing Live Data Updates

When your data changes, replace the inner chart and the widget will
automatically re-render on the next frame:

```rust
widget.set_chart(build_chart(new_data));
```

Alternatively, if you have mutable access to the chart and update it in-place,
call:

```rust
widget.mark_dirty();
```

The widget only re-renders when dirty or when the panel size has changed.
Unchanged frames reuse the previously uploaded texture without issuing new GPU
draw calls.

## Retrieving Interaction Events

egui pointer events (hover, click, drag, scroll) are automatically translated
into Gup `InteractionEvent` types with coordinates mapped to the chart's
physical pixel space:

```rust
let events = widget.take_events();
for ev in &events {
    println!("{} at ({}, {})", ev.interaction_type,
             ev.screen_position.x, ev.screen_position.y);
}
```

Scroll events that egui does not consume are forwarded as `"scroll"` events with
`scroll_x` / `scroll_y` metadata.

## Complete Example

See `gup-egui/examples/egui_chart.rs` for a full application that:

- Opens an `eframe` window.
- Renders a live-updating scatter plot (data changes each second) in a
  `CentralPanel`.
- Shows interaction events in a `SidePanel`.

Run it with:

```bash
cargo run -p gup-egui --example egui_chart
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  egui / eframe (wgpu 27)                    │
│                                             │
│   ┌──────────────┐    ┌──────────────────┐  │
│   │  egui UI     │    │ TextureManager   │  │
│   │  ui.add(w)   │───▶│  ColorImage      │  │
│   └──────────────┘    └──────────────────┘  │
│          │                     ▲             │
│          │  Response           │ RGBA pixels │
│          ▼                     │             │
│   ┌──────────────┐    ┌───────┴──────────┐  │
│   │ Event Bridge │    │  PNG decode      │  │
│   │ egui→Gup     │    └───────┬──────────┘  │
│   └──────────────┘            │             │
└───────────────────────────────┼─────────────┘
                                │
┌───────────────────────────────┼─────────────┐
│  gup (wgpu 26)                │             │
│                       ┌───────┴──────────┐  │
│                       │ render_to_png()  │  │
│                       │ OffscreenTarget  │  │
│                       └──────────────────┘  │
└─────────────────────────────────────────────┘
```

Gup creates its own headless GPU device (wgpu 26) for off-screen rendering.
egui/eframe brings its own wgpu 27. The two wgpu versions coexist as separate
Cargo dependencies and do **not** share device/queue. Pixel data crosses the
boundary as plain `Vec<u8>`.

## Known Limitations

- **Two wgpu versions linked**: Because gup uses wgpu 26 and eframe 0.33 uses
  wgpu 27, the final binary links both. This increases compile time and binary
  size but is functionally correct. When gup upgrades to wgpu 27 device sharing
  will become possible.

- **Headless / software-renderer environments**: The chart rendering path
  requires a GPU device (or software Vulkan via lavapipe). If no GPU is
  available at all, `RenderContext::new()` will fail. In CI, use lavapipe or
  skip the chart construction.

- **PNG round-trip overhead**: The current pipeline encodes the chart to PNG on
  the GPU side, then decodes it on the egui side. This is the same approach used
  by `gup-bevy` and is fast enough for interactive use, but a future
  optimisation could transfer raw pixels directly.

- **Single chart per widget**: Each `GupWidget` wraps exactly one chart. For
  multi-chart layouts, create multiple `GupWidget` instances and place them in
  separate egui panels.
