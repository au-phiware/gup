# GUP-250: Density Plot Builder

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-07-14

## Context

Scatter plots break down at scale: when thousands of overlapping points collapse
into an opaque mass, individual observations become invisible and spatial
patterns are lost. A density plot solves this by replacing the raw point cloud
with a continuous estimate of data concentration — showing _where_ data is dense
rather than _where each point is_. This is particularly important for Gup's
primary use case of large-dataset visualization, where overplotting is the rule
rather than the exception.

GUP-144 delivered 1D Kernel Density Estimation (KDE) — the statistical machinery
for computing smooth density curves — and explicitly noted it supports "1D and
2D KDE" in its acceptance criteria. This story extends that foundation to the 2D
case, evaluating a Gaussian kernel over a regular grid and producing a scalar
density field. Two complementary rendering modes are provided: a heatmap overlay
(building on GUP-248's `HeatmapChartBuilder`) and contour lines generated on the
GPU via a marching-squares compute shader (building on GUP-132's GPU
tessellation infrastructure).

GUP-018 established the Observable Plot-style chart builder pattern that all
high-level builders in this initiative follow: a fluent `Builder` type, data
bound at construction, accessors for X/Y channels, and a terminal `.build()`
call that returns a renderable `Chart`. The `DensityPlotBuilder` follows the
same conventions so it composes naturally with the rest of the builder ecosystem
— including overlay mode, where a density layer is stacked on top of a scatter
mark within a single chart.

## User Story

> "As a visualization developer working with large datasets, I want a
> `DensityPlotBuilder` that estimates and renders 2D data density as a heatmap
> or contour overlay so that I can reveal spatial concentration patterns that
> would be hidden by overplotting in a plain scatter plot."

## Acceptance Criteria

### AC1: DensityPlotBuilder Fluent API

- [ ] `gup::density_plot()` returns a `DensityPlotBuilder<()>` with no data
      bound
- [ ] `.data(vec)` binds a `Vec<T>` and returns `DensityPlotBuilder<T>`
- [ ] `.x(|d| d.field)` accepts a `Fn(&T) -> f32` accessor for the horizontal
      channel
- [ ] `.y(|d| d.field)` accepts a `Fn(&T) -> f32` accessor for the vertical
      channel
- [ ] `.bandwidth(f32)` sets a fixed KDE bandwidth; omitting it defaults to
      Silverman's rule (as implemented in GUP-144)
- [ ] `.levels(usize)` sets the number of contour iso-levels (default: 8)
- [ ] `.fill(bool)` toggles between filled-contour mode (`true`) and contour-
      line mode (`false`); default is `true`
- [ ] `.color_scheme(ColorScheme)` selects the sequential color palette used to
      encode density (default: `ColorScheme::Viridis`)
- [ ] `.build()` returns `Result<Chart, GupError>`
- [ ] All builder methods follow the owned-`self` pattern and are chainable

### AC2: 2D KDE Compute Shader

- [ ] A WGSL compute shader evaluates a 2D Gaussian kernel over a configurable
      grid (default: 256 × 256 cells)
- [ ] Input is a GPU storage buffer of `vec2<f32>` data points
- [ ] Output is a `texture_storage_2d<r32float, write>` density grid
- [ ] Bandwidth is passed as a uniform; Silverman's-rule estimate is computed on
      the CPU and uploaded before dispatch
- [ ] Grid bounds are derived automatically from the data extents with a
      configurable margin (default: 5% padding)
- [ ] Compute shader dispatch is parallelised over output grid cells (one thread
      per cell)
- [ ] Correctness: density values at test points match the CPU reference
      implementation from GUP-144 within 1 % relative error

### AC3: Contour Line Generation (Marching Squares)

- [ ] A WGSL compute shader implements the marching-squares algorithm to extract
      iso-contours from the density grid
- [ ] Iso-levels are distributed linearly between the grid minimum and maximum
      density values
- [ ] Output is a vertex buffer of line-segment endpoints suitable for
      submission to the path/line mark renderer
- [ ] GPU tessellation infrastructure from GUP-132 is reused for the final
      stroke rendering step
- [ ] Line contours render without GPU validation errors

### AC4: Filled Contour Mode

- [ ] When `.fill(true)`, each contour band (the region between two adjacent
      iso-levels) is rendered as a filled polygon
- [ ] Fill color is sampled from the chosen sequential color scheme proportional
      to the band's density level
- [ ] Filled regions tile seamlessly with no visible gaps or overlaps at band
      boundaries
- [ ] Alpha blending is supported so filled contours can be layered over other
      marks

### AC5: Heatmap Overlay Mode

- [ ] The density texture produced by the 2D KDE shader can be passed directly
      to the `HeatmapChartBuilder` rendering path from GUP-248
- [ ] `DensityPlotBuilder` exposes a `.as_heatmap_layer()` method that returns a
      composable layer value accepted by a parent chart builder
- [ ] Overlaying a density heatmap on a scatter plot does not require writing
      any manual wgpu pipeline code

### AC6: Overlay with Scatter Plot

- [ ] A combined scatter + density overlay can be built using the standard chart
      composition API established by GUP-018
- [ ] The density layer renders _beneath_ scatter points (correct z-ordering) by
      default
- [ ] An example `density_scatter_overlay` demonstrates the combined chart and
      compiles without warnings

## Technical Tasks

- [ ] Add `DensityPlotBuilder<T>` struct and builder methods in
      `src/chart_builders/density.rs` (new file)
- [ ] Implement Silverman's-rule bandwidth estimator for the 2D case:
      `h = σ · n^(-1/6)` (using per-axis σ and the joint sample count)
- [ ] Write `shaders/density_kde_2d.wgsl`: compute shader that reads a
      `array<vec2<f32>>` storage buffer and writes to a `texture_storage_2d`
- [ ] Write `shaders/density_marching_squares.wgsl`: compute shader that reads
      the density texture and emits line-segment vertices for each iso-level
- [ ] Implement filled-contour polygon generation — either extend the marching-
      squares shader to emit triangle fans or use a separate fill shader
- [ ] Connect the density texture output to the `HeatmapChartBuilder` pipeline
      (GUP-248) via a shared `DensityLayer` type
- [ ] Implement `.as_heatmap_layer()` on `DensityPlotBuilder`
- [ ] Implement `ColorScheme::Viridis` (and at least `Magma`, `Plasma`) as
      uniform color-lookup tables if not already provided by GUP-248
- [ ] Wire up the `DensityPlotBuilder` to produce a `Chart` via `.build()`
- [ ] Add `gup::density_plot()` re-export to `src/lib.rs`
- [ ] Write unit tests: KDE correctness vs CPU reference (GUP-144), marching-
      squares topology (closed contours, no dangling segments)
- [ ] Write integration test: `DensityPlotBuilder` builds without GPU errors on
      the headless test device
- [ ] Add example `examples/density_scatter_overlay.rs`
- [ ] Document public API with rustdoc; include at least one code example per
      major method

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot-Style Chart Builders ✅ — establishes the fluent
  builder pattern and `Chart` type that `DensityPlotBuilder` extends
- GUP-144: Kernel Density Estimation ✅ — provides the 1D KDE compute shader,
  Silverman's rule, and kernel function implementations that are extended to 2D
  here
- GUP-132: GPU Path Tessellation ✅ — provides the GPU tessellation
  infrastructure reused for stroke-rendering contour line segments
- GUP-248: Heatmap Chart Builder 📋 — provides the heatmap rendering pipeline
  and color-scheme uniforms that the density heatmap overlay mode plugs into

### Enables Stories

- Any story that builds violin plots or bivariate distribution summaries will
  find the 2D KDE shader a natural building block.

## Testing Strategy

- **Unit tests**: Verify the 2D KDE output against the CPU implementation
  shipped with GUP-144 using three synthetic distributions (standard bivariate
  normal, uniform rectangle, mixture of two Gaussians). Maximum relative error
  must be ≤ 1 %.
- **Unit tests**: Verify marching-squares output for a hand-crafted 4 × 4
  density grid with known contour topology — confirm correct segment count and
  connectivity.
- **Integration tests**: Run the full `DensityPlotBuilder` pipeline on the
  headless wgpu device; assert no GPU validation errors and that the output
  texture is non-trivially non-zero.
- **Visual validation**: Run `examples/density_scatter_overlay` and inspect the
  screenshot — density rings should be centred on visible data clusters; no
  rendering artefacts.
- **Performance**: For 100 K data points on a 256 × 256 KDE grid, total GPU
  compute time (KDE + contour extraction) should be < 100 ms on the CI device as
  measured by the GPU timestamp query system from GUP-161.

## Success Metrics

- [ ] `DensityPlotBuilder` API compiles and the example renders correctly on
      native wgpu
- [ ] 2D KDE values match CPU reference within 1 % for all three test
      distributions
- [ ] Contour lines are topologically valid (no dangling segments, no crossing
      lines at the same iso-level) for a 256 × 256 test grid
- [ ] KDE + contour compute pass completes in < 100 ms for 100 K points
- [ ] Filled-contour and heatmap overlay modes both render without GPU
      validation errors
- [ ] `cargo test -- --test-threads=1` passes with no new failures

## Risk Assessment

- **Medium**: Marching-squares ambiguous cases (saddle points) can produce
  topologically inconsistent contours if not handled. _Mitigation_: Use the
  standard interpolation-based disambiguation rule (compare centre value to
  iso-level); cover both ambiguous cases in unit tests.

- **Medium**: GUP-248 (Heatmap Chart Builder) is itself unplanned at time of
  writing; the `.as_heatmap_layer()` integration path cannot be implemented
  until GUP-248 is at least in progress. _Mitigation_: Implement the KDE and
  marching-squares shaders independently of the heatmap path; stub out
  `.as_heatmap_layer()` behind a compile-time feature flag or return a `todo!()`
  until GUP-248 lands.

- **Low**: `texture_storage_2d` binding requires
  `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` on some WGSL targets; may need a
  fallback to a storage buffer for web builds. _Mitigation_: Wrap grid output
  behind an abstraction that can be swapped between texture and buffer
  back-ends; test on the wasm32 CI job.

- **Low**: Sequential color-scheme uniform buffers may duplicate work done in
  GUP-248. _Mitigation_: Define color-scheme types in a shared `src/color.rs`
  module rather than inside either builder; both builders import from there.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
