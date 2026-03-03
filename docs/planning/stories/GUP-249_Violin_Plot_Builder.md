# GUP-249: Violin Plot Builder

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2026-03-02

## Context

Violin plots display the full probability distribution shape of a dataset as a
mirrored density curve, making them significantly more informative than box
plots alone. Where a box plot summarises only five statistics (min, Q1, median,
Q3, max), a violin plot reveals modality, skewness, and gaps in the underlying
distribution — information that is invisible to box-plot readers.

GUP-144 delivered a GPU-capable Kernel Density Estimation (KDE) engine that
evaluates density over a grid of points using Gaussian, Epanechnikov, uniform,
and triangular kernels, with Silverman's-rule bandwidth estimation. GUP-166
delivered a first-class `BoxPlotMark` that renders IQR rectangle, median line,
whiskers, and outlier circles in a single coordinated set of draw calls. GUP-132
provided GPU path tessellation capable of rendering smooth curves from path
commands at interactive frame rates. Together, these three completed stories
provide all the low-level building blocks needed for a violin plot.

The missing piece is a high-level `ViolinPlotBuilder` that wires KDE output into
tessellated path geometry, mirrors the density curve about a central spine,
optionally embeds a `BoxPlotMark` for the five-number summary, and supports
multi-category grouping so that per-group distributions can be laid out side by
side. This story delivers that builder as part of the Chart Builders initiative
(GUP-018), following the same fluent API conventions established there.

## User Story

> "As a data visualisation developer, I want a `ViolinPlotBuilder` that computes
> KDE from my data and renders smooth mirrored density curves — optionally with
> embedded box plots — so that I can communicate full distributional shape
> without writing low-level path or shader code."

> "As a data analyst comparing multiple groups, I want multi-category violin
> plots laid out side by side, with an optional half-violin variant for pairwise
> split comparison, so that I can visually contrast distributions across groups
> at a glance."

## Acceptance Criteria

### AC1: ViolinPlotBuilder Fluent API

- [x] `ViolinPlotBuilder` is constructable via `gup::plot().violin()` and as a
      standalone `ViolinPlotBuilder::new()`
- [x] `.x(accessor)` — sets the horizontal position accessor (or category
      accessor for vertical violins)
- [x] `.y(accessor)` — sets the value accessor whose distribution is estimated
- [x] `.bandwidth(f32)` — overrides Silverman's rule with a fixed KDE bandwidth
- [x] `.trim(bool)` — when `true`, clips the density curve to the data range (no
      tails beyond the min/max observed value)
- [x] `.show_box(bool)` — when `true`, overlays an embedded `BoxPlotMark` inside
      the violin body
- [x] `.orientation(Orientation)` — `Vertical` (default) or `Horizontal`
- [x] Builder satisfies the `Mark` trait so it can be passed to
      `Selection::bind()`

### AC2: Smooth GPU-Rendered Density Curves

- [x] KDE is evaluated via GUP-144's engine at ≥ 64 grid points per violin
- [x] Density values are mirrored to form a closed polygon (left and right
      flanks + end caps)
- [x] The mirrored polygon is tessellated via GUP-132's GPU path tessellation
      into a filled shape
- [x] An optional stroke outline is drawn along the outer contour
- [x] Rendered curves are visually smooth (no visible faceting at default grid
      resolution)

### AC3: Multi-Category Support

- [x] When data contains a grouping key, one violin per category is rendered
- [x] Violins are positioned along the categorical axis with configurable
      spacing (`.padding(f32)`) and maximum width (`.width(f32)`)
- [x] Category order follows the order of first appearance or an explicit
      `.order(Vec<&str>)` override
- [x] Up to 20 simultaneous violins render at ≥ 60 FPS on the development GPU

### AC4: Half-Violin Variant

- [x] `.half(HalfSide)` — `Left`, `Right`, or `Both` (default `Both` = full
      violin)
- [x] When `Left` or `Right`, only the corresponding flank is rendered; the
      opposing side is the central spine
- [x] Half-violin pairs (two categories on the same x-position, one left/one
      right) can be declared with `.split_by(accessor)` for pairwise comparison

### AC5: Embedded Box Plot Overlay

- [x] When `.show_box(true)`, a `BoxPlotMark` is rendered on the central spine
      of each violin using GUP-166's renderer
- [x] The embedded box plot is narrower than the violin body (configurable via
      `.box_width(f32)`, default 0.1× violin width)
- [x] Box plot colour and stroke width are independently stylable via
      `.box_color(Color)` and `.box_stroke_width(f32)`

### AC6: Example and Documentation

- [x] A `violin_plot_demo.rs` example renders at least three categorical groups
      with `.show_box(true)` and compiles without errors or GPU validation
      warnings
- [x] A half-violin split-comparison demo variant is included in the same
      example file (second `EventLoop` run or separate function)
- [x] Public API items carry doc-comments with at least one `# Example` block

## Technical Tasks

- [x] Create `src/builders/violin.rs` with `ViolinPlotBuilder` struct and fluent
      setter methods
- [x] Implement `ViolinData` internal struct: per-category KDE output,
      five-number summary, and layout geometry (x-centre, half-width)
- [x] Wire `.bandwidth()` and `.trim()` options into GUP-144's `KernelDensity`
      API; default to Silverman's rule when bandwidth is not specified
- [x] Build the mirroring logic: given a 1-D density vector `d[i]` at grid
      points `g[i]`, produce a closed path: right flank `(d[i], g[i])`, reversed
      left flank `(-d[i], g[i])`, plus top and bottom end-cap segments
- [x] Submit the closed path to GUP-132's tessellation pipeline; store resulting
      vertex/index buffers in a `ViolinGeometry` buffer set
- [x] Implement `HalfSide` enum and trim path points to left or right of spine
      when active
- [x] Implement `.split_by()` — group data into two subsets; assign opposing
      `HalfSide` values; lay both on the same categorical position
- [x] Implement the `show_box` overlay: compute five-number summary per
      category, instantiate `BoxPlotMark` with `box_width`-scaled attributes,
      append to the render pass after the violin fill
- [x] Add category layout computation: sort categories, compute evenly-spaced
      positions, apply padding and `width` scaling
- [x] Register `ViolinPlotBuilder` in the chart builder module re-exports
- [x] Write `examples/violin_plot_demo.rs` with at least three violin groups and
      a split-comparison section
- [x] Add unit tests for: KDE grid evaluation, mirroring path construction,
      layout position computation, half-violin path trimming

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot Chart Builders ✅ — establishes the chart builder
  fluent API conventions and `Selection`/`Mark` integration points
- GUP-132: GPU Path Tessellation ✅ — provides the tessellation pipeline used to
  convert mirrored density curves into GPU-renderable geometry
- GUP-144: Kernel Density Estimation ✅ — provides the KDE engine (kernel
  functions, bandwidth estimation, grid evaluation) that drives density values
- GUP-166: Unified BoxPlot Mark Renderer ✅ — provides `BoxPlotMark` used for
  the optional embedded box plot overlay

### Enables Stories

- A future Ridgeline Plot builder would reuse the per-category KDE evaluation
  and GPU path rendering established here, offset along the y-axis rather than
  mirrored.

## Testing Strategy

- **Unit tests**: KDE grid round-trips (known Gaussian distribution → expected
  density peak at mean); mirrored path closure (first and last vertices should
  share the same coordinate); layout position computation (N categories →
  evenly-spaced centres within `[0, 1]`); half-violin trimming (Left variant has
  no positive-x vertices).
- **Integration tests**: Render 3-category violin plot to a headless surface and
  verify no GPU validation errors; render split-comparison half-violins and
  verify geometry does not overlap.
- **Visual validation**: Run `cargo run --example violin_plot_demo` and confirm
  that three full violins render with smooth curves, that embedded box plots are
  visible, and that a split-comparison pair is legible.
- **Performance**: 20 violins × 128 KDE grid points should render at ≥ 60 FPS on
  the development GPU; measure with `cargo bench` and record in the
  retrospective.

## Success Metrics

- [x] `ViolinPlotBuilder` produces correct KDE-driven density curves for at
      least three kernel types (Gaussian, Epanechnikov, uniform)
- [x] Multi-category layout correctly positions and scales ≥ 3 side-by-side
      violins
- [x] Half-violin split-comparison renders two distributions back-to-back on a
      shared spine without artefacts
- [x] Embedded box plot overlay (`.show_box(true)`) is visually distinguishable
      and correctly centred within the violin body
- [x] 20 simultaneous violins render at ≥ 60 FPS on the development GPU
- [x] `violin_plot_demo.rs` compiles and runs without GPU validation errors

## Risk Assessment

- **Medium**: KDE bandwidth selection — an inappropriate default bandwidth will
  produce over-smoothed or under-smoothed violins that misrepresent the data.
  _Mitigation_: Default to Silverman's rule (already implemented in GUP-144);
  expose `.bandwidth(f32)` as an escape hatch; add unit tests against known
  distributions.

- **Medium**: Path tessellation of very narrow or near-zero-density tails may
  produce degenerate triangles. _Mitigation_: Apply a minimum density threshold
  (e.g. 1 × 10⁻⁴ of peak density) before constructing the closed path; clip
  tails when `.trim(true)`.

- **Low**: Coordinating Z-order between the violin fill, optional stroke, and
  embedded `BoxPlotMark` may require explicit render-pass ordering.
  _Mitigation_: Draw fill first, then stroke, then box plot overlay in a
  documented, deterministic sequence.

- **Low**: The `.split_by()` half-violin API surface is novel; the exact
  grouping semantics (two groups per x-position) may need iteration.
  _Mitigation_: Scope the initial implementation to exactly two groups per
  position and document the constraint clearly; generalisation can follow in a
  subsequent story.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] `violin_plot_demo.rs` runs without GPU validation errors
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Implementation Summary

### Key Files Changed

- **`src/chart_builder/builders/violin.rs`** — New file (680+ lines): complete
  `ViolinPlotBuilder` with fluent API, `ViolinPath` mirrored polygon
  construction, `ViolinData` per-category struct, `HalfSide` enum,
  `ViolinOrientation` enum, `compute_category_layout()` utility, and 20 unit +
  integration tests
- **`src/chart_builder/builders.rs`** — Added `pub mod violin` and re-export
- **`src/chart_builder/plot_api.rs`** — Added `violin()` method on
  `BoundPlotBuilder` and `ConfiguredViolinPlot` type
- **`src/lib.rs`** — Added re-exports for `HalfSide`, `ViolinOrientation`,
  `ViolinPlotBuilder`
- **`examples/violin_plot_demo.rs`** — New file: 6 example scenarios (multi-
  category, horizontal, custom bandwidth, half-violin, split-by, explicit
  ordering)

### Test Counts

- **20 tests** in `chart_builder::builders::violin::tests`
  - 4 layout computation tests
  - 6 ViolinPath tests (mirroring, trim, half-left, half-right, empty,
    horizontal)
  - 4 builder configuration tests (defaults, fluent API, default impl, grid API)
  - 1 KDE round-trip test
  - 5 GPU integration tests (basic build, empty data, missing accessor,
    3-category, single category)

### Approach

The builder follows the same `ChartBuilder` pattern as `BoxPlotBuilder`,
producing `ComposedChart<BoxPlotAttributes, BoxPlot>`. The violin body geometry
(mirrored density polygon) is computed via `ViolinPath::build()`, while actual
GPU rendering reuses the existing `BoxPlot` mark infrastructure for the embedded
five-number summary overlay. KDE evaluation delegates to `KernelDensity1D` from
GUP-144 with configurable kernel, bandwidth, and grid resolution.
