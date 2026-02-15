# Gup Examples

This directory contains examples demonstrating how to use the Gup
GPU-accelerated data visualization library. The examples are organized by
difficulty level and topic.

## Learning Path

Start here if you're new to Gup! Follow this sequence for the best learning
experience:

### 1. Basic Examples (`basic/`)

These examples teach fundamental concepts step-by-step:

| Example             | Description                         | Run Command                             |
| ------------------- | ----------------------------------- | --------------------------------------- |
| `01_hello_chart`    | Minimal scatter plot (console only) | `cargo run --example 01_hello_chart`    |
| `02_scatter_window` | Visual scatter plot in a window     | `cargo run --example 02_scatter_window` |
| `03_line_chart`     | Line chart API demonstration        | `cargo run --example 03_line_chart`     |
| `04_bar_chart`      | Bar chart API demonstration         | `cargo run --example 04_bar_chart`      |

**Recommended Order:**

1. Start with `01_hello_chart` to understand the core concepts
2. Move to `02_scatter_window` to see GPU rendering in action
3. Explore `03_line_chart` and `04_bar_chart` to learn different chart types

### 2. Intermediate Examples

Once you're comfortable with the basics:

| Example             | Description                               |
| ------------------- | ----------------------------------------- |
| `simple_window`     | Basic window with color cycling           |
| `scatter_plot_demo` | Advanced scatter plot with custom shaders |
| `windowed_demo`     | Multi-feature window demo                 |

### 3. Advanced Examples

For deep dives into specific features:

| Example                               | Description                     |
| ------------------------------------- | ------------------------------- |
| `observable_plot_showcase`            | Full Observable Plot-style API  |
| `observable_plot_visual_showcase`     | Visual Observable Plot examples |
| `label_formatting_demo`               | Advanced label formatting       |
| `axis_showcase`                       | Axis configuration options      |
| `grid_visual_demo`                    | Grid styling and customization  |
| `blend_modes_showcase`                | GPU blend mode demonstrations   |
| `composition_error_recovery_showcase` | Error handling patterns         |

### 4. Technical Deep Dives

For understanding the library internals:

| Example                | Description                |
| ---------------------- | -------------------------- |
| `text_rendering_demo`  | MSDF text rendering system |
| `shader_pipeline_demo` | Custom shader integration  |
| `gpu_debug_demo`       | GPU debugging tools        |
| `buffer_demo`          | GPU buffer management      |
| `async_streaming_demo` | Async data streaming       |

## Quick Start

Run any example with:

```bash
cargo run --example <example_name>
```

For example:

```bash
# Start with the simplest example
cargo run --example 01_hello_chart

# Or jump to a visual demo
cargo run --example 02_scatter_window
```

## Key Concepts

### Data Structure

All Gup charts start with your data:

```rust
#[derive(Debug, Clone)]
struct MyData {
    x: f32,
    y: f32,
    category: String,
}
```

### Accessor Functions

Accessors tell Gup how to extract values from your data:

```rust
let x_accessor = AccessorFunction::new(|d: &MyData| AccessorValue::Float(d.x));
let y_accessor = AccessorFunction::new(|d: &MyData| AccessorValue::Float(d.y));
```

### Chart Builders

Observable Plot-style fluent API:

```rust
let chart = scatter()
    .x(x_accessor)
    .y(y_accessor)
    .title("My Chart")
    .point_size(10.0)
    .fill_color([0.2, 0.6, 0.9, 1.0]);

let selection = chart.build_with_data(data, context)?;
```

### GPU Context

Initialize the GPU connection:

```rust
let context = Arc::new(RenderContext::new().await?);
```

## Chart Types

| Builder     | Mark Type    | Status                           |
| ----------- | ------------ | -------------------------------- |
| `scatter()` | Circles      | ✅ Full support                  |
| `line()`    | Lines        | 🚧 API ready, visual in progress |
| `bar()`     | Rectangles   | 🚧 API ready, visual in progress |
| `area()`    | Filled areas | 🚧 API ready, visual in progress |
| `heatmap()` | Color grids  | 🚧 API ready, visual in progress |

## Window Controls

Most windowed examples use these controls:

- **ESC** or **Q**: Quit the application
- **Space**: Toggle features (example-specific)
- **Arrow keys**: Navigate (example-specific)

## Troubleshooting

### GPU Not Found

If you see GPU initialization errors:

1. Ensure you have a WebGPU-compatible GPU
2. Update your graphics drivers
3. On Linux, ensure Vulkan is installed

### Window Not Appearing

On some systems, windows may take a moment to appear. Wait a few seconds after
starting the example.

### Performance Issues

- Use `cargo run --release --example <name>` for better performance
- Reduce data size if working with large datasets

## Contributing Examples

When adding new examples:

1. Place basic examples in `examples/basic/`
2. Add `[[example]]` entry to `Cargo.toml`
3. Include comprehensive comments
4. Add tests at the bottom of the file
5. Update this README with your example
