# gup-egui

egui integration for the [Gup](https://github.com/au-phiware/gup)
GPU-accelerated data visualization library.

> **Parked (GUP-390).** This crate is excluded from the Gup workspace and CI
> until RFC-001 step S13 re-wires it onto `gup-core`. Build it with
> `CARGO_TARGET_DIR=target cargo check --manifest-path gup-egui/Cargo.toml` from
> the repository root. See the root README, "Parked integration crates".

## Features

- **`GupWidget`** — stateful egui widget that renders any Gup chart inside an
  egui panel.
- **Dirty tracking** — re-renders only when data or panel size has changed.
- **Interaction bridge** — translates egui pointer events (hover, click, drag,
  scroll) into Gup `InteractionEvent` types.
- **Coordinate mapping** — correctly accounts for panel offset and display scale
  factor.

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
cargo run --manifest-path gup-egui/Cargo.toml --example egui_chart
```

See [docs/EGUI_INTEGRATION.md](../docs/EGUI_INTEGRATION.md) for a comprehensive
integration guide.

## License

GPL-3.0-or-later
