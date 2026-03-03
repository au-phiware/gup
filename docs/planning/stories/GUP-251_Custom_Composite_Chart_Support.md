# GUP-251: Custom Composite Chart Support

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2026-03-01

## Context

Individual chart builders (scatter, bar, line, area, etc.) each produce a
complete, self-contained chart. That is the right default for the common case,
but real-world dashboards routinely need to overlay multiple visual layers on a
single set of axes: a scatter plot with a regression line drawn on top, a bar
chart paired with a trend line, or a scatter field with an isodensity overlay.
Today there is no first-class mechanism in Gup for combining two builder
instances into one coordinated chart — a developer who needs this must drop down
to the low-level `Mixable` API and wire up shared scales by hand.

GUP-001 established the `Mixable` trait as Gup's universal composition
primitive, and GUP-093 built the scale-axis integration system that
automatically detects data domains and coordinates axis rendering. What is
missing is a higher-level compositor that sits above both: one that understands
"these layers belong to the same chart" and therefore should share axes, unify
domains across all data, and render each layer in declaration order through a
single GPU pass sequence.

GUP-018 (Observable Plot Chart Builders) and the planned individual builder
stories (GUP-245 Bar Chart Builder, GUP-246 Line Chart Builder) provide the
layer types that the compositor will accept. This story delivers the glue that
lets a developer say `.layer(scatter_builder).layer(line_builder)` and receive a
properly coordinated multi-layer chart with no extra axis or scale configuration
required — while still allowing opt-in dual-axis layouts for the cases where
independent y-scales are intentional.

## User Story

> "As a visualization developer, I want a `CompositeChartBuilder` that accepts
> multiple chart layer builders via `.layer()` so that I can assemble
> multi-layer charts with automatically shared axes and unified data domains
> without manually managing scales or calling low-level `Mixable` APIs."

## Acceptance Criteria

### AC1: `CompositeChartBuilder` API

- [ ] A `CompositeChartBuilder` type is available at the top-level `gup` API
      surface (e.g., `gup::composite()` or `gup::plot().composite()`).
- [ ] `.layer(builder)` accepts any chart builder that implements a `ChartLayer`
      trait (or equivalent bound); calling it multiple times appends layers in
      declaration order.
- [ ] Layers are rendered in the order they were added (first layer at the
      bottom, last layer on top).
- [ ] The builder is consumed by `.render(context)` and returns
      `Result<(), GupError>`; the same call signature as individual chart
      builders.
- [ ] At least two concrete layer types are accepted: a scatter-style builder
      (from GUP-018) and a line-style builder (from GUP-246).

### AC2: Shared Axis and Unified Domain

- [ ] When all layers share the same axis (default behaviour), the x-domain is
      the union of all layers' x-data ranges and the y-domain is the union of
      all layers' y-data ranges.
- [ ] A single set of axes and grid lines is rendered for the composite chart —
      not one per layer.
- [ ] Adding a second layer does not cause axis ticks or labels to duplicate or
      overlap.
- [ ] Domain unification uses the same `Scale`/axis machinery from GUP-093 — no
      separate scale logic is introduced.

### AC3: Dual-Axis (Independent Y) Support

- [ ] `.layer_with_y2(builder)` (or equivalent opt-in API such as
      `.layer(builder).y_axis(Axis::Secondary)`) assigns a layer to an
      independent right-hand y-axis.
- [ ] When at least one layer uses the secondary y-axis, a second y-axis is
      rendered on the right side of the chart area with its own ticks, labels,
      and domain.
- [ ] Layers on the primary y-axis are unaffected by the secondary y-axis
      domain.
- [ ] At most one secondary y-axis is rendered regardless of how many layers are
      assigned to it (their y-domains are unified in the same way as
      primary-axis layers).

### AC4: Working Examples

- [ ] An example `composite_scatter_regression` is added under `examples/` that
      renders a scatter plot of sample data with a best-fit line overlaid, using
      `CompositeChartBuilder`.
- [ ] An example `composite_bar_trend` is added under `examples/` that renders a
      bar chart with a trend line on either the primary or secondary y-axis.
- [ ] Both examples compile with `cargo check --examples` and produce a visible,
      correct output when run.

### AC5: No GPU Validation Errors

- [ ] Both composite examples run to completion without wgpu validation errors
      or panics on all supported backends (Vulkan / Metal / DX12 / GL).
- [ ] Rendering a composite chart with two layers does not produce more draw
      calls than the sum of the individual layers' draw calls plus one axis draw
      call.

## Technical Tasks

- [ ] Define a `ChartLayer` trait (or augment an existing builder trait) with
      the interface `CompositeChartBuilder` requires: data-domain introspection
      and a `render_layer(context, scale_x, scale_y)` method.
- [ ] Implement `CompositeChartBuilder` struct with an ordered `Vec` of boxed
      `ChartLayer` trait objects.
- [ ] Implement `.layer(builder)` to push a layer onto the stack with primary
      y-axis assignment.
- [ ] Implement `.layer_with_y2(builder)` (or equivalent) to push a layer with
      secondary y-axis assignment.
- [ ] Implement domain-unification logic: iterate all primary-y layers, collect
      x and y extents, produce a unified `Scale` for each axis using GUP-093
      primitives.
- [ ] Implement secondary y-axis domain unification for y2 layers.
- [ ] Implement `.render(context)`: build scales, render axes once, then call
      each layer's `render_layer` in declaration order.
- [ ] Adapt the scatter builder (GUP-018) and line builder (GUP-246) to
      implement `ChartLayer`.
- [ ] Adapt the bar builder (GUP-245) to implement `ChartLayer`.
- [ ] Write the `composite_scatter_regression` example.
- [ ] Write the `composite_bar_trend` example.
- [ ] Write unit tests for domain-unification logic (pure data, no GPU
      required).
- [ ] Write integration tests that construct a `CompositeChartBuilder` with two
      layers and assert no render errors.

## Dependencies

### Prerequisite Stories

- GUP-001: Build Mixable Trait ✅ — provides the `Mixable` composition primitive
  that underpins layer composition.
- GUP-018: Observable Plot Chart Builders ✅ — provides the scatter layer type
  and the chart builder API conventions this story follows.
- GUP-093: Scale-Axis Integration System ✅ — provides the scale/axis machinery
  used for shared-axis and domain-unification logic.
- GUP-245: Bar Chart Builder 📋 — provides the bar chart layer adapted by this
  story.
- GUP-246: Line Chart Builder 📋 — provides the line chart layer adapted by this
  story.

### Enables Stories

- Future density/contour overlay stories — the `ChartLayer` trait established
  here is the extension point they will implement.

## Testing Strategy

- **Unit tests**: Test domain-unification logic in isolation with constructed
  `Scale` / extent values; verify that the union of two non-overlapping domains
  spans both, that empty layers are ignored, and that a zero-range domain does
  not panic.
- **Integration tests**: Construct a `CompositeChartBuilder` with a mock or
  headless GPU context, add two layers, call `.render()`, and assert no error is
  returned and no GPU validation messages are emitted.
- **Visual validation**: Run `composite_scatter_regression` and
  `composite_bar_trend` examples, screenshot the output, and confirm visually
  that (a) both layers are visible, (b) only one shared x-axis and primary
  y-axis appear, and (c) the secondary y-axis appears when enabled.
- **Performance**: Rendering a two-layer composite should not regress the
  per-frame time of any single-layer chart by more than 10 % when measured
  against the same data size.

## Success Metrics

- [ ] `gup::composite().layer(scatter).layer(line).render(ctx)` compiles and
      runs successfully with realistic sample data.
- [ ] Domain unification test passes for five distinct data-range combinations
      including overlapping, non-overlapping, and single-point ranges.
- [ ] Both composite examples produce output without wgpu validation errors on
      at least one GPU backend.
- [ ] `cargo test -- --test-threads=1` passes with no new failures.
- [ ] `cargo check --examples` passes for all examples including the two new
      composite examples.

## Risk Assessment

- **Medium**: The `ChartLayer` trait design must be flexible enough to
  accommodate future layer types (density, contour, candlestick) without
  breaking changes. _Mitigation_: Keep `ChartLayer` minimal at first — only
  domain introspection and a single `render_layer` method. Extend in follow-up
  stories.

- **Medium**: Domain unification for categorical x-axes (bar charts) combined
  with continuous x-axes (scatter/line) is semantically ill-defined.
  _Mitigation_: For this story, document that compositing layers with
  incompatible x-scale types is a user error. Add a runtime check that returns a
  clear `Err` rather than silently producing incorrect output.

- **Low**: Rendering order interacts with GPU blend state — a line drawn before
  a scatter may be occluded. _Mitigation_: Layers are rendered in declaration
  order (bottom-to-top), and default alpha blending from GUP-027 handles typical
  overlay cases. Document that callers should declare background layers first.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
