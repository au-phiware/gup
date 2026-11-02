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
| `scatter_bubbles`      | Bubble chart: size, colour and a log axis       | `cargo run --example scatter_bubbles`      |
| `line_monotone`        | Three monotone-curve lines with month labels    | `cargo run --example line_monotone`        |
| `bar_grouped_labels`   | Grouped bars with a value label on every bar    | `cargo run --example bar_grouped_labels`   |
| `barh_ranked`          | A ranking as horizontal bars with value labels  | `cargo run --example barh_ranked`          |
| `area_stacked`         | Stacked areas with an outline                   | `cargo run --example area_stacked`         |
| `histogram_stacked`    | A histogram stacked by mode, in bins of five    | `cargo run --example histogram_stacked`    |
| `heatmap_weekly`       | A weekday-by-hour heatmap through viridis       | `cargo run --example heatmap_weekly`       |
| `boxplot_latency`      | Box plots of latency with outliers              | `cargo run --example boxplot_latency`      |
| `annotated_prices`     | The Plot API: a step line with event labels     | `cargo run --example annotated_prices`     |
| `dark_multiline`       | Five lines in the dark theme                    | `cargo run --example dark_multiline`       |

The examples that used the pre-flip render core were deleted in GUP-488; the
history keeps them. The builders that exist today are scatter, line (with step
and smooth curves), bar and barh (grouped, stacked, with value labels), area
(single, overlapping and stacked), histogram, heatmap and boxplot, with text
labels (`Plot::text`), a subtitle and `Theme::dark()`; the
[tour of chart types](../docs/tutorials/07_chart_types.md) shows each with its
code and picture. Violin, density and choropleth charts are not built yet.
