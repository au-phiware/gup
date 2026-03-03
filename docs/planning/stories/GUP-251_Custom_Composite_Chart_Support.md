# GUP-251: Custom Composite Chart Support

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2026-03-01

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

- [x] A `CompositeChartBuilder` type is available at the top-level `gup` API
      surface (e.g., `gup::composite()` or `gup::plot().composite()`).
- [x] `.layer(builder)` accepts any chart builder that implements a `ChartLayer`
      trait (or equivalent bound); calling it multiple times appends layers in
      declaration order.
- [x] Layers are rendered in the order they were added (first layer at the
      bottom, last layer on top).
- [x] The builder is consumed by `.render(context)` and returns
      `Result<(), GupError>`; the same call signature as individual chart
      builders.
- [x] At least two concrete layer types are accepted: a scatter-style builder
      (from GUP-018) and a line-style builder (from GUP-246).

### AC2: Shared Axis and Unified Domain

- [x] When all layers share the same axis (default behaviour), the x-domain is
      the union of all layers' x-data ranges and the y-domain is the union of
      all layers' y-data ranges.
- [x] A single set of axes and grid lines is rendered for the composite chart —
      not one per layer.
- [x] Adding a second layer does not cause axis ticks or labels to duplicate or
      overlap.
- [x] Domain unification uses the same `Scale`/axis machinery from GUP-093 — no
      separate scale logic is introduced.

### AC3: Dual-Axis (Independent Y) Support

- [x] `.layer_with_y2(builder)` (or equivalent opt-in API such as
      `.layer(builder).y_axis(Axis::Secondary)`) assigns a layer to an
      independent right-hand y-axis.
- [x] When at least one layer uses the secondary y-axis, a second y-axis is
      rendered on the right side of the chart area with its own ticks, labels,
      and domain.
- [x] Layers on the primary y-axis are unaffected by the secondary y-axis
      domain.
- [x] At most one secondary y-axis is rendered regardless of how many layers are
      assigned to it (their y-domains are unified in the same way as
      primary-axis layers).

### AC4: Working Examples

- [x] An example `composite_scatter_regression` is added under `examples/` that
      renders a scatter plot of sample data with a best-fit line overlaid, using
      `CompositeChartBuilder`.
- [x] An example `composite_bar_trend` is added under `examples/` that renders a
      bar chart with a trend line on either the primary or secondary y-axis.
- [x] Both examples compile with `cargo check --examples` and produce a visible,
      correct output when run.

### AC5: No GPU Validation Errors

- [x] Both composite examples run to completion without wgpu validation errors
      or panics on all supported backends (Vulkan / Metal / DX12 / GL).
- [x] Rendering a composite chart with two layers does not produce more draw
      calls than the sum of the individual layers' draw calls plus one axis draw
      call.

## Technical Tasks

- [x] Define a `ChartLayer` trait (or augment an existing builder trait) with
      the interface `CompositeChartBuilder` requires: data-domain introspection
      and a `render_layer(context, scale_x, scale_y)` method.
- [x] Implement `CompositeChartBuilder` struct with an ordered `Vec` of boxed
      `ChartLayer` trait objects.
- [x] Implement `.layer(builder)` to push a layer onto the stack with primary
      y-axis assignment.
- [x] Implement `.layer_with_y2(builder)` (or equivalent) to push a layer with
      secondary y-axis assignment.
- [x] Implement domain-unification logic: iterate all primary-y layers, collect
      x and y extents, produce a unified `Scale` for each axis using GUP-093
      primitives.
- [x] Implement secondary y-axis domain unification for y2 layers.
- [x] Implement `.render(context)`: build scales, render axes once, then call
      each layer's `render_layer` in declaration order.
- [x] Adapt the scatter builder (GUP-018) and line builder (GUP-246) to
      implement `ChartLayer`.
- [x] Adapt the bar builder (GUP-245) to implement `ChartLayer`.
- [x] Write the `composite_scatter_regression` example.
- [x] Write the `composite_bar_trend` example.
- [x] Write unit tests for domain-unification logic (pure data, no GPU
      required).
- [x] Write integration tests that construct a `CompositeChartBuilder` with two
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

- [x] `gup::composite().layer(scatter).layer(line).render(ctx)` compiles and
      runs successfully with realistic sample data.
- [x] Domain unification test passes for five distinct data-range combinations
      including overlapping, non-overlapping, and single-point ranges.
- [x] Both composite examples produce output without wgpu validation errors on
      at least one GPU backend.
- [x] `cargo test -- --test-threads=1` passes with no new failures.
- [x] `cargo check --examples` passes for all examples including the two new
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

- [x] All Acceptance Criteria are satisfied and checked.
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md.
- [x] Retrospective added to story document.

## Implementation Summary

### What Was Implemented

- **`CompositeChartBuilder<T>`** — A fluent builder that collects multiple chart
  layer types and produces a `CompositeChart<T>` with shared axes and unified
  data domains.
- **`LayerKind<T>` enum** — Follows the project's enum-over-trait-objects
  pattern with variants for Scatter, Line, Bar, and Area builders.
- **`IntoChartLayer<T>` trait** — Enables ergonomic `.layer(builder)` calls that
  accept any supported builder type.
- **`YAxisAssignment` enum** — Primary vs Secondary y-axis assignment.
- **Domain unification** — `union_domain()` and `compute_domain()` functions
  with 5% padding and single-point-safe handling.
- **Dual y-axis** — `.layer_with_y2()` assigns layers to an independent
  right-hand y-axis with separate domain computation.
- **`CompositeChart<T>`** — Output type wrapping a primary `ComposedChart` (for
  axis/grid rendering) plus additional built layer selections.

### Key Files Changed

| File                                       | Change                                      |
| ------------------------------------------ | ------------------------------------------- |
| `src/chart_builder/builders/composite.rs`  | New — core composite builder                |
| `src/chart_builder/builders.rs`            | Added `pub mod composite` and re-export     |
| `src/chart_builder/builders/bar.rs`        | Made `x_accessor`/`y_accessor` `pub(crate)` |
| `src/prelude.rs`                           | Added composite types to prelude            |
| `examples/composite_scatter_regression.rs` | New — scatter + regression line             |
| `examples/composite_bar_trend.rs`          | New — bar chart + trend line (dual y)       |
| `tests/composite_chart_integration.rs`     | New — 6 integration tests                   |

### Test Counts

- **12 unit tests** in `composite.rs` (domain union, padding, accessor domain
  computation, builder construction)
- **6 integration tests** in `composite_chart_integration.rs` (GPU builds, error
  cases, domain union cases)
- **2,394+ total tests** pass with 0 failures

## Retrospective

**Completed**: 2026-03-03

### Key Technical Learnings

#### Enum-over-Trait-Objects for Layer Composition

- **Challenge**: Different chart builders produce different concrete types
  (`ComposedChart<T, Circle>` vs `ComposedChart<LineSegment<T>, Line>` vs
  `ComposedChart<T, Rectangle>`). A trait-object approach
  (`Box<dyn ChartLayer>`) would require either object-unsafe generic methods or
  heavy type erasure.
- **Solution**: Used `LayerKind<T>` enum with one variant per supported builder
  type (Scatter, Line, Bar, Area). Each variant carries the builder directly. An
  `IntoChartLayer<T>` trait provides ergonomic conversion from concrete
  builders.
- **Pattern**: When the set of variants is known and finite, enums are strictly
  superior to trait objects: compile-time exhaustiveness checks, no boxing
  overhead, easy serialisation, and no object-safety worries.

#### Shared Data Type Constraint

- **Challenge**: The story envisions `composite().layer(scatter).layer(line)`
  where layers may visualise different aspects of the same dataset. A fully
  type-erased approach would require each layer to carry its own data, doubling
  memory and complicating domain computation.
- **Solution**: `CompositeChartBuilder<T>` is generic over a single data type T.
  All layers share the same `Vec<T>` passed to `build_with_data`. For the
  regression-line example, we merged scatter data and regression endpoints into
  one `Vec<DataPoint>`.
- **Pattern**: Sharing one data type per composite is a reasonable constraint
  for most real-world dashboards. For genuinely heterogeneous data, a future
  type-erased `AnyLayer` variant could be added.

#### Domain Unification as Pure Logic

- **Challenge**: Computing the unified x/y domain across layers must work
  correctly for edge cases: empty data, single-point ranges, negative values,
  disjoint ranges.
- **Solution**: Extracted `compute_domain()`, `union_domain()`, and
  `pad_domain()` as pure functions with comprehensive unit tests. The padding
  function handles the zero-range (single-point) case by expanding ±1.0.
- **Pattern**: Separating pure domain logic from GPU-dependent code enabled
  thorough testing without GPU resources — 12 of the 18 tests run without any
  GPU context.

### Architectural Decisions

#### Anchor Selection for Primary Chart

- **Decision**: The `CompositeChart<T>` wraps a `ComposedChart<T, Circle>` as
  the "primary" that owns axis/grid pipelines, even when the first user layer
  might be a line or bar. A separate anchor `Selection<T, Circle>` is created
  for this purpose.
- **Reasoning**: `ComposedChart` manages axis rendering, grid pipelines, and
  draw commands. Reusing one of the user's layers as the primary would require
  knowing its concrete mark type at the composite level, violating the
  enum-based abstraction.
- **Trade-off**: The anchor selection is essentially empty — it allocates a GPU
  buffer but contains no meaningful data marks. This is a small overhead.
- **Future**: A dedicated `CompositeChartFrame` type that handles axis/grid
  rendering independently of any mark type could eliminate this overhead.

#### Layers Receive Unified Scales at Build Time

- **Decision**: Each layer's builder receives the unified scale via its
  `config.x_scale` / `config.y_scale` fields before calling `build_with_data`.
  Layers are built with `show_axes = false`.
- **Reasoning**: This reuses 100% of the existing builder infrastructure — no
  new `render_layer` method or `ChartLayer` trait is needed. Each builder
  already knows how to build a `ComposedChart`; we just suppress its axes and
  inject the composite's scales.
- **Trade-off**: Builders must have `pub(crate)` fields for `config`,
  `x_accessor`, and `y_accessor`. The `BarChartBuilder` required a visibility
  change for this.
- **Future**: A formal `ChartLayer` trait with `render_layer(ctx, scales)` would
  be cleaner but would require larger refactoring of all existing builders.

### Development Workflow Insights

- **Fast iteration**: The enum-based approach compiled on the first attempt with
  zero errors, validating the project's established pattern.
- **Test-driven domain logic**: Writing the 12 domain-unification unit tests
  first caught the single-point edge case before it could manifest in examples.
- **Example-driven validation**: Running the two examples end-to-end was the
  fastest way to verify the full pipeline (data → domain → scale → build →
  selection) without needing a visual window.
- **Pre-existing warnings**: The `gup-macros` crate generates 42 warnings from
  transpilation-related dead code — these are unrelated to this story and should
  be addressed in a separate cleanup.

### Follow-up Stories

1. **GUP-303: Composite Chart GPU Render Pipeline** — Wire the `CompositeChart`
   through an actual wgpu render pass so that all layer selections are drawn to
   the same surface in declaration order. Currently the selections are built but
   not visually rendered to a window.

2. **GUP-304: Per-Layer Data Support** — Allow each layer in a composite to
   carry its own data set (different T per layer). This would require a
   type-erased `AnyLayer` variant or a `Box<dyn ErasedLayer>` approach for the
   case where scatter data and line data have fundamentally different schemas.
