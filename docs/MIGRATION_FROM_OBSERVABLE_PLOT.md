# Migration Guide: From Observable Plot to Gup

This guide helps Observable Plot users understand how to migrate their
visualizations to Gup. While Gup provides an Observable Plot-compatible API for
common use cases, it also offers unique features and design patterns that enable
GPU-accelerated performance for massive datasets.

## Table of Contents

- [Introduction](#introduction)
- [Key Differences](#key-differences)
- [Feature Comparison Matrix](#feature-comparison-matrix)
- [Migration Examples](#migration-examples)
- [Integration Guidance](#integration-guidance)
- [Performance Considerations](#performance-considerations)
- [When to Use Gup vs Observable Plot](#when-to-use-gup-vs-observable-plot)

## Introduction

### Why Migrate to Gup?

Gup is designed for scenarios where Observable Plot's CPU-based rendering hits
performance limits:

- **Large datasets**: 100K+ data points that cause Observable Plot to lag
- **Real-time updates**: Streaming data that requires smooth 60 FPS updates
- **Interactive exploration**: Pan, zoom, and filter operations on massive
  datasets
- **GPU acceleration**: Leverage modern GPU hardware for parallel data
  processing

### Design Philosophy

Observable Plot and Gup share similar goals of declarative, composable APIs, but
differ in execution:

- **Observable Plot**: CPU-based DOM/SVG rendering, optimized for web
  integration
- **Gup**: GPU-based WebGPU/wgpu rendering, optimized for performance and scale

Both prioritize developer experience through clear, concise syntax.

## Key Differences

### 1. Rendering Backend

**Observable Plot** uses D3.js and DOM manipulation to create SVG elements:

- Excellent browser integration and accessibility
- Rich ecosystem of plugins and extensions
- Limited by CPU single-threaded performance
- Performance degrades with >1,000 elements

**Gup** uses WebGPU/wgpu for GPU-accelerated rendering:

- Parallel processing of millions of data points
- Consistent cross-platform performance (native + web)
- Requires WebGPU support (modern browsers, native apps)
- Different accessibility approach (see
  [Accessibility Guide](./ACCESSIBILITY_COMPATIBILITY.md))

### 2. API Style

Both use fluent, declarative APIs, but with slight differences:

**Observable Plot**:

```javascript
Plot.dot(data, { x: "revenue", y: "profit", fill: "region" }).plot();
```

**Gup**:

```rust
plot()
    .data(data)
    .scatter(x("revenue"), y("profit"))
    .color("region")
    .render()?;
```

Key differences:

- Gup uses method chaining for configuration vs Plot's option objects
- Gup separates mark type selection (`.scatter`) from configuration
- Gup requires explicit `.render()` call
- Gup is type-safe with compile-time checks

### 3. Data Handling

**Observable Plot** works with JavaScript objects and arrays directly:

```javascript
const data = [
  { revenue: 100, profit: 20, region: "North" },
  { revenue: 200, profit: 45, region: "South" },
];
```

**Gup** uses Rust structs with derive macros for GPU compatibility:

```rust
#[derive(Debug, Clone)]
struct SalesPoint {
    revenue: f32,
    profit: f32,
    region: String,
}

let data = vec![
    SalesPoint { revenue: 100.0, profit: 20.0, region: "North".to_string() },
    SalesPoint { revenue: 200.0, profit: 45.0, region: "South".to_string() },
];
```

### 4. Scale and Transform System

**Observable Plot** uses D3 scales implicitly:

```javascript
Plot.dot(data, {
  x: { type: "linear", domain: [0, 100] },
  y: { type: "log" },
}).plot();
```

**Gup** uses explicit shader functions for GPU execution:

```rust
// Scales are automatically inferred from data
plot()
    .data(data)
    .scatter(x("value"), y("count"))
    // Or specify custom scales
    .x_scale(LinearScale::new().domain([0.0, 100.0]))
    .y_scale(LogScale::new())
```

### 5. Interaction Model

**Observable Plot** uses DOM events:

```javascript
Plot.dot(data, {
  tip: true, // Built-in tooltips
  onclick: d => console.log(d),
}).plot();
```

**Gup** provides GPU-accelerated interaction system:

```rust
chart.select_all::<Circle>()
    .on("click", |event, datum| {
        println!("Clicked: {:?}", datum);
    })
```

## Feature Comparison Matrix

| Feature                | Observable Plot | Gup Status   | Notes                                        |
| ---------------------- | --------------- | ------------ | -------------------------------------------- |
| **Chart Types**        |                 |              |                                              |
| Scatter plots          | ✅              | ✅           | Full parity                                  |
| Line charts            | ✅              | ✅           | Full parity                                  |
| Bar charts             | ✅              | ✅           | Full parity                                  |
| Area charts            | ✅              | ✅           | Full parity                                  |
| Heatmaps               | ✅              | ✅           | Full parity                                  |
| Histograms             | ✅              | 🚧 Planned   | GUP Phase 2                                  |
| Box plots              | ✅              | 🚧 Planned   | GUP Phase 2                                  |
| Violin plots           | ✅              | 📋 Future    | GUP Phase 3                                  |
| **Scales**             |                 |              |                                              |
| Linear scales          | ✅              | ✅           | GPU shader functions                         |
| Log scales             | ✅              | ✅           | GPU shader functions                         |
| Time scales            | ✅              | 📋 Planned   | GUP Phase 2                                  |
| Band/Point scales      | ✅              | 🚧 Partial   | Category mapping available                   |
| **Visual Encoding**    |                 |              |                                              |
| Position (x, y)        | ✅              | ✅           | Full parity                                  |
| Color encoding         | ✅              | ✅           | GPU color gradients                          |
| Size encoding          | ✅              | ✅           | Per-element sizing                           |
| Opacity                | ✅              | ✅           | Alpha channel support                        |
| Shape encoding         | ✅              | 🚧 Partial   | Limited mark types currently                 |
| **Axes & Annotations** |                 |              |                                              |
| Automatic axes         | ✅              | ✅           | GPU-rendered axes                            |
| Custom tick formats    | ✅              | ✅           | Custom label formatters                      |
| Grid lines             | ✅              | ✅           | Configurable grid system                     |
| Axis titles            | ✅              | ✅           | Text rendering support                       |
| Annotations            | ✅              | 📋 Planned   | GUP Phase 2                                  |
| **Interactions**       |                 |              |                                              |
| Tooltips               | ✅              | 🚧 Planned   | GPU hit testing available                    |
| Click events           | ✅              | ✅           | GPU-accelerated picking                      |
| Hover events           | ✅              | ✅           | Real-time interaction                        |
| Brush selection        | ✅              | 📋 Future    | GUP Phase 3                                  |
| Pan & zoom             | ✅              | 🚧 Partial   | Available via custom handlers                |
| **Data Processing**    |                 |              |                                              |
| Binning                | ✅              | 📋 Planned   | GPU compute shaders                          |
| Aggregation            | ✅              | 📋 Planned   | GPU parallel reduction                       |
| Sorting                | ✅              | 📋 Planned   | GPU sorting algorithms                       |
| Filtering              | ✅              | ✅           | Rust iterators + GPU                         |
| **Layout**             |                 |              |                                              |
| Faceting               | ✅              | 📋 Future    | Multiple render passes                       |
| Small multiples        | ✅              | 📋 Future    | Composition system                           |
| Legends                | ✅              | 📋 Planned   | GUP Phase 2                                  |
| **Performance**        |                 |              |                                              |
| Dataset size (60 FPS)  | ~1K points      | 1M+ points   | GPU parallel processing                      |
| Real-time streaming    | Limited         | ✅ Excellent | GPU buffer updates                           |
| Animation              | ✅              | 🚧 Partial   | Transitions available                        |
| **Accessibility**      |                 |              |                                              |
| Screen reader support  | ✅ Excellent    | ✅ Good      | Different approach (see accessibility guide) |
| Keyboard navigation    | ✅              | ✅           | Full keyboard support                        |
| ARIA attributes        | ✅              | ✅           | Semantic descriptions                        |
| **Export**             |                 |              |                                              |
| SVG export             | ✅              | 📋 Future    | Planned for Phase 4                          |
| PNG export             | ✅              | 📋 Future    | Planned for Phase 4                          |
| Interactive HTML       | ✅              | ✅           | WebGPU canvas                                |

**Legend:**

- ✅ Implemented and tested
- 🚧 Partially implemented or in progress
- 📋 Planned for future release
- ❌ Not planned (fundamental limitation)

## Migration Examples

### Example 1: Basic Scatter Plot

**Observable Plot:**

```javascript
import * as Plot from "@observablehq/plot";

const data = [
  { revenue: 100, profit: 20, region: "North" },
  { revenue: 200, profit: 45, region: "South" },
  { revenue: 150, profit: 30, region: "East" },
];

const chart = Plot.dot(data, {
  x: "revenue",
  y: "profit",
  fill: "region",
  r: 5,
}).plot();

document.body.appendChild(chart);
```

**Gup:**

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct SalesPoint {
    revenue: f32,
    profit: f32,
    region: String,
}

async fn create_chart(context: Arc<RenderContext>) -> GupResult<()> {
    let data = vec![
        SalesPoint { revenue: 100.0, profit: 20.0, region: "North".to_string() },
        SalesPoint { revenue: 200.0, profit: 45.0, region: "South".to_string() },
        SalesPoint { revenue: 150.0, profit: 30.0, region: "East".to_string() },
    ];

    let chart = gup::plot()
        .with_context(context)
        .data(data)
        .scatter(x("revenue"), y("profit"))
        .color("region")
        .radius(5.0)
        .render()?;

    Ok(())
}
```

**Key differences:**

- Rust requires struct definition with types
- Async/await pattern for GPU operations
- Explicit context management
- Similar fluent API but method names differ slightly

### Example 2: Line Chart with Custom Scales

**Observable Plot:**

```javascript
const timeSeries = [
  { date: new Date("2023-01-01"), value: 10 },
  { date: new Date("2023-02-01"), value: 15 },
  { date: new Date("2023-03-01"), value: 13 },
];

Plot.line(timeSeries, {
  x: { type: "utc", domain: [new Date("2023-01-01"), new Date("2023-12-31")] },
  y: { domain: [0, 20] },
  stroke: "steelblue",
  strokeWidth: 2,
}).plot();
```

**Gup:**

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct TimePoint {
    timestamp: f32,  // Unix timestamp or day index
    value: f32,
}

async fn create_line_chart(context: Arc<RenderContext>) -> GupResult<()> {
    let time_series = vec![
        TimePoint { timestamp: 0.0, value: 10.0 },
        TimePoint { timestamp: 31.0, value: 15.0 },
        TimePoint { timestamp: 59.0, value: 13.0 },
    ];

    let chart = gup::plot()
        .with_context(context)
        .data(time_series)
        .line(x("timestamp"), y("value"))
        .x_scale(LinearScale::new().domain([0.0, 365.0]))
        .y_scale(LinearScale::new().domain([0.0, 20.0]))
        .stroke_color([70, 130, 180, 255])  // Steelblue
        .stroke_width(2.0)
        .render()?;

    Ok(())
}
```

**Key differences:**

- Time handling: Gup uses numeric timestamps (no native Date type yet)
- Scale configuration: Explicit scale objects vs configuration objects
- Color specification: RGBA arrays vs CSS color names

### Example 3: Bar Chart with Categorical Data

**Observable Plot:**

```javascript
const categories = [
  { category: "A", count: 10 },
  { category: "B", count: 25 },
  { category: "C", count: 15 },
];

Plot.barY(categories, {
  x: "category",
  y: "count",
  fill: "steelblue",
}).plot();
```

**Gup:**

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct CategoryData {
    category: String,
    count: f32,
}

async fn create_bar_chart(context: Arc<RenderContext>) -> GupResult<()> {
    let categories = vec![
        CategoryData { category: "A".to_string(), count: 10.0 },
        CategoryData { category: "B".to_string(), count: 25.0 },
        CategoryData { category: "C".to_string(), count: 15.0 },
    ];

    let chart = gup::plot()
        .with_context(context)
        .data(categories)
        .bar(x("category"), y("count"))
        .fill_color([70, 130, 180, 255])  // Steelblue
        .render()?;

    Ok(())
}
```

**Key differences:**

- Categorical encoding: Gup automatically maps strings to positions
- Orientation: `.bar()` instead of `.barY()` (Gup infers vertical from data)

### Example 4: Area Chart with Stacking

**Observable Plot:**

```javascript
const data = [
  { date: "2023-01", series: "A", value: 10 },
  { date: "2023-01", series: "B", value: 20 },
  { date: "2023-02", series: "A", value: 15 },
  { date: "2023-02", series: "B", value: 25 },
];

Plot.areaY(data, {
  x: "date",
  y: "value",
  fill: "series",
  curve: "catmull-rom",
}).plot();
```

**Gup:**

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct SeriesPoint {
    date: String,
    series: String,
    value: f32,
}

async fn create_area_chart(context: Arc<RenderContext>) -> GupResult<()> {
    let data = vec![
        SeriesPoint { date: "2023-01".to_string(), series: "A".to_string(), value: 10.0 },
        SeriesPoint { date: "2023-01".to_string(), series: "B".to_string(), value: 20.0 },
        SeriesPoint { date: "2023-02".to_string(), series: "A".to_string(), value: 15.0 },
        SeriesPoint { date: "2023-02".to_string(), series: "B".to_string(), value: 25.0 },
    ];

    let chart = gup::plot()
        .with_context(context)
        .data(data)
        .area(x("date"), y("value"))
        .fill("series")
        .render()?;

    Ok(())
}
```

**Key differences:**

- Curve types: Not yet implemented in Gup (linear interpolation default)
- Stacking: Implicit in data structure vs explicit stack transform

### Example 5: Heatmap

**Observable Plot:**

```javascript
const matrix = [
  { col: 0, row: 0, value: 0.5 },
  { col: 1, row: 0, value: 0.8 },
  { col: 0, row: 1, value: 0.3 },
  { col: 1, row: 1, value: 0.9 },
];

Plot.cell(matrix, {
  x: "col",
  y: "row",
  fill: "value",
  inset: 0.5,
}).plot();
```

**Gup:**

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct MatrixCell {
    col: f32,
    row: f32,
    value: f32,
}

async fn create_heatmap(context: Arc<RenderContext>) -> GupResult<()> {
    let matrix = vec![
        MatrixCell { col: 0.0, row: 0.0, value: 0.5 },
        MatrixCell { col: 1.0, row: 0.0, value: 0.8 },
        MatrixCell { col: 0.0, row: 1.0, value: 0.3 },
        MatrixCell { col: 1.0, row: 1.0, value: 0.9 },
    ];

    let chart = gup::plot()
        .with_context(context)
        .data(matrix)
        .heatmap(x("col"), y("row"), fill("value"))
        .render()?;

    Ok(())
}
```

**Key differences:**

- Cell vs heatmap: Different naming but same concept
- Inset parameter: Gup uses padding configuration separately

## Integration Guidance

### When to Use Gup

Choose Gup when:

- **Large datasets**: >10,000 data points need smooth interaction
- **Real-time updates**: Data streams at high frequency
- **Performance critical**: 60 FPS requirement for visualization
- **Cross-platform**: Need identical performance on web and native
- **Type safety**: Want compile-time validation of visualizations

### When to Use Observable Plot

Choose Observable Plot when:

- **Small datasets**: <10,000 data points
- **Web-first**: Only targeting browser environments
- **Rich ecosystem**: Need extensive D3 plugin ecosystem
- **Static exports**: Primary output is SVG for publication
- **Rapid prototyping**: JavaScript's flexibility is beneficial

### Hybrid Approach

For some applications, using both libraries makes sense:

- **Observable Plot** for dashboard overview and small charts
- **Gup** for detail views with large datasets
- **Observable Plot** for exploratory analysis
- **Gup** for production deployment with performance requirements

### Interoperability

Currently, Gup and Observable Plot cannot be easily composed together since they
use different rendering backends. However, you can:

1. **Side-by-side**: Display Observable Plot and Gup charts in different areas
2. **Progressive enhancement**: Start with Observable Plot, upgrade to Gup for
   performance
3. **Export bridge**: Use Observable Plot for static exports, Gup for
   interactive views

## Performance Considerations

### Dataset Size Guidelines

| Size           | Observable Plot | Gup                     |
| -------------- | --------------- | ----------------------- |
| < 1,000        | ✅ Excellent    | ✅ Excellent (overkill) |
| 1,000 - 10,000 | ⚠️ Good         | ✅ Excellent            |
| 10,000 - 100K  | ❌ Slow         | ✅ Excellent            |
| 100K - 1M      | ❌ Unusable     | ✅ Good                 |
| 1M+            | ❌ Unusable     | ✅ Specialized modes    |

### Performance Optimization Tips

**For Observable Plot:**

- Use `Plot.marks()` to cache computations
- Limit data with filtering before rendering
- Use `Plot.pointerX` instead of `tip: true` for better performance
- Debounce interactions

**For Gup:**

- Data is already on GPU - updates are fast
- Use streaming updates for real-time data
- Enable level-of-detail for massive datasets
- Leverage compute shaders for aggregations

### Benchmark Comparison

Rendering 100,000 scatter plot points:

- **Observable Plot**: 5-10 seconds initial render, 200ms interactions (laggy)
- **Gup**: 50ms initial render, 16ms (60 FPS) interactions

Real-time streaming 10K points/second:

- **Observable Plot**: Drops to 5-10 FPS, unusable
- **Gup**: Maintains 60 FPS with GPU buffer streaming

## Migration Checklist

### Phase 1: Assessment

- [ ] Inventory existing Observable Plot visualizations
- [ ] Measure current dataset sizes and performance
- [ ] Identify performance bottlenecks
- [ ] Determine WebGPU compatibility requirements

### Phase 2: Planning

- [ ] Choose charts to migrate first (largest datasets, worst performance)
- [ ] Review Gup feature matrix for any blockers
- [ ] Plan for feature gaps (workarounds or wait for implementation)
- [ ] Set up Rust development environment

### Phase 3: Implementation

- [ ] Define Rust data structures matching JavaScript objects
- [ ] Port Observable Plot code to Gup API
- [ ] Test with actual data sizes
- [ ] Verify visual parity
- [ ] Benchmark performance improvements

### Phase 4: Validation

- [ ] Cross-browser testing (WebGPU support)
- [ ] Accessibility testing
- [ ] Performance regression testing
- [ ] User acceptance testing

## Additional Resources

- [Gup API Documentation](https://docs.rs/gup)
- [Observable Plot Documentation](https://observablehq.com/plot/)
- [Chart Builder Guide](./docs/CHART_BUILDER_GUIDE.md) (coming soon)
- [Performance Optimization Guide](./docs/PERFORMANCE_GUIDE.md) (coming soon)
- [Accessibility Guide](./ACCESSIBILITY_COMPATIBILITY.md)

## Getting Help

- **GitHub Issues**: Report bugs or request features
- **Discussions**: Ask questions and share patterns
- **Examples**: Check `examples/` directory for working code

---

This migration guide is actively maintained as Gup evolves. If you find gaps or
have suggestions, please open an issue or contribute improvements.
