# egui Integration Guide

`gup-egui` puts any [`gup_core::Chart`] in an egui panel. It renders on egui's
own GPU device into a texture that egui samples directly (no CPU readback) and
forwards pointer input to the chart.

## Adding the dependency

```toml
[dependencies]
gup-egui = { path = "gup-egui" }
gup-core = { path = "crates/gup-core" }
eframe = { version = "0.33", features = ["wgpu"] }
```

The app must use eframe's wgpu backend
(`NativeOptions { renderer: eframe::Renderer::Wgpu, .. }`).

## Minimal code

```rust,ignore
use gup_core::prelude::*;
use gup_egui::GupWidget;

struct App {
    widget: GupWidget,
    chart: gup_core::ScatterChart<(f64, f64)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc.wgpu_render_state.as_ref().expect("the wgpu backend");
        Self {
            widget: GupWidget::new(render_state),
            chart: gup_core::scatter(vec![(1.0, 2.0), (2.0, 3.0)]).x(|p| p.0).y(|p| p.1),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.widget.show(ui, egui::vec2(480.0, 320.0), &mut self.chart);
        });
    }
}
```

Keep one `GupWidget` per chart. The chart stays yours (`&mut dyn Chart`), so
mutate it, rebuild it or swap it between frames freely.

## Live data

Take an `Appender` from the chart (`chart.appender()`) and push from any thread.
`show` hands the chart a waker that calls `egui::Context::request_repaint`, so
egui repaints when rows arrive. `examples/egui_chart.rs` feeds a line from a
thread this way.

## Interaction

`show` maps egui's pointer events to `gup_core::Input` (`gup_egui::to_input`)
and calls `Chart::handle`: moves, presses inside the widget, releases (also
outside, after a press), the mouse wheel while hovered, and the pointer leaving.
Tooltips, brushing and zoom are the chart's own. `show` returns the
`egui::Response` of the allocated rect.

## Architecture

- `gup_egui::context(render_state)` makes one `gup_core::Context` over egui's
  device (`Context::from_wgpu`) and keeps it in the renderer's
  `callback_resources`, so widgets share pipelines and caches.
- Each widget owns an `Rgba8Unorm` texture of `size × pixels_per_point` and
  draws the chart's scene into it with Gup's own pass (`TextureTarget` and
  `Renderer::render`). egui samples the texture raw: Gup writes sRGB bytes and
  egui's shader works on gamma bytes, so an sRGB view would decode them twice.
- `show` renders on every call; a chart that has not changed uploads nothing.

## Known limitations

- Native only; egui on the web is out of scope (`gup_core::web` is the browser
  host).
- The chart is drawn in its own pass, not inside egui's, so it cannot be clipped
  by egui paint callbacks or layered between egui shapes in one pass.
