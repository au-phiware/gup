# gup-egui

egui integration for the [Gup](https://github.com/au-phiware/gup)
GPU-accelerated data visualization library.

## Features

- **`GupWidget`** — stateful egui widget that renders any Gup chart inside
  an egui panel.
- **Dirty tracking** — re-renders only when data or panel size has changed.
- **Interaction bridge** — translates egui pointer events (hover, click,
  drag, scroll) into Gup `InteractionEvent` types.
- **Coordinate mapping** — correctly accounts for panel offset and display
  scale factor.

## Quick Start

```rust
use gup_egui::GupWidget;

// 1. Build a chart with the Gup chart-builder API.
let chart = scatter().x(x_acc).y(y_acc).build_with_data(data, ctx)?;

// 2. Wrap it in a GupWidget.
let mut widget = GupWidget::new(chart);

// 3. Display in any egui panel.
ui.add(&mut widget);

// 4. When data changes:
widget.mark_dirty();
```

## Example

```bash
cargo run -p gup-egui --example egui_chart
```

See [docs/EGUI_INTEGRATION.md](../docs/EGUI_INTEGRATION.md) for a
comprehensive integration guide.

## License

GPL-3.0-or-later
