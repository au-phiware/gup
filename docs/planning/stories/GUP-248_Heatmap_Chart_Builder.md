# GUP-248: Heatmap Chart Builder

## Story Overview

**Initiative**: Chart Builders  
**Status**: 🚧 In Progress  
**Created**: 2026-03-02

## Context

A heatmap is a foundational visualization type that encodes a scalar value as a
color in a 2D grid. It is indispensable for correlation matrices, time-of-day
activity patterns, genomics expression data, and geographic density maps. Gup
already has strong foundations: rectangle marks (GUP-067) provide the per-cell
rendering primitive, the scale-axis integration system (GUP-093) handles
automatic axis generation, and the Observable Plot-style chart builder API
(GUP-018) establishes the fluent builder pattern that `HeatmapChartBuilder` must
follow.

The primary challenge unique to heatmaps is the combination of 2D binning and
color-scale mapping. Input data arrives as a flat record set (e.g.,
`{x, y, value}` rows) and must be partitioned into a regular grid of cells, with
an aggregate function (sum, mean, count) applied per cell. The resulting cell
values are then mapped to colors via a continuous color scale — work that is
specified in GUP-255 (`ColorScale` GPU shader function). Because even modest
grids can reach 1 000 × 1 000 = 1 M cells, GPU instanced rendering is
non-negotiable for smooth 60 FPS performance.

This story also supports pre-binned data (where the caller has already computed
per-cell aggregates) so that users working with server-side aggregation
pipelines or static matrices can feed data directly without re-aggregating on
the client. A colorbar axis — a thin color-scaled legend rendered alongside the
main chart — is required so that viewers can read off the encoded values.

## User Story

> "As a data analyst, I want to call
> `.heatmap(x("hour"), y("weekday"), fill("count"))` on a flat dataset so that I
> can produce a GPU-rendered time-of-day activity grid with correct color
> scaling and a readable colorbar legend — without writing any GPU code."

> "As a performance-sensitive developer, I want the heatmap renderer to sustain
> 60 FPS at 1 M cells so that I can embed large heatmaps in real-time
> dashboards."

## Acceptance Criteria

### AC1: Fluent Builder API

- [ ] `HeatmapChartBuilder` struct is accessible via `gup::plot().heatmap(...)`
      following the existing chart builder conventions from GUP-018.
- [ ] Builder exposes `.x(accessor)`, `.y(accessor)`, `.fill(accessor)` methods
      for column/accessor binding.
- [ ] `.x_bins(n: usize)` and `.y_bins(n: usize)` configure grid resolution;
      sensible defaults are applied when omitted (e.g., Sturges' rule or a fixed
      default of 20).
- [ ] `.aggregate(AggregateFunc)` selects the per-cell reduction: `Count`,
      `Sum`, `Mean`, `Min`, `Max`; defaults to `Count`.
- [ ] `.color_scale(ColorScaleSpec)` accepts a named palette or a custom color
      ramp; defaults to a built-in sequential scale (e.g., Viridis).
- [ ] `.render()` returns `Result<(), GupError>` consistent with other chart
      builders.

### AC2: 2D Binning of Raw Data

- [ ] Given a flat `Vec<T>` of records, the builder correctly partitions data
      into an `x_bins × y_bins` grid using the selected x/y accessors.
- [ ] Bin boundaries are computed from the data's observed min/max (or from
      explicit `.x_domain()`/`.y_domain()` overrides) using linear binning.
- [ ] The aggregate function is applied correctly per cell; empty cells default
      to a configurable "no-data" value (default: `f32::NAN`) and are rendered
      transparently or with a distinct color.
- [ ] Round-trip test: aggregate `Sum` over 10 000 uniformly distributed points
      into a 10×10 grid and verify total equals input sum within floating-point
      tolerance.

### AC3: Pre-Binned Data Support

- [ ] `HeatmapChartBuilder::from_grid(data: Vec<HeatmapCell>)` accepts records
      that already carry `{x_index, y_index, value}` fields, bypassing the
      binning step.
- [ ] Pre-binned and raw-data paths produce visually identical output for the
      same dataset.
- [ ] API is documented with a `//! # Examples` doc-test showing pre-binned
      usage.

### AC4: GPU Instanced Rendering

- [ ] Cells are rendered via a single instanced draw call using the Rectangle
      mark infrastructure from GUP-067; one GPU instance per cell.
- [ ] A 1 000 × 1 000 (1 M cell) heatmap renders at ≥ 60 FPS on a mid-range
      discrete GPU (validated in the example below; frame time logged).
- [ ] Cell colors are computed in the GPU vertex/fragment shader using the
      `ColorScale` shader function from GUP-255; no CPU-side color expansion.
- [ ] GPU validation layer reports zero errors during rendering.

### AC5: ColorScale Integration

- [ ] The `ColorScale` from GUP-255 is wired into the heatmap fragment shader so
      that value→color mapping happens entirely on the GPU.
- [ ] Domain clamping: values outside `[min, max]` are rendered at the boundary
      color, not wrapped.
- [ ] The color scale's domain is automatically derived from the full range of
      cell values after aggregation, with an optional `.fill_domain(min, max)`
      override.

### AC6: Colorbar Axis (Color Legend)

- [ ] A colorbar (a thin gradient-filled rectangle with tick marks and numeric
      labels) is rendered adjacent to the plot by default; suppressible with
      `.colorbar(false)`.
- [ ] The colorbar uses the same `ColorScale` as the cells and inherits the same
      domain.
- [ ] Tick count and formatting follow the conventions of the existing axis
      system (GUP-093).

### AC7: Example and Documentation

- [ ] A runnable example at `examples/heatmap_chart.rs` demonstrates: - Raw-data
      heatmap from a synthetic dataset (time-of-week activity pattern). -
      Pre-binned heatmap from a 100×100 matrix. - A large 1 000×1 000 heatmap
      with frame-time output to stdout.
- [ ] Public API is documented with `///` doc comments; `cargo doc` produces no
      warnings for this module.

## Technical Tasks

- [ ] Create `src/chart_builders/heatmap.rs` with `HeatmapChartBuilder` struct
      and fluent builder methods (`.x`, `.y`, `.fill`, `.x_bins`, `.y_bins`,
      `.aggregate`, `.color_scale`, `.colorbar`, `.x_domain`, `.y_domain`,
      `.fill_domain`, `.render`).
- [ ] Implement `AggregateFunc` enum (`Count`, `Sum`, `Mean`, `Min`, `Max`) and
      the CPU-side 2D binning loop in `src/chart_builders/heatmap/binning.rs`.
- [ ] Define `HeatmapCell { x_index: u32, y_index: u32, value: f32 }` and
      `from_grid()` constructor.
- [ ] Create `HeatmapInstanceData` GPU struct (x_index, y_index, value as f32)
      and upload as an instance buffer via the Rectangle mark pipeline.
- [ ] Write `src/shaders/heatmap.wgsl`: vertex shader positions each instance as
      a screen-space rectangle; fragment shader calls `color_scale(value)` from
      GUP-255 and discards NaN cells.
- [ ] Integrate the GUP-255 `ColorScale` bind group / uniform buffer into the
      heatmap render pipeline.
- [ ] Implement colorbar rendering as a separate thin instanced gradient strip
      with axis tick overlay using GUP-093 primitives.
- [ ] Wire `HeatmapChartBuilder` into the top-level `PlotBuilder` so
      `gup::plot().heatmap(...)` compiles.
- [ ] Add unit tests in `src/chart_builders/heatmap/binning.rs` covering: empty
      input, single-cell grid, uniform distribution sum, NaN cell handling.
- [ ] Add integration test in `tests/heatmap_integration.rs` comparing raw-data
      and pre-binned paths for identical output.
- [ ] Write `examples/heatmap_chart.rs` with the three demonstration scenarios
      described in AC7.
- [ ] Update `cargo doc` entry point and re-export `HeatmapChartBuilder` from
      `gup::chart_builders`.

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot Chart Builders ✅ — establishes the `PlotBuilder`
  fluent API pattern and the extension points that `HeatmapChartBuilder` must
  integrate with.
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides the GPU
  rectangle instanced-draw pipeline that renders each heatmap cell.
- GUP-093: Scale-Axis Integration System ✅ — provides automatic scale
  detection, axis rendering, and tick/label formatting used for the colorbar and
  both grid axes.
- GUP-255: ColorScale GPU Shader Function 📋 — provides the `color_scale()` WGSL
  function and associated bind group that map scalar cell values to RGBA on the
  GPU; the heatmap fragment shader cannot be completed without it.

### Enables Stories

- A future "Density / Contour Chart Builder" story would naturally reuse the 2D
  binning infrastructure introduced here.

## Testing Strategy

- **Unit tests**: Binning logic tested in isolation with deterministic synthetic
  data — verify bucket boundaries, aggregate correctness (sum, mean, count, min,
  max), empty-cell handling, and domain clamping.
- **Integration tests**: `tests/heatmap_integration.rs` renders a small (10×10)
  heatmap headlessly and asserts that the GPU pipeline completes without
  validation errors and that the instance buffer contains the expected cell
  count.
- **Visual validation**: Run `cargo run --example heatmap_chart` and inspect the
  rendered output; the colorbar gradient must align with cell colors.
- **Performance**: The 1 000×1 000 example prints per-frame GPU time to stdout;
  the gate is ≥ 60 FPS (≤ 16.7 ms/frame) on a mid-range discrete GPU. This is
  informational during review, not a hard CI gate (GPU hardware varies in CI).

## Success Metrics

- [ ] `cargo test -- --test-threads=1` passes with all new heatmap tests green.
- [ ] `examples/heatmap_chart` runs end-to-end without panics or GPU validation
      errors.
- [ ] 1 M-cell heatmap achieves ≤ 16.7 ms average frame time on a mid-range
      discrete GPU.
- [ ] `cargo doc` produces no new warnings for the `heatmap` module.
- [ ] API surface is consistent with the existing chart builder conventions
      established in GUP-018.

## Risk Assessment

- **Medium**: GUP-255 (`ColorScale` GPU Shader Function) is not yet complete.
  The heatmap fragment shader depends on the `color_scale()` WGSL function
  signature. _Mitigation_: Implement a local stub `color_scale()` (e.g., a
  greyscale ramp) during development so all other work can proceed; replace with
  the GUP-255 implementation once available. Track the stub with a
  `// TODO(GUP-255)` comment.

- **Medium**: CPU-side 2D binning for very large flat datasets (10 M+ rows) may
  be slow on the main thread.  
  _Mitigation_: Implement the binning loop with Rayon parallel iterators behind
  a feature flag; document the trade-off. If this proves insufficient, a
  follow-up story can move binning to a compute shader.

- **Low**: Instanced rectangle rendering for 1 M cells may exceed GPU vertex
  buffer limits on some WebGPU/WASM targets.  
  _Mitigation_: Chunk draw calls when `x_bins * y_bins` exceeds
  `wgpu::Limits::max_vertex_buffer_array_stride`-derived limits; add an
  assertion with a clear error message.

- **Low**: NaN handling in the fragment shader (discarding transparent cells)
  must be tested explicitly, as NaN comparisons in WGSL behave differently from
  IEEE 754 expectations in some drivers.  
  _Mitigation_: Use `!= value` NaN-detection idiom in WGSL and cover with a
  headless GPU validation test.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
