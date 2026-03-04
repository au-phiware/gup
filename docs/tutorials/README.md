# Gup Tutorials

Welcome to the Gup tutorial series! These tutorials take you from your first
chart to advanced topics like custom GPU shaders and streaming data, one step at
a time.

## Prerequisites

Before you begin, make sure you have:

- **Rust** (stable toolchain, 1.85+) — install via [rustup](https://rustup.rs/)
- A **wgpu-compatible GPU** — most modern integrated and discrete GPUs work.
  Check the [wgpu wiki](https://github.com/gfx-rs/wgpu/wiki/Running-Locally)
  for details.
- **Cargo** — ships with Rust; used to build and run all examples.

## Tutorials

| # | Tutorial | Description |
|---|----------|-------------|
| 1 | [Getting Started](01_getting_started.md) | Create your first GPU-accelerated scatter chart with the chart builder API. |
| 2 | [Data Binding](02_data_binding.md) | Bind custom Rust structs to a `Selection<T, M>` with accessor functions. |
| 3 | [Custom Shader Functions](03_custom_shader_functions.md) | Write GPU shader transforms with `ComposableShaderFunction` and the `wgsl_function!` macro. |
| 4 | [Interactions](04_interactions.md) | Add hover, click, brush, and zoom/pan interactions to your charts. |
| 5 | [Streaming Data](05_streaming_data.md) | Connect live data sources using `StreamingDataSource` and `DataStream`. |
| 6 | [Custom Marks](06_custom_marks.md) | Implement a new mark type from scratch with the `Mark` trait and `#[derive(Mark)]`. |

## Learning Path

The tutorials are ordered by increasing complexity. If you are new to Gup, start
with **Tutorial 1** and work through them in order. Experienced Rust developers
may want to jump directly to the topic that interests them — each tutorial is
self-contained.

```text
Tutorial 1 ─── Getting Started
     │
Tutorial 2 ─── Data Binding (Selection<T, M>)
     │
     ├──── Tutorial 3 ─── Custom Shader Functions
     │
     ├──── Tutorial 4 ─── Interactions
     │
     ├──── Tutorial 5 ─── Streaming Data
     │
     └──── Tutorial 6 ─── Custom Marks
```

## Screenshot Sources

The screenshots in these tutorials are captured from the examples shipped with
Gup. To regenerate them, run the corresponding example and take a screenshot of
the window:

| Screenshot | Example command |
|-----------|----------------|
| `assets/tutorial01_scatter.png` | `cargo run --example 01_hello_chart` |
| `assets/tutorial04_interactions.png` | `cargo run --example interactive_circles` |
| `assets/tutorial05_streaming.png` | `cargo run --example streaming_live_chart` |
| `assets/tutorial06_custom_mark.png` | `cargo run --example custom_mark_demo` |

## Further Reading

- [API Reference](../TECHNICAL_APPROACH.md) — <!-- TODO(GUP-280): link to
  generated API docs --> deep dive into the unified shader function architecture
- [Custom Mark Guide](../CUSTOM_MARK_GUIDE.md) — architectural overview of the
  mark system
- [Grid System](../GRID_SYSTEM.md) — configuring grid lines and axes
- [Mark System](../mark-system/README.md) — mark architecture and built-in marks
- [Performance Guide](../PERFORMANCE_GUIDE.md) — profiling and optimisation tips
