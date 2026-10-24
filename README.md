# Gup: GPU-Accelerated Data Visualization

> **Observable Plot's simplicity meets GPU performance for billion-point
> datasets**

Gup (GPU Update Pattern) is a data visualization library for Rust. Data
encodings (scales, palettes, transfer functions) are shader functions that run
on the GPU, each with an exact CPU twin for axes, legends, picking and SVG, so
zooming and panning change uniforms rather than data.

## Motivation

### The Performance Crisis

Current visualization libraries hit fundamental walls:

- **D3.js**: Limited to ~1,000 points at 60 FPS
- **Observable Plot**: Same CPU limitations despite simpler API
- **Plotters**: Designed for static charts, crashes with real-time updates
- **Three.js**: General 3D graphics, not optimized for data visualization
  patterns

### The GPU Opportunity

Modern GPUs can process millions of data points in parallel, but existing
libraries don't leverage this power. Gup is built GPU-first from the ground up.

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

## Quick Start

Add to `Cargo.toml`:

```toml
[dependencies]
gup = "0.1"
```

Create your first visualization (this is the doctest in `src/lib.rs`):

```rust
use gup::prelude::*;

struct Row { gdp: f64, life_exp: f64, continent: String, population: f64 }
let data = vec![
    Row { gdp: 1e3, life_exp: 55.0, continent: "Africa".into(), population: 3e7 },
    Row { gdp: 2e4, life_exp: 74.0, continent: "Europe".into(), population: 6e7 },
    Row { gdp: 6e4, life_exp: 81.0, continent: "Asia".into(), population: 1e9 },
];

let mut chart = gup::scatter(data)
    .x(|d| d.gdp)
    .y(|d| d.life_exp)
    .color(|d| d.continent.as_str())
    .size(|d| d.population);
let image = chart.render(&Context::shared()?, 800, 500)?;
assert_eq!(image.dimensions(), (800, 500));
# Ok::<(), gup::Error>(())
```

To open a window instead, pass the chart to `gup::show(chart)`.

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

RFC-001 (the core architecture) is implemented: steps S0 to S14, with `gup` a
facade over `gup-core`. Next, from the
[strategic review](./docs/planning/STRATEGIC_REVIEW_2026-10.md): T5, more
builders on `gup`; T6, a flagship gallery and docs-as-tests; T7, the crate
split, level of detail. See
[`IMPLEMENTATION_STRATEGY.md`](./docs/IMPLEMENTATION_STRATEGY.md).

## Documentation

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

---

**Gup is transforming data visualization by bringing GPU performance to
declarative, composable APIs. Join us in building the future of high-performance
data visualization.**
