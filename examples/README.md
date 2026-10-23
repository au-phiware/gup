# Gup Examples

Runnable examples for `gup`, the facade over `gup-core`. Run one with
`cargo run --example <name>`; set `GUP_SCREENSHOT_PATH=/tmp/out.png` to render a
single frame to a PNG instead of opening a window.

| Example                | Description                                     | Run Command                                |
| ---------------------- | ----------------------------------------------- | ------------------------------------------ |
| `hello_world`          | A sine-wave scatter with tooltips, in ten lines | `cargo run --example hello_world`          |
| `01_hello_chart`       | Minimal scatter chart                           | `cargo run --example 01_hello_chart`       |
| `03_line_chart`        | Line chart with two colour series               | `cargo run --example 03_line_chart`        |
| `04_bar_chart`         | Grouped (or stacked) bar chart                  | `cargo run --example 04_bar_chart`         |
| `composite_mixed_data` | One plot over two row types                     | `cargo run --example composite_mixed_data` |

The examples that used the pre-flip render core were deleted in GUP-488; git
history keeps them. More builders (area, boxplot, violin, density, heatmap,
choropleth) return with the T5 stories.

The `gup-tauri`, `android` and `ios_scatter` directories belong to parked
integrations (strategic review T7) and are not built.
