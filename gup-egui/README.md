# gup-egui

egui integration for the [Gup](https://github.com/au-phiware/gup)
GPU-accelerated data visualization library, built on `gup-core`.

```rust,ignore
let widget = gup_egui::GupWidget::new(render_state); // once, per chart
widget.show(ui, egui::vec2(480.0, 320.0), &mut chart); // every frame
```

See [the integration guide](../docs/EGUI_INTEGRATION.md) and
`cargo run -p gup-egui --example egui_chart`.
