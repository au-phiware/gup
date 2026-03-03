# GUP-247: Area Chart Builder

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-01-30

## Context

An area chart is conceptually a line chart whose region between the drawn line
and a baseline is filled — making it ideal for visualising cumulative values,
proportions over time, and stacked multi-series comparisons. GUP-018 sketched
the `AreaChartBuilder` as part of the original Observable Plot-style API
catalogue, and GUP-246 will deliver the companion `LineChartBuilder`. This story
realises the filled variant by composing the line stroke with a GPU-tessellated
filled polygon, leaning on the Path mark tessellation pipeline established by
GUP-132.

The key complexity over a plain line chart is the closed polygon: the area shape
must be correctly closed at both ends (connecting the upper path to the baseline
and back), even when the baseline itself varies per data point (band/ribbon
areas). Stacked area charts add a second layer of complexity: each series' lower
boundary is the cumulative sum of all series below it, requiring a pre-pass over
the data before any GPU upload.

Normalised stacked (100%) areas divide each series' value by the per-x-bin total
so that all series together always fill exactly the chart height — a common
pattern for visualising relative composition. Gradient fills along the y-axis
map the existing `ColorScale` machinery from prior chart builders onto the
tessellated polygon's vertex colours, enabling smooth continuous fills at no
extra draw call cost.

## User Story

> "As a visualization developer, I want an `AreaChartBuilder` with a fluent Rust
> API so that I can render filled area charts — including stacked, normalised,
> and band variants — with GPU-efficient performance and without writing
> low-level shader or geometry code."

## Acceptance Criteria

### AC1: Basic Area Chart

- [ ] `AreaChartBuilder::new()` is publicly constructable and accepts generic
      record types via `.data(Vec<T>)`
- [ ] `.x(accessor)`, `.y(accessor)`, and `.y0(accessor | f32)` are fluent
      methods that compile with any `Fn(&T) -> f32` or equivalent
- [ ] When `.y0()` is omitted the baseline defaults to `0.0`
- [ ] `.color(accessor | &str)` maps a categorical or continuous color to the
      fill
- [ ] `.opacity(f32)` sets the fill alpha (default `0.8`, stroke remains fully
      opaque)
- [ ] A single-series area chart renders without GPU validation errors
- [ ] The rendered polygon correctly closes between the data path, the baseline
      at the last point, and the baseline at the first point

### AC2: Stacked Area Chart

- [ ] `.stack()` enables cumulative stacking mode; requires a series key
      accessor set via `.color(accessor)` or `.series(accessor)`
- [ ] In stacking mode each series' `y0` is computed as the cumulative sum of
      all preceding series at the same `x` value
- [ ] Series rendering order matches the stacking order (bottom series drawn
      first, top series drawn last) so fills do not incorrectly occlude each
      other
- [ ] Stacked areas share x-axis domain and y-axis domain is derived from the
      maximum cumulative value

### AC3: Normalised Stacked Area (100%)

- [ ] `.stack_normalized()` enables percentage-stacking mode
- [ ] Each series is scaled so that the total at every x bin is exactly `1.0`
      (full chart height)
- [ ] Y-axis tick formatting automatically uses `%` notation when normalised
      mode is active

### AC4: Band / Ribbon Area

- [ ] When `.y0(accessor)` receives a per-record accessor (not a constant) the
      area fills between `y` and `y0` per point
- [ ] Both boundaries follow the data curve (no straight baseline assumption)
- [ ] Useful for confidence-interval ribbons; renders correctly even when `y0`
      crosses `y` for some points

### AC5: Gradient Fills

- [ ] `.gradient(color_scale)` accepts a `ColorScale` and applies a y-mapped
      gradient across the filled polygon
- [ ] Gradient colours are interpolated per vertex by the GPU; no CPU colour
      pre-computation per data point is required
- [ ] Gradient and flat-color modes can be selected independently per series in
      a stacked chart

### AC6: GPU Efficiency

- [ ] The closed polygon for each series is tessellated via the GUP-132 GPU Path
      tessellation pipeline — no CPU-side triangle fan generation
- [ ] For a 10 000-point single-series area chart the frame render time stays
      under 16 ms on the reference hardware used by CI
- [ ] Re-rendering after a data update (`.data(new_data)`) re-uploads only
      changed GPU buffers, not the full pipeline

### AC7: Integration and Ergonomics

- [ ] The builder integrates with the `gup::plot()` fluent entry point
      established in GUP-018:
      `gup::plot().data(ts).area(x("date"), y("value")).render()?`
- [ ] `AreaChartBuilder` implements the `ChartBuilder` trait defined in GUP-018
- [ ] At least one runnable example is added under `examples/` demonstrating a
      single-series, a stacked, and a band area chart

## Technical Tasks

- [ ] Create `src/chart_builders/area.rs` with `AreaChartBuilder<T>` struct
- [ ] Implement `.x()`, `.y()`, `.y0()`, `.color()`, `.opacity()` fluent methods
      returning `Self` for chaining
- [ ] Implement `.stack()` and `.stack_normalized()` builder methods; add a
      `StackMode` enum (`None`, `Stacked`, `Normalized`)
- [ ] Write a
      `compute_stack_offsets(data, x_accessor, y_accessor,     series_accessor, mode) -> Vec<StackedPoint>`
      helper that performs the cumulative-sum pre-pass on CPU before upload
- [ ] Implement
      `close_area_polygon(upper: &[Vec2], lower: &[Vec2]) -> PathCommands` to
      produce the closed winding for the GPU path tessellator
- [ ] Wire `close_area_polygon` output into the GUP-132 `PathMark` tessellation
      pipeline
- [ ] Add gradient vertex-colour support: map `ColorScale` → per-vertex RGBA
      during path construction, interpolated by the existing vertex shader
- [ ] Implement `ChartBuilder` trait for `AreaChartBuilder<T>`
- [ ] Register `AreaChartBuilder` on the `PlotBuilder` via `.area(x, y)` method
      (parallel to `.line(x, y)` from GUP-246)
- [ ] Infer x/y scale domains from data (reuse scale inference from GUP-246's
      `LineChartBuilder` if available)
- [ ] Add `%` formatter to the y-axis tick renderer when in normalised mode
- [ ] Add `examples/area_chart.rs` covering single-series, stacked, and band
      scenarios
- [ ] Write unit tests for `compute_stack_offsets` (edge cases: single series,
      zero values, negative values)
- [ ] Write unit tests for `close_area_polygon` (open path, degenerate single
      point, varying baseline)

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot-Style Chart Builders ✅ — defines `ChartBuilder`
  trait, `PlotBuilder` entry point, and the fluent API convention this story
  extends
- GUP-246: Line Chart Builder 📋 — area chart builds directly on the line
  rendering path; shares scale inference, x/y accessor machinery, and the
  `PlotBuilder::line` registration pattern
- GUP-132: GPU Path Tessellation ✅ — provides the compute-shader-based path
  tessellation pipeline used to render the closed area polygon on the GPU

### Enables Stories

- GUP-251: Custom Composite Chart Support — composite charts that layer area
  series with other mark types depend on `AreaChartBuilder` being available as a
  composable primitive

## Testing Strategy

- **Unit tests**: `compute_stack_offsets` correctness (single series, multiple
  series, normalised mode, zero-value bins); `close_area_polygon` output shape
  for straight, curved, and variable-baseline inputs
- **Integration tests**: render a 1 000-point single-series area chart
  headlessly via `wgpu` in `tests/` and assert no GPU validation errors; render
  a three-series stacked area chart and verify the topmost series' cumulative y
  equals the sum of all three series
- **Visual validation**: run `cargo run --example area_chart` and capture a
  screenshot; confirm filled regions are visible and correctly ordered for
  stacked variant
- **Performance**: informal frame-time check in the example with 10 000 points;
  document observed time in the retrospective

## Success Metrics

- [ ] All seven Acceptance Criteria are satisfied and checked
- [ ] `cargo run --example area_chart` produces a visually correct
      single-series, stacked, and band area chart without GPU errors
- [ ] `cargo test -- --test-threads=1` passes including new unit and integration
      tests
- [ ] 10 000-point single-series area chart renders in under 16 ms on CI
      reference hardware

## Risk Assessment

- **Medium**: Correctly closing the polygon when the baseline is per-point
  (band/ribbon mode) requires reversing the lower boundary path before
  concatenating — a subtle winding-order bug here will produce inverted or
  missing fills. _Mitigation_: Unit-test `close_area_polygon` with an explicit
  expected vertex sequence before wiring into the GPU pipeline.

- **Medium**: GUP-246 (Line Chart Builder) is a prerequisite but is itself
  `📋 Planned`. If GUP-246 is delayed, the shared scale-inference and
  `PlotBuilder` registration work must either be duplicated temporarily or this
  story must wait. _Mitigation_: Work on this story after GUP-246 is underway;
  extract shared scale infrastructure into a common `chart_builders/scale.rs`
  helper early in GUP-246 so it can be reused here with minimal coupling.

- **Low**: Gradient fills require per-vertex colour data in the path
  tessellation pipeline. If GUP-132's current vertex layout has no colour
  channel, a small shader modification will be needed. _Mitigation_: Inspect
  `PathMark` vertex struct during task implementation; adding a
  `color: [f32; 4]` field is a well-contained change.

- **Low**: Normalised stacking requires a two-pass over the data (first to
  compute per-bin totals, then to normalise). For very large datasets (>100 000
  points) this CPU pre-pass could be a bottleneck. _Mitigation_: Benchmark the
  pre-pass at 100 000 points; if slow, parallelize with `rayon` or move the
  normalisation into a compute shader pass in a future story.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
