# Gup: GPU-Accelerated Data Visualization

> **Observable Plot's simplicity meets GPU performance for billion-point
> datasets**

Gup (GPU Update Pattern) is a high-performance data visualization library for
Rust that combines the declarative elegance of D3.js with the raw power of GPU
computing. Designed for real-time visualization of massive datasets, Gup enables
smooth 60+ FPS interactions with millions to billions of data points.

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

### 🚀 Unprecedented Performance

- **1 billion points** at 30+ FPS
- **Real-time streaming** data updates
- **GPU-parallel** data transformations
- **Cross-platform** identical performance

### 🎨 Dual API Design

```rust
// Observable Plot simplicity
gup::plot()
    .data(sales_data)
    .scatter(x("revenue"), y("profit"))
    .color("region")
    .render()?;

// D3.js-style control
chart.select_all::<Circle>()
    .data(sales_data)
    .attr("position", position_transform)
    .attr("color", color_scale)
    .on("click", |event, datum| { /* ... */ });
```

### 🔧 Unified Shader Functions

All data transformations are composable WGSL functions running on GPU:

```rust
chart.select_all::<Circle>()
    .attr("position",
        geographic_projection
            .compose(screen_transform)
    )
    .attr("color",
        temperature_scale
            .compose(color_interpolation)
    );
```

### ♿ Accessibility First

- **Screen reader support** with semantic data descriptions
- **Keyboard navigation** for all interactions
- **High contrast** and color-blind friendly modes
- **WCAG 2.1 AA** compliance from day one

## Quick Start

Add to `Cargo.toml`:

```toml
[dependencies]
gup = "0.1"
```

Create your first visualization:

```rust
use gup::prelude::*;

// Load your data
let data = load_csv("sales_data.csv")?;

// Create scatter plot
gup::plot()
    .data(data)
    .scatter(x("revenue"), y("profit"))
    .color("region")
    .size("market_cap")
    .render()?;
```

### Type Construction with Macros

Gup provides ergonomic macros for creating GPU-compatible vectors and matrices:

```rust
use gup::*;

// Create vectors
let position = vec3![1.0, 2.0, 3.0];
let color = vec4![1.0, 0.5, 0.0, 1.0];

// Create matrices
let transform = mat4![
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
];
```

These macros ensure proper GPU memory alignment and provide zero-cost
abstractions. See the
[Type Construction Guide](./docs/TYPE_CONSTRUCTION_GUIDE.md) for complete
documentation.

## Architecture Highlights

### GPU-First Design

- **Vertex buffers** store data directly on GPU
- **Compute shaders** handle statistical analysis
- **Parallel processing** for all transformations
- **Streaming updates** without CPU bottlenecks

### Universal Composability

Following D3.js's philosophy, everything in Gup composes naturally:

- **Shader functions** chain like D3 methods
- **Scales** mix with **selections**
- **Interactions** compose with **animations**
- **Custom marks** integrate seamlessly

### Type Safety

Rust's type system ensures correctness:

- **Compile-time validation** of shader function composition
- **Automatic WGSL generation** from Rust types
- **IDE support** with full IntelliSense
- **Migration assistance** from existing libraries

## Market Position

> **"Observable Plot for the GPU Era"**

Gup uniquely combines:

- **Observable Plot's ease**: One-line chart creation
- **D3.js's power**: Deep customization and control
- **GPU performance**: Handle datasets 1000x larger
- **Rust safety**: Eliminate entire classes of runtime errors

## Development Status

**Phase 1: Low-Level Foundation** (In Progress)

- Core GPU primitives and Selection API
- Unified shader function system
- Basic mark types and scales
- Cross-platform support

See [`IMPLEMENTATION_STRATEGY.md`](./docs/IMPLEMENTATION_STRATEGY.md) for
detailed roadmap.

## Documentation

### Migration Guides

- [**Migration from Observable Plot**](./docs/MIGRATION_FROM_OBSERVABLE_PLOT.md) -
  Comprehensive guide for Observable Plot users transitioning to Gup

### Technical Documentation

- [`MISSION_AND_GOALS.md`](./docs/MISSION_AND_GOALS.md) - Project vision and
  objectives
- [`TECHNICAL_APPROACH.md`](./docs/TECHNICAL_APPROACH.md) - Architecture deep
  dive
- [`MARKET_ANALYSIS.md`](./docs/MARKET_ANALYSIS.md) - Competitive positioning
- [`TYPE_CONSTRUCTION_GUIDE.md`](./docs/TYPE_CONSTRUCTION_GUIDE.md) - Guide to
  GPU-compatible type construction

## Contributing

Gup is developed in the open with community input. See the documentation above
for technical details and architectural decisions.

## Examples

Check out the `examples/` directory for complete working examples:

```bash
# Run an interactive scatter plot demo with window
cargo run --example scatter_plot_demo

# Interactive visual blend mode demonstration
cargo run --example visual_blend_demo

# GPU buffer management demonstration
cargo run --example buffer_demo
```

See [examples/INDEX.md](examples/INDEX.md) for a full listing, or
[examples/README.md](examples/README.md) for a guided learning path.

For WebAssembly builds:

```bash
# Build and serve examples in browser
mask start
```

## Documentation

Full documentation is in the [`docs/`](docs/) directory. See
[docs/INDEX.md](docs/INDEX.md) for a complete listing, or start with:

- [docs/README.md](docs/README.md) — Architecture overview and reading guide
- [docs/IMPLEMENTATION_STRATEGY.md](docs/IMPLEMENTATION_STRATEGY.md) —
  Development strategy
- [docs/planning/stories/INDEX.md](docs/planning/stories/INDEX.md) — All stories
  and their status

## Project Structure

```text
gup/
├── src/
│   ├── core/       # Core data structures and Selection API
│   ├── gpu/        # GPU abstraction and WebGPU integration
│   ├── marks/      # Visualization mark implementations
│   └── utils/      # Utility functions and helpers
├── examples/       # Example applications and tutorials
├── benches/        # Performance benchmarks
└── docs/          # Comprehensive documentation and guides
```

## License

This project is licensed under the GNU General Public License v3.0 or later
(GPL-3.0-or-later). See [COPYING](COPYING) for details.

---

**Gup is transforming data visualization by bringing GPU performance to
declarative, composable APIs. Join us in building the future of high-performance
data visualization.**
