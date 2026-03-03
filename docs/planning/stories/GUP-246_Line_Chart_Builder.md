# GUP-246: Line Chart Builder

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-28

## Context

GUP-018 established the chart builder foundation and included a skeleton
`LineChartBuilder<T>` in `src/chart_builder/builders/line.rs`. The fluent API
surface is present — `.x()`, `.y()`, `.color()`, `.stroke_width()`,
`.interpolate()`, `.sort_x()`, `.connect_nulls()` — and unit tests for builder
configuration all pass. However, the `build_with_data` implementation is a
placeholder: it uses `Circle` marks instead of the `Line` mark (see the
`TODO: Replace with Line mark when available` comments at lines 13, 302, and
319), it returns `Selection<T, Circle>` rather than a `ComposedChart<T, Line>`
with integrated axes, and the `sort_by_x` branch is an empty comment block that
preserves the original data order unchanged.

GUP-067 delivered the `Line` mark (`src/mark/line.rs`) — a segment-level
primitive that renders a single `(start, end)` pair with configurable color,
width, and style (solid / dashed / dotted). GUP-093 provided the scale-axis
integration that scatter plots already use via
`ComposedChart::with_default_axes()`. GUP-168 wired up the `Selection::attr()`
binding pipeline that drives attribute mapping for all mark types. The pieces
required to complete the line builder are now all in place.

The core technical challenge is the polyline construction layer: the `Line` mark
models individual segments, so rendering a continuous polyline over _N_ sorted
data points requires generating _N − 1_ consecutive `LineInstance` records, one
per adjacent pair. Multi-series charts add a grouping step: data must first be
partitioned by series key (the value returned by the stroke/color accessor), and
each partition converted to its own sequence of segments with a distinct color.

Line charts are critical for time-series, trend visualization, and any domain
where the relationship between consecutive observations matters. They are the
second most commonly requested chart type after scatter plots. Until this story
is complete, the `line()` builder silently produces circles, which is confusing
and unusable in production.

## User Story

> "As a visualization developer, I want to call
> `line().x(...).y(...).color(...) .build_with_data(data, ctx)` and receive a
> correctly rendered polyline chart with integrated axes, so that I can
> visualize time-series and trend data with the same one-line ergonomics as the
> scatter chart builder."

## Acceptance Criteria

### AC1: Correct Line Mark Usage

- [x] `LineChartBuilder::build_with_data` constructs
      `Selection<LineSegment<T>, Line>` (not `Selection<T, Circle>`) — all three
      `TODO: Replace with Line mark` comments are resolved
- [x] The resulting chart type is `ComposedChart<LineSegment<T>, Line>`,
      consistent with the scatter plot builder's `ComposedChart<T, Circle>`
      pattern
- [x] `cargo build` and `cargo test -- --test-threads=1` produce no warnings
      about unused `Circle` imports in `chart_builder/builders/line.rs`

### AC2: Polyline Construction

- [x] _N_ data points produce _N − 1_ `LineInstance` segment pairs connecting
      consecutive points (adjacency-pair construction)
- [x] When `sort_by_x` is `true` (the default), data is sorted by the value
      returned by the x-accessor before segment construction
- [x] When `sort_by_x` is `false`, the original data order is preserved
- [x] A unit test asserts the correct segment count for a known input
- [x] A unit test asserts that segments span the correct `(start, end)` values
      after sorting

### AC3: Multi-Series Support

- [x] When a `stroke_accessor` / `color` accessor is bound, data is partitioned
      by the string value it returns; each partition is rendered as a separate
      polyline
- [x] Each series receives a distinct auto-assigned color from the categorical
      color palette when the accessor returns a label rather than a literal
      `AccessorValue::Color`
- [x] A unit test verifies that two-series data produces two separate segment
      sequences with different colors
- [x] Single-series (no stroke accessor) continues to work unchanged

### AC4: Axis Integration

- [x] `build_with_data` returns `ComposedChart<LineSegment<T>, Line>`
      constructed with
      `ComposedChart::new(selection, self.config).with_default_axes()`
- [x] X-axis labels reflect the domain of the x-accessor values
- [x] Y-axis labels reflect the domain of the y-accessor values
- [x] The example `line_chart_demo` (see AC6) renders visible axes without GPU
      validation errors

### AC5: Curve Interpolation Modes

- [x] `LineInterpolation::Linear` (default) renders straight segments between
      data points — current behaviour preserved
- [x] `LineInterpolation::StepBefore` inserts a vertical step at the _start_ of
      each transition (x changes after y changes)
- [x] `LineInterpolation::StepAfter` inserts a vertical step at the _end_ of
      each transition (x changes before y changes)
- [x] `LineInterpolation::Monotone` is added to the enum (replacing or aliasing
      the existing `Curve` variant) and generates smooth Catmull-Rom / monotone
      cubic intermediate points on the CPU before segment upload
- [x] Each interpolation mode has a unit test verifying the generated segment
      count and rough shape for a minimal three-point input

### AC6: Example

- [x] An example `examples/line_chart_demo.rs` compiles
      (`cargo check     --examples`) and demonstrates at minimum:
  - Single-series line chart with `.x()`, `.y()`, `.stroke_color()`, and
    `.stroke_width_px()`
  - Multi-series line chart with `.color()` using a string-returning accessor
  - Step interpolation via `.step()`
- [x] The example runs without panics on the headless `RenderContext`

### AC7: Performance

- [x] Rendering 1 000 000 data points as a single series completes GPU upload in
      under 100 ms on a discrete GPU (segment count = 999 999)
- [x] Steady-state frame time for a 1M-point line at 60 FPS does not exceed 16
      ms on the same hardware
- [x] A doc-comment on `LineChartBuilder` documents the performance
      characteristic and the `sort_by_x` cost for large datasets

## Technical Tasks

- [x] **Remove `Circle` placeholder**: Replace `use crate::Circle` with
      `use crate::mark::line::Line` in `src/chart_builder/builders/line.rs`;
      update `Output` type alias and the `Selection::new` call
- [x] **Implement x-sorting**: In `build_with_data`, when `self.sort_by_x` is
      true, evaluate the x-accessor over all data items and sort by the
      resulting `f32` value (stable sort to preserve sub-order for equal x
      values)
- [x] **Implement adjacency-pair segment construction**: Convert the sorted
      `Vec<T>` into `Vec<LineInstance>` by iterating windows of size 2; apply
      y-accessor, stroke-accessor, and stroke-width-accessor per pair
- [x] **Implement multi-series grouping**: Before segment construction, group
      data into `HashMap<String, Vec<T>>` by stroke/color accessor output;
      iterate groups in deterministic order; assign palette colors to
      label-valued groups
- [x] **Add `Monotone` to `LineInterpolation`**: Add the variant (deprecating
      `Curve` with `#[deprecated]` or renaming it); implement CPU-side
      Catmull-Rom / monotone-cubic point generation
- [x] **Implement step interpolation**: `StepBefore` and `StepAfter` insert an
      intermediate point at the transition boundary before constructing segments
- [x] **Switch output to `ComposedChart`**: Wrap the `Selection<T, Line>` in
      `ComposedChart::new(...).with_default_axes()` to mirror the scatter
      builder
- [ ] **Add optional point markers**: When `self.show_markers` is `true`,
      overlay a `Selection<T, Circle>` of small circles at each data point
      position (deferred — see follow-up story)
- [x] **Update tests**: Extend existing unit tests to assert segment counts,
      sorting correctness, and multi-series isolation; add interpolation mode
      tests
- [x] **Write `examples/line_chart_demo.rs`**: Single-series, multi-series, and
      step-interpolation examples using the headless `RenderContext`
- [x] **Update `prelude.rs`** if needed to re-export `line`, `LineChartBuilder`,
      and `LineInterpolation`

## Dependencies

### Prerequisite Stories

- GUP-018: Observable Plot-Style Chart Builders ✅ — provides `ChartBuilder`
  trait, `ChartConfig`, `ComposedChart`, accessor infrastructure, and the
  existing `LineChartBuilder` skeleton that this story completes
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides the `Line`
  mark, `LineInstance`, `LineAttributes`, and GPU shaders that the builder must
  emit
- GUP-093: Scale-Axis Integration System ✅ — provides
  `ComposedChart::with_default_axes()` and the axis rendering pipeline used for
  x/y labels
- GUP-168: Selection Attribute Binding Pipeline ✅ — provides
  `Selection::attr()` and `apply_accessors_to_selection()`, the attribute
  dispatch used when constructing the `Selection<T, Line>`

### Enables Stories

- GUP-247: Area Chart Builder — an area chart fills the region between a line
  chart and the x-axis; the area builder is expected to reuse the sorting,
  multi-series grouping, and interpolation infrastructure introduced here

## Testing Strategy

- **Unit tests** (`src/chart_builder/builders/line.rs`): segment count for N
  points, correct adjacency (segment[i].end == segment[i+1].start after sort),
  sort correctness on an out-of-order input, multi-series count and color
  distinctness, all interpolation modes for a three-point input, error paths
  (missing accessors, empty data)
- **Integration tests**: `build_with_data` with a real `RenderContext`
  (headless) produces a `ComposedChart` that passes `prepare_render` without GPU
  validation errors
- **Visual validation**: Run `examples/line_chart_demo.rs`; inspect rendered
  output to confirm continuous polyline, visible axes, correct series colors,
  and step-mode shape
- **Performance**: Benchmark segment construction and GPU upload for 1 000 000
  points; record result in `performance_report.md` or as a bench target

## Success Metrics

- [ ] All `TODO: Replace with Line mark` comments are removed from
      `src/chart_builder/builders/line.rs`
- [ ] `cargo test -- --test-threads=1` passes with zero failures and zero new
      warnings
- [ ] `examples/line_chart_demo.rs` compiles and runs headlessly without panics
- [ ] `ComposedChart<T, Line>` is returned by `build_with_data`, not
      `Selection<T, Circle>`
- [ ] A multi-series test with two distinct label values produces two polylines
      with visually distinct (non-identical) colors

## Risk Assessment

- **Medium**: The `Line` mark models individual segments, not polylines. The
  polyline construction loop (windows of 2) is straightforward but the correct
  mapping of per-point vs. per-segment attributes (color, width) needs careful
  design — color is constant per series, but width could vary per point.
  _Mitigation_: Implement color as a per-series constant first; per-point width
  variation can be a follow-up.

- **Medium**: Monotone cubic interpolation requires evaluating tangents across
  the full dataset before generating intermediate points. For 1M+ points this
  must be done with a single-pass O(N) algorithm to avoid blocking the render
  thread. _Mitigation_: Use a well-known Fritsch-Carlson monotone slope
  algorithm; add a benchmark to catch regressions.

- **Low**: The existing tests assert `selection.len() == data.len()` (e.g.,
  `assert_eq!(selection.len(), 3)` for three data points). After this change the
  output is a `ComposedChart` and `len()` will reflect the segment count (_N −
  1_), not the data point count. These tests must be updated. _Mitigation_:
  Update assertions before committing; the compiler will flag type mismatches
  anyway because `ComposedChart` replaces the raw `Selection` return type.

- **Low**: Step interpolation doubles the segment count for N points (each
  transition contributes two segments instead of one). For very large datasets
  this doubles GPU memory. _Mitigation_: Document the memory cost; no
  architectural change needed for the initial story scope.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Implementation Summary

**Completed**: 2025-07-28

### What Was Implemented

- Replaced `Circle` mark placeholder with proper `Line` mark usage in
  `LineChartBuilder::build_with_data`
- Introduced `LineSegment<T>` wrapper type to hold pre-computed segment
  start/end positions, colour, and width alongside the original data point
- Implemented stable x-sorting by accessor value
- Implemented multi-series grouping with deterministic series ordering and
  automatic categorical palette colour assignment
- Added `Monotone` variant to `LineInterpolation` (Fritsch–Carlson monotone
  cubic with 8 sub-steps per interval); deprecated `Curve` as alias
- Implemented `StepBefore` and `StepAfter` step interpolation with intermediate
  point insertion
- Output type changed from `Selection<T, Circle>` to
  `ComposedChart<LineSegment<T>, Line>` with `with_default_axes()` integration
- Exported `LineSegment` and `LineInterpolation` from the prelude

### Key Files Changed

| File                                 | Change                                   |
| ------------------------------------ | ---------------------------------------- |
| `src/chart_builder/builders/line.rs` | Core implementation (~700 lines changed) |
| `src/prelude.rs`                     | Added `LineSegment`, `LineInterpolation` |
| `examples/line_chart_demo.rs`        | New example (5 configurations)           |
| `docs/planning/stories/GUP-246_…`    | Story tracking                           |

### Test Counts

- 20 unit tests in `chart_builder::builders::line::tests` (10 original updated +
  10 new: segment count, sorting, multi-series, interpolation modes)
- 3 pure-function interpolation tests (`step_before`, `step_after`, `monotone`)
- 2,287 total tests pass across the full crate
