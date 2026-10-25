# Gup Tutorials

Welcome to the Gup tutorial series! These tutorials take you from your first
chart to advanced topics like custom GPU shaders and streaming data, one step at
a time.

## Prerequisites

Before you begin, make sure you have:

- **Rust 1.85+ (edition 2024)** — install via [rustup](https://rustup.rs/)
- A **wgpu-compatible GPU** — most modern integrated and discrete GPUs work.
  Check the [wgpu wiki](https://github.com/gfx-rs/wgpu/wiki/Running-Locally) for
  details.
- **Cargo** — ships with Rust; used to build and run all examples.

## Tutorials

| #   | Tutorial                                                 | Description                                                                                                  |
| --- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 1   | [Getting Started](01_getting_started.md)                 | Draw scatter, line and bar charts from your own structs, save them as PNGs, colour by key and size by value. |
| 2   | [Plots and Selections](02_data_binding.md)               | What a builder is made of: layers of marks on shared scales, colour scales with legends, a second y axis.    |
| 3   | [Custom Shader Functions](03_custom_shader_functions.md) | Chain scales with `.then(..)`, write your own in WGSL with `#[wgsl_function]` and give it a CPU mirror.      |
| 4   | [Interactions](04_interactions.md)                       | Tooltips, hover and click on rows, wheel zoom, drag pan, brushing, and testing it all by sending `Input`.    |
| 5   | [Streaming Data](05_streaming_data.md)                   | Push rows from another thread with an `Appender`, bound memory with `Window::LastRows`, grow keyed lines.    |
| 6   | [Custom Marks](06_custom_marks.md)                       | Write a new mark with `#[derive(Mark)]` and WGSL: channel roles, setters, CPU and GPU picking.               |

## Learning Path

The tutorials are ordered by increasing complexity. If you are new to Gup, start
with **Tutorial 1** and work through them in order. Experienced Rust developers
may want to jump directly to the topic that interests them — each tutorial is
self-contained.

```text
Tutorial 1 ─── Getting Started
     │
Tutorial 2 ─── Plots and Selections
     │
     ├──── Tutorial 3 ─── Custom Shader Functions
     │
     ├──── Tutorial 4 ─── Interactions
     │
     ├──── Tutorial 5 ─── Streaming Data
     │
     └──── Tutorial 6 ─── Custom Marks
```

## Images

The images in the tutorials are golden images, not screenshots:
`tests/tutorials.rs` repeats each chart's calls from the tutorial text, renders
it at 640 by 400 and compares it with `tests/golden/tutorials/<name>.png`, so a
chart that changes fails CI instead of leaving a stale picture. After an
intentional change, run
`GUP_BLESS=1 cargo test -p gup --test tutorials -- --test-threads=1`, look at
every image that changed, and commit them. Every code block is a doctest:
`cargo test --doc -p gup -- tutorials`.

## Further Reading

- [Technical Approach](../TECHNICAL_APPROACH.md) — a deep dive into the unified
  shader function architecture
- The API reference: run `cargo doc --open -p gup` (the `gup` crate re-exports
  `gup-core`)
- [Performance Guide](../PERFORMANCE_GUIDE.md) — profiling and optimisation tips
