# GUP-245: Bar Chart Builder

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**:
2025-01-31

## Context

GUP-018 delivered Gup's primary chart builder API — `ScatterPlotBuilder`,
`ChartConfig`, and `ComposedChart` — establishing the fluent, Observable
Plot-style interface that all chart builders share. GUP-067 implemented the
`Rectangle` mark with GPU-instanced quad rendering, and GUP-093 delivered the
scale-axis integration system including automatic detection of categorical data
and ordinal scale support. Together these form a solid foundation upon which a
full bar chart builder can be constructed.

Bar charts are one of the most common chart types in data visualisation: they
map a categorical dimension (product names, time periods, survey responses) to a
numeric quantity, making comparisons across categories immediately readable.
Despite their conceptual simplicity, bar charts have significant configuration
surface area — orientation (vertical vs horizontal), grouping strategy (simple,
grouped, stacked), gap/padding between bars, and multi-series colour encoding —
that must all compose cleanly through the builder API.

GUP-254 (OrdinalScale GPU Shader Function) is a planned prerequisite that
provides the GPU-side function mapping a category index to a pixel position
along an axis. Without it, bar positions must be computed CPU-side and uploaded
per frame, which breaks the GPU-first architecture and prevents 100K+ bar
performance. This story therefore depends on GUP-254 being complete before the
`BarChartBuilder` can delegate position mapping to the GPU.

GUP-087 demonstrated that the chart builder pipeline itself can be optimised to
eliminate redundant CPU overhead. This story applies the same discipline: all
per-bar transforms (category-to-position, value-to-height, colour mapping) must
be expressed as composable GPU shader functions, not CPU closures applied during
data upload.

## User Story

> "As a visualisation developer, I want a fluent `BarChartBuilder` API that
> accepts categorical and numeric accessors, handles grouping and stacking
> automatically, and renders using GPU-instanced rectangles, so that I can
> produce professional bar charts for large datasets with a single method
> chain."
>
> "As a data analyst embedding Gup charts in a dashboard, I want to switch
> between vertical and horizontal bar orientations and between grouped and
> stacked layouts without restructuring my data pipeline, so that I can explore
> the most readable presentation for my audience."

## Acceptance Criteria

### AC1: Core BarChartBuilder API

- [ ] `BarChartBuilder<T>` is publicly exported from the `gup` crate
- [ ] `.data(Vec<T>)` binds the data and returns a `BoundBarChart<T>`
- [ ] `.x(impl Fn(&T) -> impl Into<Category>)` maps data to the categorical axis
- [ ] `.y(impl Fn(&T) -> f32)` maps data to the numeric axis
- [ ] `.color(impl Fn(&T) -> impl Into<Color>)` assigns bar fill colour
      (optional; defaults to a single theme colour)
- [ ] `.gap(f32)` sets the fractional gap between bar groups (0.0 = no gap, 1.0
      = bars invisible; default `0.1`)
- [ ] `.build()` / `.render()` returns a `ComposedChart` compatible with the
      existing `ChartConfig` / `ComposedChart` infrastructure from GUP-018

### AC2: Orientation Support

- [ ] `.orient(Orientation::Vertical)` (default) renders bars rising from the
      x-axis
- [ ] `.orient(Orientation::Horizontal)` renders bars extending from the y-axis
- [ ] Axis labels and tick positions swap automatically between orientations
- [ ] Both orientations render without GPU validation errors

### AC3: Grouped Bars

- [ ] `.group_by(impl Fn(&T) -> impl Into<Category>)` activates grouped mode,
      placing one bar per series within each category band
- [ ] Bar widths within each band are divided equally across series, respecting
      the `.gap()` setting
- [ ] An auto-generated legend entry is produced for each distinct group value
- [ ] Grouped charts render correctly for up to 8 distinct series

### AC4: Stacked Bars

- [ ] `.stack_by(impl Fn(&T) -> impl Into<Category>)` activates stacked mode,
      accumulating bar segments vertically (or horizontally)
- [ ] Segment order within a stack is stable (sorted by first occurrence in the
      input data)
- [ ] `.stack_by()` and `.group_by()` are mutually exclusive; calling both
      produces a compile-time or documented runtime error
- [ ] Stacked charts render correctly for up to 8 distinct series

### AC5: Ordinal Axis Integration

- [ ] The categorical axis is rendered using the ordinal scale from GUP-254,
      mapping category names to evenly-spaced pixel positions on the GPU
- [ ] Tick labels display each category name, rotated when label density exceeds
      a legibility threshold (>10 categories)
- [ ] The numeric axis uses `LinearScale` auto-domain from the data values,
      including stacked totals when `.stack_by()` is active
- [ ] Both axes inherit the `AxisConfig` styling from the surrounding
      `ChartConfig`

### AC6: GPU Rendering Performance

- [ ] 100 000 bars render at ≥ 60 FPS on the CI benchmark GPU (or headless
      adapter) as measured by the existing `criterion` benchmark harness
- [ ] Each bar is rendered as a GPU-instanced `Rectangle` mark; no per-bar draw
      calls
- [ ] Bar position, width, and height are computed by GPU shader functions; only
      category-index lookup tables and scale uniforms are uploaded per frame
- [ ] No GPU validation layer errors or warnings during rendering

### AC7: Example and Documentation

- [ ] A working example `examples/bar_chart.rs` demonstrates a simple vertical
      bar chart, a grouped bar chart, and a stacked bar chart against synthetic
      data
- [ ] The example compiles and runs: `cargo run --example bar_chart`
- [ ] Public API items carry `///` doc comments sufficient to appear in
      `cargo doc` output

## Technical Tasks

- [ ] Define `Category` newtype (wrapping `String` or `u32` index) and implement
      `From<&str>`, `From<String>`, `From<u32>`
- [ ] Define `Orientation` enum (`Vertical`, `Horizontal`)
- [ ] Define `BarLayout` enum (`Simple`, `Grouped`, `Stacked`) with associated
      series-key accessor storage
- [ ] Implement `BarChartBuilder<T>` struct with builder methods: `.data()`,
      `.x()`, `.y()`, `.color()`, `.gap()`, `.orient()`, `.group_by()`,
      `.stack_by()`
- [ ] Implement category de-duplication and stable index assignment (CPU-side,
      once, at `.build()` time)
- [ ] For stacked layout: implement CPU-side stack accumulation to compute
      per-bar baseline and height values before GPU upload
- [ ] Integrate `OrdinalScale` (GUP-254) as the GPU shader function for the
      categorical axis; upload category-count and range uniforms
- [ ] Wire the numeric axis to `LinearScale` with auto-domain that accounts for
      stacked totals
- [ ] Compute instanced `RectangleAttributes` (x, y, width, height, fill) from
      scale outputs for each bar; upload to GPU instance buffer
- [ ] Implement `.orient(Horizontal)` by swapping the axis roles and rotating
      the instance buffer computation
- [ ] Generate axis and tick geometry via GUP-093 `ScaleAxisIntegrator`, reusing
      existing `AxisConfig` plumbing
- [ ] Add auto-legend generation for grouped / stacked series
- [ ] Write unit tests for: category index assignment, stack accumulation, gap
      computation, orientation axis-swap
- [ ] Write integration test: build a `BarChartBuilder`, call `.build()`, assert
      the returned `ComposedChart` has the expected number of marks and no GPU
      errors
- [ ] Write `examples/bar_chart.rs` covering simple, grouped, and stacked cases
- [ ] Add a `criterion` benchmark: `benches/bar_chart_100k.rs` measuring render
      time for 100K bars

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot Chart Builders ✅ — provides `ChartConfig`,
  `ComposedChart`, and the fluent builder pattern this story extends
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides the `Rectangle`
  mark and GPU-instanced quad rendering used for every bar
- GUP-093: Scale-Axis Integration System ✅ — provides `ScaleAxisIntegrator`,
  `AxisConfig`, and automatic categorical axis plumbing
- GUP-254: OrdinalScale GPU Shader Function 📋 — provides the GPU shader
  function that maps a category index to a pixel position; required for GPU-side
  bar position computation

### Enables Stories

- GUP-251: Custom Composite Chart Support — bar charts are a primary target for
  composition (e.g. bars + line overlay); the `ComposedChart` output of this
  story is the direct input to GUP-251's mixing API
- GUP-275: Choropleth Chart Builder — reuses the bar chart's categorical layout
  patterns (ordinal scale, gap computation, legend generation) as the model for
  its own region-to-value mapping

## Testing Strategy

- **Unit tests**: Category de-duplication and stable ordering; stack
  accumulation arithmetic (baseline + height = total);
  gap-fraction-to-pixel-width conversion; horizontal-vs-vertical axis role swap
- **Integration tests**: Full `BarChartBuilder` pipeline — bind data, call
  `.build()`, assert returned mark count equals input row count (simple), input
  row count × 1 rectangle per bar (grouped/stacked), and that `ComposedChart`
  renders without GPU validation errors using the headless adapter
- **Visual validation**: Run `cargo run --example bar_chart` and capture a
  screenshot; verify bars are positioned at the correct category offsets and
  heights are proportional to values
- **Performance**: `benches/bar_chart_100k.rs` must report ≥ 60 FPS (≤ 16.7 ms
  per frame) for 100 000 bars on the CI benchmark adapter

## Success Metrics

- [ ] All seven Acceptance Criteria are fully checked off
- [ ] `cargo run --example bar_chart` produces visually correct output for
      simple, grouped, and stacked configurations
- [ ] The `bar_chart_100k` benchmark reports ≤ 16.7 ms render time (60 FPS) with
      no regressions against the existing benchmark baseline
- [ ] `cargo doc --no-deps` generates complete documentation for
      `BarChartBuilder` and all public associated types

## Risk Assessment

- **Medium**: OrdinalScale GPU shader (GUP-254) may not yet be merged when this
  story is picked up. _Mitigation_: The CPU-side category index table is
  self-contained; bar positions can be computed CPU-side as a temporary
  fallback, with the GPU shader wired in once GUP-254 is complete. Document this
  as a known limitation in the interim implementation.

- **Medium**: Stack accumulation requires a stable traversal order over
  (category, series) pairs. If input data is unsorted, the order must be
  deterministic across renders. _Mitigation_: Sort by (category index, series
  first-occurrence index) at `.build()` time and document the ordering contract.

- **Low**: Grouped bar width computation with many series (>8) can produce
  sub-pixel bars. _Mitigation_: Clamp minimum bar width to 1 px and emit a
  `tracing::warn!` when the clamp is triggered; document the recommended maximum
  series count.

- **Low**: The mutual-exclusivity constraint between `.group_by()` and
  `.stack_by()` could be enforced via typestate or at runtime. _Mitigation_:
  Start with a runtime `panic!` (consistent with existing builder patterns in
  the codebase) and note a typestate refactor as a follow-up if the API is
  extended.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
