# Grid System Documentation

The Gup Grid System provides GPU-accelerated grid line rendering for
professional data visualizations. Grid lines enhance chart readability by
providing visual reference points aligned with axis tick marks.

## Table of Contents

1. [Quick Start Guide](#quick-start-guide)
2. [Core Concepts](#core-concepts)
3. [API Reference](#api-reference)
4. [Configuration Guide](#configuration-guide)
5. [Examples and Tutorials](#examples-and-tutorials)
6. [Performance Guide](#performance-guide)
7. [Troubleshooting](#troubleshooting)
8. [Advanced Topics](#advanced-topics)

---

## Quick Start Guide

### Enabling Grid Lines

The simplest way to add grid lines is through the chart builder API:

```rust
use gup::chart_builder::ScatterPlotBuilder;

let chart = ScatterPlotBuilder::new()
    .data(data)
    .grid()  // Enable grid with professional defaults
    .build()?;
```

One method call gives you subtle, professional grid lines that enhance
readability without competing with the data.

### Using Theme Presets

Choose from six built-in themes designed for common use cases:

```rust
// Light backgrounds (default web/document style)
let chart = ScatterPlotBuilder::new().data(data).light_grid().build()?;

// Dark backgrounds (dashboards, dark mode)
let chart = ScatterPlotBuilder::new().data(data).dark_grid().build()?;

// Scientific publications (includes minor grids)
let chart = ScatterPlotBuilder::new().data(data).scientific_grid().build()?;

// Business dashboards (horizontal-only, clean look)
let chart = ScatterPlotBuilder::new().data(data).business_grid().build()?;

// Minimal design (extremely subtle)
let chart = ScatterPlotBuilder::new().data(data).minimal_grid().build()?;

// Accessibility-focused (high contrast, major + minor grids)
let chart = ScatterPlotBuilder::new().data(data).high_contrast_grid().build()?;
```

### Quick Styling

Adjust grid appearance with convenience methods:

```rust
let chart = ScatterPlotBuilder::new()
    .data(data)
    .grid()
    .grid_color("#cccccc")    // Hex color string
    .grid_opacity(0.5)        // Transparency (0.0–1.0)
    .grid_width(1.0)          // Line thickness in pixels
    .build()?;
```

### Directional Grids

Show grid lines in one direction only:

```rust
// Horizontal grid lines only (common for bar charts)
let chart = ScatterPlotBuilder::new()
    .data(data)
    .horizontal_grid()
    .build()?;

// Vertical grid lines only
let chart = ScatterPlotBuilder::new()
    .data(data)
    .vertical_grid()
    .build()?;
```

---

## Core Concepts

### Architecture Overview

The Grid System is built on three layers:

```
┌──────────────────────────────────────────────────┐
│  Chart Builder API (GridCapableBuilder trait)     │  ← User-facing
├──────────────────────────────────────────────────┤
│  GridSystem / AxisGridCoordinator                │  ← Coordination
├──────────────────────────────────────────────────┤
│  GridRenderer (GPU line generation + caching)    │  ← GPU rendering
└──────────────────────────────────────────────────┘
```

1. **Chart Builder API**: The `GridCapableBuilder` trait adds grid methods to
   all chart builders (scatter, line, box plot, bar, area, heatmap). This is the
   primary API for most users.

2. **GridSystem**: Manages configuration and delegates to the renderer. Use
   directly when building custom chart types.

3. **GridRenderer**: Generates `LineAttributes` geometry from tick positions and
   configuration, with fingerprint-based caching to avoid per-frame
   regeneration.

### Grid Line Types

The system supports two types of grid lines:

- **Major grid lines**: Aligned with axis tick marks. Enabled by default.
  Typically thicker and more visible.
- **Minor grid lines**: Subdivisions between major ticks. Disabled by default.
  Thinner and more subtle, useful for scientific precision.

### Axis Alignment

Grid lines are automatically aligned with axis tick positions through the
`AxisGridCoordinator`. When you enable grids via the chart builder, horizontal
grid lines match the Y-axis ticks and vertical grid lines match the X-axis
ticks. This alignment is maintained even when tick positions change due to data
updates or zoom/pan interactions.

### Rendering Order (Z-Ordering)

Grid lines render **behind** the data and axes to avoid obscuring important
information:

```
Back ──────────────────────────── Front
 Grid Lines → Axes → Data Points → Labels
```

### Geometry Caching

The `GridRenderer` uses a fingerprint-based cache. It hashes tick positions,
chart bounds, and configuration into a 64-bit fingerprint. If the fingerprint
matches the previous frame, geometry regeneration is skipped entirely, yielding
zero-cost grid rendering on static views.

---

## API Reference

### Color

Color representation for grid styling.

```rust
pub struct Color {
    pub r: f32,  // Red component (0.0–1.0)
    pub g: f32,  // Green component (0.0–1.0)
    pub b: f32,  // Blue component (0.0–1.0)
    pub a: f32,  // Alpha component (0.0–1.0)
}
```

#### Construction

```rust
// Direct constructor
let color = Color::new(0.8, 0.8, 0.8, 0.7);

// From hex string
let color = Color::from_hex("#cccccc").unwrap();
let color = Color::from_hex("#ccc").unwrap();  // Shorthand

// From tuples
let color: Color = (0.8, 0.8, 0.8).into();        // RGB (alpha = 1.0)
let color: Color = (0.8, 0.8, 0.8, 0.7).into();   // RGBA

// From array
let color: Color = [0.8, 0.8, 0.8, 0.7].into();

// From &str (hex)
let color: Color = "#cccccc".into();
```

#### Preset Colors

| Constant                    | Value                     | Use Case          |
| --------------------------- | ------------------------- | ----------------- |
| `Color::LIGHT_GRID`         | `(0.9, 0.9, 0.9, 0.7)`    | Light backgrounds |
| `Color::DARK_GRID`          | `(0.3, 0.3, 0.3, 0.8)`    | Dark backgrounds  |
| `Color::SUBTLE_GRID`        | `(0.95, 0.95, 0.95, 0.5)` | Minimal styling   |
| `Color::HIGH_CONTRAST_GRID` | `(0.0, 0.0, 0.0, 0.8)`    | Accessibility     |

#### Conversion

```rust
// To RGBA array
let rgba: [f32; 4] = color.to_rgba();
let rgba: [f32; 4] = color.into();
```

---

### ChartBounds

Defines the coordinate space for the chart rendering area.

```rust
pub struct ChartBounds {
    pub left: f32,    // Left edge X coordinate
    pub right: f32,   // Right edge X coordinate
    pub top: f32,     // Top edge Y coordinate
    pub bottom: f32,  // Bottom edge Y coordinate
}
```

#### Methods

| Method     | Signature                                  | Description             |
| ---------- | ------------------------------------------ | ----------------------- |
| `new`      | `fn new(left, right, top, bottom) -> Self` | Create bounds           |
| `width`    | `fn width(&self) -> f32`                   | Width (`right - left`)  |
| `height`   | `fn height(&self) -> f32`                  | Height (`bottom - top`) |
| `center`   | `fn center(&self) -> Vec2`                 | Center point            |
| `contains` | `fn contains(&self, point: Vec2) -> bool`  | Point-in-bounds test    |

#### Example

```rust
use gup::grid::ChartBounds;

let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
assert_eq!(bounds.width(), 700.0);
assert_eq!(bounds.height(), 500.0);
```

---

### GridLineConfig

Controls the visual appearance of a set of grid lines (major or minor).

```rust
pub struct GridLineConfig {
    pub enabled: bool,                  // Whether these lines are drawn
    pub color: [f32; 4],                // RGBA color values (0.0–1.0)
    pub line_width: f32,                // Width in pixels
    pub opacity: f32,                   // Opacity multiplier (0.0–1.0)
    pub dash_pattern: Option<Vec<f32>>, // Optional dash pattern
}
```

#### Builder Methods

All builder methods consume and return `self` for chaining:

```rust
use gup::GridLineConfig;

let config = GridLineConfig::default()
    .with_color([0.5, 0.5, 0.5, 1.0])
    .with_line_width(1.0)
    .with_opacity(0.8)
    .with_dash_pattern(vec![5.0, 3.0]);
```

| Method                        | Description                         |
| ----------------------------- | ----------------------------------- |
| `with_color([f32; 4])`        | Set RGBA color                      |
| `with_line_width(f32)`        | Set pixel width                     |
| `with_opacity(f32)`           | Set opacity multiplier              |
| `with_dash_pattern(Vec<f32>)` | Enable dashed lines                 |
| `disabled()`                  | Create a disabled config            |
| `minor_default()`             | Default config for minor grid lines |

#### Default Values

| Field          | Major Default          | Minor Default          |
| -------------- | ---------------------- | ---------------------- |
| `enabled`      | `true`                 | `false`                |
| `color`        | `[0.8, 0.8, 0.8, 1.0]` | `[0.9, 0.9, 0.9, 1.0]` |
| `line_width`   | `0.5`                  | `0.25`                 |
| `opacity`      | `0.6`                  | `0.3`                  |
| `dash_pattern` | `None` (solid)         | `None` (solid)         |

---

### GridConfiguration

Complete grid appearance configuration with major/minor grid settings and
directional controls.

```rust
pub struct GridConfiguration {
    pub major_grid: GridLineConfig,  // Major grid line settings
    pub minor_grid: GridLineConfig,  // Minor grid line settings
    pub show_horizontal: bool,       // Enable horizontal grid lines
    pub show_vertical: bool,         // Enable vertical grid lines
}
```

#### Construction Methods

| Method                                 | Description                     |
| -------------------------------------- | ------------------------------- |
| `GridConfiguration::default()`         | Major H+V grids, minor disabled |
| `GridConfiguration::horizontal_only()` | Horizontal grids only           |
| `GridConfiguration::vertical_only()`   | Vertical grids only             |

#### Builder Methods

```rust
use gup::GridConfiguration;

let config = GridConfiguration::default()
    .with_minor_grid()                               // Enable minor grids
    .with_major_grid(GridLineConfig::default()        // Custom major config
        .with_line_width(1.0))
    .with_minor_grid_config(GridLineConfig::default() // Custom minor config
        .with_opacity(0.2));
```

| Method                                   | Description              |
| ---------------------------------------- | ------------------------ |
| `with_minor_grid()`                      | Enable minor grid lines  |
| `without_minor_grid()`                   | Disable minor grid lines |
| `with_major_grid(GridLineConfig)`        | Set major grid config    |
| `with_minor_grid_config(GridLineConfig)` | Set minor grid config    |

#### Theme Presets

| Method            | Major Color  | Major Width | Minor                | Direction       | Best For           |
| ----------------- | ------------ | ----------- | -------------------- | --------------- | ------------------ |
| `light_theme()`   | Black α=0.15 | 0.5         | Off                  | Both            | Bright backgrounds |
| `dark_theme()`    | White α=0.25 | 0.5         | Off                  | Both            | Dark backgrounds   |
| `scientific()`    | Gray 0.3     | 0.75        | On (gray 0.7, α=0.4) | Both            | Publications       |
| `business()`      | Gray 0.9     | 0.5         | Off                  | Horizontal only | Dashboards         |
| `minimal()`       | Gray 0.95    | 0.25        | Off                  | Both            | Minimalist design  |
| `high_contrast()` | Black        | 1.0         | On (gray 0.4)        | Both            | Accessibility      |

---

### GridRenderer

GPU-accelerated grid line renderer with geometry caching.

#### Key Methods

| Method                     | Description                                       |
| -------------------------- | ------------------------------------------------- |
| `new()`                    | Create an empty renderer                          |
| `render_grid(...)`         | Generate + render grid lines (cached)             |
| `generate_grid_lines(...)` | Generate lines without rendering (for benchmarks) |
| `clear_grid_lines()`       | Remove all generated lines                        |
| `total_line_count()`       | Count of all generated lines                      |
| `major_lines()`            | Iterator over major line attributes               |
| `minor_lines()`            | Iterator over minor line attributes               |
| `invalidate_cache()`       | Force regeneration on next render                 |
| `cache_hit_rate()`         | Cache efficiency (0.0–1.0)                        |
| `cache_stats()`            | `(hits, misses)` tuple                            |

#### Static Line Generation

For direct line generation without a `GridRenderer` instance:

```rust
use gup::grid::{GridRenderer, GridLineConfig, ChartBounds};

let bounds = ChartBounds::new(0.0, 800.0, 0.0, 600.0);
let config = GridLineConfig::default();
let y_ticks = vec![100.0, 200.0, 300.0, 400.0, 500.0];

let mut lines = Vec::new();
GridRenderer::generate_horizontal_lines_static(
    &y_ticks, bounds, &config, &mut lines
)?;
// lines now contains 5 horizontal LineAttributes
```

---

### GridSystem

High-level grid coordinator that manages configuration and rendering together.

```rust
use gup::grid::{GridSystem, GridConfiguration};

let mut grid = GridSystem::new(GridConfiguration::scientific());

// Or use defaults:
let mut grid = GridSystem::with_defaults();
```

| Method                      | Description                         |
| --------------------------- | ----------------------------------- |
| `new(config)`               | Create with specific configuration  |
| `with_defaults()`           | Create with default configuration   |
| `render_grid(...)`          | Render using internal config        |
| `set_configuration(config)` | Update configuration                |
| `configuration()`           | Get current configuration reference |
| `total_line_count()`        | Number of generated lines           |
| `is_grid_enabled()`         | Whether any grid lines will render  |

---

### AxisGridCoordinator

Coordinates axis tick positions with grid rendering for perfect alignment.

```rust
use gup::grid::{AxisGridCoordinator, GridConfiguration};

let mut coordinator = AxisGridCoordinator::new(GridConfiguration::default());

// Render axes and grid together with proper z-ordering
coordinator.render_axes_and_grid(
    &mut context,
    &axes,      // &[Box<dyn Axis>]
    &scales,    // &[Option<&dyn Scale>]
    chart_bounds,
)?;
```

| Method                           | Description                            |
| -------------------------------- | -------------------------------------- |
| `new(config)`                    | Create coordinator                     |
| `render_axes_and_grid(...)`      | Render grid (behind) then axes (front) |
| `set_grid_configuration(config)` | Update grid config                     |
| `grid_configuration()`           | Get current config reference           |
| `total_grid_line_count()`        | Number of grid lines                   |
| `is_grid_enabled()`              | Whether grid rendering is active       |

---

### GridCapableBuilder Trait

The `GridCapableBuilder` trait adds grid methods to chart builders. It is
implemented for `ScatterPlotBuilder`, `LineChartBuilder`, `BoxPlotBuilder`,
`BarChartBuilder`, `AreaChartBuilder`, and `HeatmapBuilder`.

#### Core Configuration Methods

These methods require implementation by each builder:

```rust
fn major_grid_style(self, config: GridLineConfig) -> Self;
fn minor_grid_style(self, config: GridLineConfig) -> Self;
fn horizontal_grid_only(self) -> Self;
fn vertical_grid_only(self) -> Self;
fn with_minor_grid(self) -> Self;
fn without_minor_grid(self) -> Self;
fn grid_configuration(self, config: GridConfiguration) -> Self;
```

#### Convenience Methods (provided defaults)

These are automatically available on any `GridCapableBuilder` implementor:

```rust
fn grid(self) -> Self;                              // Enable with defaults
fn horizontal_grid(self) -> Self;                   // Horizontal lines only
fn vertical_grid(self) -> Self;                     // Vertical lines only
fn grid_color(self, color: impl Into<Color>) -> Self;
fn grid_opacity(self, opacity: f32) -> Self;
fn grid_width(self, width: f32) -> Self;
fn light_grid(self) -> Self;
fn dark_grid(self) -> Self;
fn scientific_grid(self) -> Self;
fn business_grid(self) -> Self;
fn minimal_grid(self) -> Self;
fn high_contrast_grid(self) -> Self;
```

---

## Configuration Guide

### Choosing the Right Theme

| Scenario                | Recommended Theme      | Why                              |
| ----------------------- | ---------------------- | -------------------------------- |
| Web application (light) | `light_grid()`         | Subtle on white backgrounds      |
| Dashboard (dark mode)   | `dark_grid()`          | Visible on dark backgrounds      |
| Scientific paper        | `scientific_grid()`    | Minor grids for precision        |
| Executive report        | `business_grid()`      | Clean horizontal-only look       |
| Design-focused          | `minimal_grid()`       | Nearly invisible support lines   |
| Accessible design       | `high_contrast_grid()` | Maximum visibility for all users |

### Custom Grid Styling

For fine-grained control, combine `GridLineConfig` and `GridConfiguration`:

```rust
use gup::{GridConfiguration, GridLineConfig};

let config = GridConfiguration {
    major_grid: GridLineConfig {
        enabled: true,
        color: [0.3, 0.3, 0.3, 1.0],
        line_width: 1.0,
        opacity: 0.6,
        dash_pattern: None,
    },
    minor_grid: GridLineConfig {
        enabled: true,
        color: [0.7, 0.7, 0.7, 1.0],
        line_width: 0.5,
        opacity: 0.4,
        dash_pattern: None,
    },
    show_horizontal: true,
    show_vertical: true,
};
```

### Color Specification

The `grid_color()` method accepts anything that implements `Into<Color>`:

```rust
// Hex string (3 or 6 characters, with or without #)
.grid_color("#cccccc")
.grid_color("#ccc")

// RGB tuple (alpha defaults to 1.0)
.grid_color((0.8, 0.8, 0.8))

// RGBA tuple
.grid_color((0.8, 0.8, 0.8, 0.5))

// RGBA array
.grid_color([0.8, 0.8, 0.8, 0.5])
```

### Effective Opacity

The final opacity of a grid line is the product of the `color` alpha channel and
the `opacity` field:

```
effective_alpha = color[3] * opacity
```

For example, with `color: [0.5, 0.5, 0.5, 0.8]` and `opacity: 0.5`, the
effective alpha is `0.8 × 0.5 = 0.4`.

---

## Examples and Tutorials

### Tutorial 1: Basic Scatter Plot with Grid

```rust
use gup::chart_builder::ScatterPlotBuilder;

#[derive(Debug, Clone)]
struct DataPoint {
    pub revenue: f32,
    pub profit: f32,
}

fn main() -> gup::GupResult<()> {
    let data = vec![
        DataPoint { revenue: 15.0, profit: 5.2 },
        DataPoint { revenue: 45.0, profit: 12.1 },
        DataPoint { revenue: 72.0, profit: 18.5 },
        DataPoint { revenue: 95.0, profit: 22.8 },
    ];

    let chart = ScatterPlotBuilder::new()
        .data(data)
        .grid()
        .build()?;

    Ok(())
}
```

### Tutorial 2: Scientific Chart with Minor Grids

```rust
use gup::chart_builder::ScatterPlotBuilder;

// Scientific theme enables minor grids automatically
let chart = ScatterPlotBuilder::new()
    .data(data)
    .scientific_grid()
    .build()?;

// Or manually enable minor grids on any theme:
let chart = ScatterPlotBuilder::new()
    .data(data)
    .grid()
    .with_minor_grid()
    .build()?;
```

### Tutorial 3: Business Dashboard with Horizontal Grid Only

```rust
use gup::chart_builder::ScatterPlotBuilder;

// Business theme: clean horizontal lines, no vertical clutter
let chart = ScatterPlotBuilder::new()
    .data(data)
    .business_grid()
    .build()?;

// Or use directional shortcut:
let chart = ScatterPlotBuilder::new()
    .data(data)
    .horizontal_grid()
    .grid_color("#e0e0e0")
    .grid_opacity(0.7)
    .build()?;
```

### Tutorial 4: Custom Grid with Advanced Configuration

```rust
use gup::{GridConfiguration, GridLineConfig};
use gup::chart_builder::ScatterPlotBuilder;

let custom_config = GridConfiguration {
    major_grid: GridLineConfig {
        enabled: true,
        color: [0.2, 0.4, 0.6, 1.0],  // Blue-gray major lines
        line_width: 0.75,
        opacity: 0.5,
        dash_pattern: None,
    },
    minor_grid: GridLineConfig {
        enabled: true,
        color: [0.6, 0.7, 0.8, 1.0],  // Light blue minor lines
        line_width: 0.25,
        opacity: 0.25,
        dash_pattern: Some(vec![4.0, 2.0]),  // Dashed minor lines
    },
    show_horizontal: true,
    show_vertical: true,
};

let chart = ScatterPlotBuilder::new()
    .data(data)
    .grid_configuration(custom_config)
    .build()?;
```

### Tutorial 5: Low-Level Grid Rendering

For custom chart implementations that don't use the chart builder:

```rust
use gup::grid::{GridSystem, GridConfiguration, GridRenderer, ChartBounds};

let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
let mut grid_system = GridSystem::new(GridConfiguration::scientific());

// Generate grid lines from tick positions
let x_ticks = vec![100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0];
let y_ticks = vec![100.0, 200.0, 300.0, 400.0, 500.0];
let x_minor = vec![150.0, 250.0, 350.0, 450.0, 550.0, 650.0];
let y_minor = vec![150.0, 250.0, 350.0, 450.0];

grid_system.render_grid(
    &mut context,
    &x_ticks, &y_ticks,
    &x_minor, &y_minor,
    bounds,
)?;

println!("Rendered {} grid lines", grid_system.total_line_count());
```

### Tutorial 6: Inspecting Generated Grid Lines

```rust
use gup::grid::{GridRenderer, GridConfiguration, ChartBounds, GridLineConfig};

let bounds = ChartBounds::new(0.0, 100.0, 0.0, 100.0);
let config = GridConfiguration::scientific();
let mut renderer = GridRenderer::new();

let line_count = renderer.generate_grid_lines(
    &[20.0, 40.0, 60.0, 80.0],  // horizontal ticks
    &[25.0, 50.0, 75.0],         // vertical ticks
    &[10.0, 30.0, 50.0, 70.0, 90.0],  // horizontal minor
    &[12.5, 37.5, 62.5, 87.5],         // vertical minor
    bounds,
    &config,
)?;

println!("Generated {} lines", line_count);

// Inspect individual lines
for line in renderer.major_lines() {
    println!(
        "Major: ({}, {}) → ({}, {}), width={}",
        line.start.x, line.start.y,
        line.end.x, line.end.y,
        line.width
    );
}

for line in renderer.minor_lines() {
    println!(
        "Minor: ({}, {}) → ({}, {}), width={}",
        line.start.x, line.start.y,
        line.end.x, line.end.y,
        line.width
    );
}
```

---

## Performance Guide

### Performance Characteristics

The Grid Line Rendering System is designed for high performance:

- **< 0.05 ms rendering** for 20 grid lines on standard hardware
- **Linear scaling** with grid line count
- **Minimal memory overhead** — < 10% additional GPU memory usage
- **No data impact** — grid rendering does not affect data point performance
- **Geometry caching** — zero-cost rendering for static views

### Optimization Recommendations

#### Grid Line Count

| Range  | Performance Impact           | Recommendation            |
| ------ | ---------------------------- | ------------------------- |
| 1–20   | Negligible                   | Default range, optimal    |
| 20–50  | Negligible                   | Good for detailed views   |
| 50–100 | Minimal                      | Acceptable for dense data |
| 100+   | May affect lower-end devices | Console warning emitted   |

#### Configuration Choices

| Configuration        | Relative Cost      | Use Case             |
| -------------------- | ------------------ | -------------------- |
| Major grids only     | Baseline           | Most visualizations  |
| Major + minor grids  | ~1.5–2× baseline   | Scientific precision |
| One direction only   | ~0.5× baseline     | Business dashboards  |
| Custom dash patterns | Minimal extra cost | Styling flexibility  |

#### Caching Behavior

The `GridRenderer` caches generated geometry based on a fingerprint hash of:

- Tick positions (horizontal and vertical, major and minor)
- Chart bounds (left, right, top, bottom)
- Configuration flags (enabled state, line widths)

**Cache hits** skip geometry regeneration entirely, making grid rendering
virtually free for static views. The cache is automatically invalidated when any
input changes.

Monitor cache performance:

```rust
let (hits, misses) = renderer.cache_stats();
let hit_rate = renderer.cache_hit_rate();
println!("Cache: {:.1}% hit rate ({} hits, {} misses)", hit_rate * 100.0, hits, misses);
```

Force a cache invalidation when external factors change:

```rust
renderer.invalidate_cache();
```

### Platform Considerations

| Platform                           | Expected Performance |
| ---------------------------------- | -------------------- |
| Native desktop (Vulkan/Metal/DX12) | Full performance     |
| WebAssembly (WebGPU)               | 85–95% of native     |
| Mobile (native)                    | Full performance     |

---

## Troubleshooting

### Grid Lines Not Appearing

**Symptom**: Grid is enabled but no lines are visible.

**Possible causes and solutions**:

1. **Grid not enabled**: Ensure you call `.grid()` or `.show_grid(true)` on the
   chart builder.

2. **Zero opacity**: Check that `grid_opacity` is not set to `0.0`.

3. **Color matches background**: If your grid color is the same as the
   background, lines will be invisible. Use a contrasting color or try
   `.high_contrast_grid()` to verify.

4. **Tick positions outside bounds**: Grid lines are only generated for tick
   values within the `ChartBounds`. If all ticks fall outside, no lines will
   render.

5. **Both directions disabled**: Setting both `show_horizontal` and
   `show_vertical` to `false` disables all grid lines.

### Grid Lines Too Prominent

**Symptom**: Grid lines dominate the visualization.

**Solutions**:

- Reduce opacity: `.grid_opacity(0.3)`
- Use a lighter color: `.grid_color("#eeeeee")`
- Reduce line width: `.grid_width(0.25)`
- Try the minimal theme: `.minimal_grid()`
- Show only one direction: `.horizontal_grid()` or `.vertical_grid()`

### Minor Grid Lines Not Showing

**Symptom**: Minor grids enabled but not visible.

**Check**:

1. Minor grids are disabled by default. Ensure `minor_grid.enabled` is `true`.
2. Use `.with_minor_grid()` on the builder or `.scientific_grid()` theme.
3. Minor grid opacity (default `0.3`) may be too subtle on some backgrounds.
   Increase with a custom `GridLineConfig`.

### Performance Warning Messages

**Symptom**: Console warning about grid line count.

**Message**: `"Warning: Rendering N grid lines may impact performance"`

This warning appears when more than 50 grid lines are generated. To resolve:

- Reduce the number of axis ticks
- Disable minor grids if not needed
- Show grid lines in only one direction

### Misaligned Grid Lines

**Symptom**: Grid lines don't align with axis tick marks.

**Cause**: Grid tick positions must match axis tick positions. When using the
chart builder API and `AxisGridCoordinator`, alignment is automatic. If using
the low-level API directly, ensure you pass the same tick values to both the
axis system and the grid renderer.

---

## Advanced Topics

### Custom Chart Integration

When building a custom chart type that doesn't use the built-in chart builders,
you can integrate the grid system directly:

```rust
use gup::grid::{AxisGridCoordinator, GridConfiguration, ChartBounds};

struct CustomChart {
    coordinator: AxisGridCoordinator,
    bounds: ChartBounds,
}

impl CustomChart {
    fn new() -> Self {
        Self {
            coordinator: AxisGridCoordinator::new(GridConfiguration::default()),
            bounds: ChartBounds::new(50.0, 750.0, 50.0, 550.0),
        }
    }

    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Grid renders behind data automatically
        self.coordinator.render_axes_and_grid(
            context,
            &self.axes,
            &self.scales,
            self.bounds,
        )?;

        // Then render data on top
        self.render_data(context)?;

        Ok(())
    }
}
```

### Dynamic Grid Updates

Grid configuration can be changed at runtime:

```rust
// Start with default grid
let mut grid_system = GridSystem::with_defaults();

// Switch to scientific theme for detailed view
grid_system.set_configuration(GridConfiguration::scientific());

// Cache is automatically invalidated on configuration change
```

### Extending Grid Functionality

The `GridLineConfig` struct is fully public, so you can create reusable custom
configurations:

```rust
fn corporate_grid() -> GridConfiguration {
    GridConfiguration {
        major_grid: GridLineConfig {
            enabled: true,
            color: [0.0, 0.2, 0.4, 0.15],  // Corporate blue tint
            line_width: 0.5,
            opacity: 1.0,
            dash_pattern: None,
        },
        minor_grid: GridLineConfig::disabled(),
        show_horizontal: true,
        show_vertical: false,
    }
}
```

### Grid-Axis Coordination Details

The `AxisGridCoordinator` handles the complete integration between axes and
grids:

1. **Tick collection**: Iterates over all axes and collects major and minor tick
   positions, converting normalized positions (0.0–1.0) to world coordinates
   using the chart bounds.

2. **Direction mapping**: Bottom/Top axis ticks produce vertical grid lines;
   Left/Right axis ticks produce horizontal grid lines.

3. **Render ordering**: Grid lines render first, then axes render on top,
   ensuring proper visual layering.

4. **Scale integration**: Axes can provide tick positions based on their
   associated scales, ensuring grid lines always match the data domain.

### Benchmarking Grid Performance

Use the `generate_grid_lines()` method for CPU-only benchmarking without
requiring a GPU context:

```rust
use gup::grid::{GridRenderer, GridConfiguration, ChartBounds};

let bounds = ChartBounds::new(0.0, 1000.0, 0.0, 1000.0);
let config = GridConfiguration::scientific();
let mut renderer = GridRenderer::new();

// Generate 50 tick positions
let major_ticks: Vec<f64> = (0..10).map(|i| i as f64 * 100.0).collect();
let minor_ticks: Vec<f64> = (0..50).map(|i| i as f64 * 20.0).collect();

let start = std::time::Instant::now();
let count = renderer.generate_grid_lines(
    &major_ticks, &major_ticks,
    &minor_ticks, &minor_ticks,
    bounds, &config,
)?;
let elapsed = start.elapsed();
println!("Generated {} lines in {:?}", count, elapsed);

// Second call should be a cache hit
let start = std::time::Instant::now();
let count = renderer.generate_grid_lines(
    &major_ticks, &major_ticks,
    &minor_ticks, &minor_ticks,
    bounds, &config,
)?;
let elapsed = start.elapsed();
println!("Cached: {} lines in {:?} (hit rate: {:.0}%)",
    count, elapsed, renderer.cache_hit_rate() * 100.0);
```

You can also compute the fingerprint directly for diagnostic purposes:

```rust
let fingerprint = GridRenderer::compute_fingerprint_public(
    &major_ticks, &major_ticks,
    &minor_ticks, &minor_ticks,
    bounds, &config,
);
println!("Grid fingerprint: {}", fingerprint);
```

---

## Related Documentation

- [Custom Mark Guide](CUSTOM_MARK_GUIDE.md) — Building custom marks including
  line marks used by the grid system
- [Migration Guide](MIGRATION_FROM_OBSERVABLE_PLOT.md) — Transitioning to Gup's
  chart builder API
- [Accessibility Guide](ACCESSIBILITY_KNOWN_ISSUES.md) — Grid accessibility
  considerations including high-contrast themes
- [Tutorial 1: Getting Started](tutorials/01_getting_started.md) — Create your
  first chart with Gup, including grid and axes configuration
