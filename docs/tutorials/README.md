# Gup Tutorials

> **Out of date.** This tutorial was written for the API before the RFC-001 flip
> and does not compile against `gup` today. It is being rewritten as tested
> Markdown (strategic review 2026-11, track N1). Until then, start from the
> [README](../../README.md) Quick Start and
> [Technical Approach](../TECHNICAL_APPROACH.md).

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
| 4   | [Interactions](04_interactions.md)                       | Add hover, click, brush, and zoom/pan interactions to your charts.                                           |
| 5   | [Streaming Data](05_streaming_data.md)                   | Connect live data sources using `StreamingDataSource` and `DataStream`.                                      |
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

The images in tutorials 1 and 2 are golden images, not screenshots:
`tests/tutorials.rs` repeats each chart's calls from the tutorial text, renders
it at 640 by 400 and compares it with `tests/golden/tutorials/<name>.png`, so a
chart that changes fails CI instead of leaving a stale picture. After an
intentional change, run
`GUP_BLESS=1 cargo test -p gup --test tutorials -- --test-threads=1`, look at
every image that changed, and commit them. The other tutorials still use
screenshots in `assets/` until they are rewritten.

## Further Reading

- [API Reference](../TECHNICAL_APPROACH.md) — <!-- TODO(GUP-280): link to
  generated API docs --> deep dive into the unified shader function architecture
- [Performance Guide](../PERFORMANCE_GUIDE.md) — profiling and optimisation tips
