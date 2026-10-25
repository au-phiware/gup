# Gup: GPU-Accelerated Data Visualization

> A few lines of Rust for a publication-quality chart, drawn on the GPU: in a
> window, a PNG, an SVG, egui, Bevy or the browser.

Gup (GPU Update Pattern) is a data visualization library for Rust. Data
encodings (scales, palettes, transfer functions) are shader functions that run
on the GPU, each with an exact CPU twin for axes, legends, picking and SVG, so
zooming and panning change uniforms rather than data.

## Why Gup

Figures are from [`PERF_BUDGETS.md`](./crates/gup-core/PERF_BUDGETS.md), release
build, Intel HD Graphics 630 (an integrated GPU), Mesa 26.0.0, Vulkan:

- Zooming writes no column bytes: `zoom.columns.bytes` is 0 over 600 frames.
- Culling by chunk: 4M sorted rows draw in about 10 ms of GPU time with 14 draws
  (`zoom_culled.gpu_pass.median_ms`, `zoom_culled.draws.median`).
- The reference scatter's WASM build is about 340 KB gzipped, under a 400 KB cap
  for gup-core over wgpu (`wasm.scatter.gz_bytes`, `wasm.over_wgpu.gz_bytes`).

The timings apply to that machine; measure on yours before comparing.

## Key Features

- **Encodings are GPU functions.** Scales and palettes are WGSL with f64 CPU
  mirrors; write your own with `#[wgsl_function]`.
- **A chunked GPU column store.** Accessors run once; zooming writes no column
  bytes.
- **One chart, every target.** A window, a PNG, an SVG, a texture for egui or
  Bevy, and a WebGPU canvas.
- **Typed channels.** A wrong value type, an unknown channel or the wrong mark
  does not compile.
- **Builders that do not trap you.** Start from `gup::scatter`, then drop to the
  typed `Selection` API.

## Not yet

Each item names its track in
[`IMPLEMENTATION_STRATEGY.md`](./docs/IMPLEMENTATION_STRATEGY.md):

- No CSV, serde or Arrow input: rows enter through closures (N5).
- Nothing above 10M rows is measured, and there is no level of detail (N5).
- No PDF export (N6).
- No accessibility support yet: a minimum comes in N3, keyboard navigation in
  N4.
- CI renders on Linux only; Windows comes in N3.
- Chart types are those the builders list today; more come in N2.

## Quick Start

Gup is not on crates.io yet; release 0.1 will be. Add it from git to
`Cargo.toml`:

```toml
[dependencies]
gup = { git = "https://github.com/au-phiware/gup" }
```

Then draw a scatter chart and save it as a PNG:

```rust,no_run
use gup::prelude::*;

struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }

fn main() -> gup::Result<()> {
    let data = vec![
        Row { gdp: 1e3, life_exp: 55.0, continent: "Africa".into(), population: 3e7 },
        Row { gdp: 2e4, life_exp: 74.0, continent: "Europe".into(), population: 6e7 },
        Row { gdp: 6e4, life_exp: 81.0, continent: "Asia".into(), population: 1e9 },
    ];

    let mut chart = gup::scatter(data)
        .x(|d| d.gdp)
        .y(|d| d.life_exp)
        .color(|d| d.continent.clone())
        .size(|d| d.population);
    let cx = Context::shared()?;
    chart.save_png(&cx, "chart.png", 800, 500)?;
    Ok(())
}
```

To keep the image in memory instead, for a test say:

```rust
use gup::prelude::*;

struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }

fn main() -> gup::Result<()> {
    let data = vec![
        Row { gdp: 1e3, life_exp: 55.0, continent: "Africa".into(), population: 3e7 },
        Row { gdp: 2e4, life_exp: 74.0, continent: "Europe".into(), population: 6e7 },
        Row { gdp: 6e4, life_exp: 81.0, continent: "Asia".into(), population: 1e9 },
    ];

    let mut chart = gup::scatter(data)
        .x(|d| d.gdp)
        .y(|d| d.life_exp)
        .color(|d| d.continent.clone())
        .size(|d| d.population);
    let image = chart.render(&Context::shared()?, 800, 500)?;
    assert_eq!(image.dimensions(), (800, 500));
    Ok(())
}
```

To open a window instead, pass the chart to `gup::show(chart)`. Continue with
[Tutorial 1, Getting Started](./docs/tutorials/01_getting_started.md), or see
all [tutorials](./docs/tutorials/README.md).

## Architecture Highlights

See [`TECHNICAL_APPROACH.md`](./docs/TECHNICAL_APPROACH.md).

- **Shader functions with CPU mirrors**: `ShaderFn`, `then` and
  `#[wgsl_function]`, composed at build time.
- **GPU column store**: chunked, relative origins, validity bits, dictionaries,
  `Retain`, `append` and `Window`.
- **Typed channels and marks**: `#[derive(Mark)]`, compile-time errors.
- **One scale family**: linear, pow, log, symlog, time, band, point and colour
  scales, with ticks and legends from the mirrors.
- **`Plot` and `Scene`**: layout, guides and legends resolve to a plain scene.
- **Render targets**: image, window, texture, canvas and SVG, one render pass.
- **Interaction**: CPU and GPU picking, tooltips, zoom, pan, brush and culling.
- **Builders**: `scatter`, `line` and `bar`, over the same `Plot`.
- **Hosts**: `show`, a WebGPU canvas, `gup-egui` and `gup-bevy`.
- **Gates**: performance and WASM budgets, visual regression, a browser smoke
  test and a dogfood suite.

## Development Status

RFC-001 (the core architecture) is implemented: `gup` is a facade over
`gup-core`. Next, from the
[strategic review](./docs/planning/STRATEGIC_REVIEW_2026-11.md): N0, hygiene;
N1, tested tutorials; N2, more chart types; then N3, release 0.1. See
[`IMPLEMENTATION_STRATEGY.md`](./docs/IMPLEMENTATION_STRATEGY.md).

## Documentation

- [`docs/tutorials/README.md`](./docs/tutorials/README.md) - Six tested
  tutorials, from a first chart to custom marks
- [`MISSION_AND_GOALS.md`](./docs/MISSION_AND_GOALS.md) - Project vision and
  objectives
- [`TECHNICAL_APPROACH.md`](./docs/TECHNICAL_APPROACH.md) - How Gup works today
- [`IMPLEMENTATION_STRATEGY.md`](./docs/IMPLEMENTATION_STRATEGY.md) - How it is
  built, and what is next
- [`MARKET_ANALYSIS.md`](./docs/MARKET_ANALYSIS.md) - Competitive positioning
- [docs/README.md](docs/README.md) and [docs/INDEX.md](docs/INDEX.md) - Reading
  guide and full listing
- [docs/planning/stories/INDEX.md](docs/planning/stories/INDEX.md) - All stories
  and their status

## Contributing

Gup is developed in the open with community input. See the documentation above
for technical details and architectural decisions.

## Examples

```bash
cargo run --example hello_world       # a sine wave with tooltips
cargo run --example 04_bar_chart      # grouped bars (--stacked to stack)
cargo run -p gup-core --example live_append   # rows appended from a thread
```

See [examples/README.md](examples/README.md) for the full list. Set
`GUP_SCREENSHOT_PATH=out.png` to render one frame headless. For a browser build,
see `mask wasm-browser`.

## Workspace Crates

A plain `cargo build` or `cargo test` at the repository root builds only the
`gup` crate (`default-members = ["."]`), a facade over `gup-core`. The other
workspace members are built with `-p`:

| Crate                          | Status                                                                |
| ------------------------------ | --------------------------------------------------------------------- |
| `crates/gup-core`              | The core: context, column store, scales, marks, plot, scene, targets. |
| `crates/gup-core-macros`       | `#[wgsl_function]` and `#[derive(Mark)]`.                             |
| `crates/gup-wgsl`              | WGSL library, build-time composition and the run-time linker.         |
| `crates/gup-text`              | Font loading, shaping and the glyph atlas.                            |
| `crates/gup-visual-regression` | Golden-image and structural check harness (`mask visual-regression`). |
| `gup-egui`                     | egui integration (`GupWidget`).                                       |
| `gup-bevy`                     | Bevy integration (`GupPlugin`), built by the `integrations` CI job.   |

`crates/gup-culling-lod` holds GPU culling and LOD code from before the core was
rewritten. It is parked: out of the workspace and CI, until the LOD work in
strategic review T7 ports what it needs. See its
[README](crates/gup-culling-lod/README.md).

`cargo test -p gup-egui` tests the egui integration. `gup-bevy` is built by the
`integrations` CI job, not by `mask all-check`:
`cargo test -p gup-bevy -- --test-threads=1`.

## License

This project is licensed under the GNU General Public License v3.0 or later
(GPL-3.0-or-later). See [COPYING](COPYING) for details.
