---
implementer: sonnet
implementer-reason:
  a new builder following the line and bar builders, with the stacking
  alignment, row type and defaults fixed in Decisions
touches: [crates/gup-core/src/builder/mod.rs, crates/gup-core/src/lib.rs]
---

# GUP-547: The `gup::area` Builder and Stacked Areas

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-27) **Created**: 2026-10-24

## Context

With the `Area` mark (GUP-545) and its picking (GUP-546), an area chart still
takes a `Plot`, a layer per series and hand-computed stacking. The builders are
how newcomers draw charts (`gup::scatter`, `gup::line`, `gup::bar`), and area
and stacked area are the review's first missing type. This story adds
`gup::area` with overlapping series, stacking and an optional outline.

Stacking areas differs from stacking bars (`builder/bar.rs`, `stack_tables`) in
one way: an area is continuous in x, so every stacked series must have a point
at every x the stack uses, or a layer's top edge (interpolated over its own
rows) would not meet the next layer's base. The builder therefore aligns series
on the union of their x values, which means its layers cannot hold the user's
rows directly.

## Read First

- `crates/gup-core/src/builder/line.rs` (whole: splitting by key, colours,
  legend) and `builder/bar.rs` lines 100–135 and 300–420 (`stack_tables`,
  `materialise`).
- `crates/gup-core/src/builder/mod.rs` (the pattern, `plot_setters!`, the
  migration table).
- GUP-545's Decisions (`Area`'s channels) and GUP-546's (tooltips on areas).
- `crates/gup-core/tests/builders.rs` lines 250–470 (the line builder tests:
  byte-identical to a hand-built plot, first-seen order).

## User Story

> "As an analyst, I want `gup::area(rows).x(..).y(..).stack(..)` to draw a
> stacked area chart with a legend, so that I can show how parts make up a total
> over time."

## Acceptance Criteria

### AC1: The builder

- [x] `gup::area(rows) -> AreaChart<T>` with the methods in Decisions, exported
      from the crate root beside `line`; a doctest per mode (single, `color`,
      `stack`) from the `gup` crate.
- [x] A single area is byte-identical to the hand-built `Plot` with one `Area`
      layer, `y_end` 0, the default fill (test, as the line builder's).
- [x] `color(key)`: one area per key in first-seen order, each its palette
      colour at opacity 0.5 by default, with a legend (test from pixels at known
      points).

### AC2: Stacking

- [x] `stack(key)`: series in first-seen order from the bottom; at every x in
      the union of all series' x values each layer's base is the sum of the
      layers below (positive values upward from 0, negatives downward, as bars);
      a series missing an x contributes 0 there; a null value counts as 0;
      duplicate x within a series sum (unit tests on the stacking function).
- [x] Rows of each stacked series are sorted by x; unstacked series keep data
      order (tests).
- [x] Pixel test: on a 3-series stack, the pixel just above layer 1's top and
      just below layer 2's base at the same x is layer 2's and layer 1's colour
      respectively (no gap, no overlap: within 1 px).

### AC3: Outline, tooltips, picking

- [x] `outline(px)` adds one `Line` per series along its top edge (stacked: the
      layer's top) in the series colour at opacity 1 (test: the line's pixels
      are the opaque colour).
- [x] `tooltip(|r: &T| ..)` and `pick(at) -> Option<&T>` report the user's row
      nearest the pointer on the series under it; an aligned point that came
      from no user row shows no tooltip and picks `None` (tests).

### AC4: Visual

- [x] Golden `gup_core/builder_area_stacked` (640×400): three series over 24
      months with a legend and `grid(Axes::Y)`; golden `gup_core/builder_area`:
      two overlapping series. The worker reads both: bands meet cleanly, the
      legend lists series in first-seen order (the bottom layer first, as
      stacked bars do), no seams.
- [x] The builder module's table of builders and `examples/README.md`'s note on
      "More builders" mention `area`.

## Decisions

- **API** (setters take and return `self`, layer setters panic after the first
  draw, as every builder):

  ```rust
  pub fn area<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> AreaChart<T>;

  impl<T: Send + Sync + 'static> AreaChart<T> {
      // plot_setters!(): title, x_title, y_title, theme, grid, legends,
      // x_tick_format, y_tick_format
      pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn y_end<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn x_scale<S: PositionScale>(self, scale: S) -> Self;
      pub fn y_scale<S: PositionScale>(self, scale: S) -> Self;
      pub fn color<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn stack<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn color_scale(self, scale: Categorical) -> Self;
      pub fn fill(self, color: Color) -> Self;
      pub fn opacity(self, opacity: f32) -> Self;
      pub fn outline(self, px: f32) -> Self;
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self;
      pub fn layers_mut(&mut self) -> Vec<&mut Selection<AreaPoint, Area>>;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
      pub fn pick(&mut self, at: Point) -> Option<&T>;
  }

  /// One aligned point of an area layer.
  pub struct AreaPoint { pub x: f64, pub y: f64, pub y_end: f64, pub row: Option<usize> }
  ```

  ```rust
  let chart = gup::area(visits)
      .x(|v| v.day)
      .y(|v| v.count)
      .stack(|v| v.source.clone())
      .outline(1.0);
  ```

  Why: the same accessors as `line` (`x`, `y`, `color`), the same `stack` as
  `bar`; `y_end` is the baseline name the mark and `Rect` use, so a range band
  is `.y(|r| r.high).y_end(|r| r.low)`. `outline` names the visible result (a
  line along the top).

- **Row type**: every layer is a `Selection<AreaPoint, Area>`; the builder keeps
  the user's rows in an `Arc<[T]>` and `AreaPoint::row` indexes them (`None` for
  a point stacking inserted). Tooltips and `pick` map through it. This is why
  `layers_mut` names `AreaPoint`, documented.
- **Defaults**: baseline 0 (`y_end` not set); opacity 1 for a single area and
  for stacks, 0.5 for `color` without `stack` (overlapping areas must not hide
  each other); outline off. `opacity(..)` overrides all. The last of `color`,
  `stack` and `fill` wins for colouring, as in `bar`; `stack` ignores `y_end`
  (documented).
- **Stacking** lives in a pure function `stack_areas` in `builder/area.rs`
  (unit-tested like `stack_tables`), keyed by `f64::to_bits` of x after
  normalising `-0.0` to `0.0`.
- **Appending** is not supported by the area builder (no `appender`); a live
  area uses a `Plot` with `Plot::area` and `Selection::appender`.

## Escalation Triggers

- Two stacked layers' shared edge shows a gap or overlap at a chunk boundary
  (bridges, GUP-432) in the pixel test: report; it would be a mark bug.
- A builder-owned row type breaks `Chart`, `show` or `gup-egui` use of the chart
  (it should not: `T` is still the chart's parameter).

## Out of Scope

- Curved areas (after GUP-551), streamgraph offsets, normalised (100%) stacks,
  horizontal areas.
- An appender for the area builder.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] `builder/area.rs` with single and `color` modes; byte-identical test;
      commit.
- [x] `stack_areas` with unit tests; stacked mode; pixel test; commit.
- [x] `outline`, tooltips, `pick`; goldens; docs; commit.

## Dependencies

### Prerequisite Stories

- GUP-545: The `Area` mark 📋; GUP-546: area picking 📋.
- GUP-537: owned category keys 📋 — `color` and `stack` take owned keys through
  `builder::erase_key`.
- GUP-541: Builder tick formats 📋 (soft) — `plot_setters!` gains the format
  setters there.

### Enables Stories

- GUP-536: the chart tour shows areas.

## Testing Strategy

- **Unit tests**: `stack_areas` (alignment, nulls, duplicates, signs).
- **Integration tests**: byte-identical single area, colours, stack edges,
  outline, tooltips, pick.
- **Visual validation**: the two goldens, read by eye.

## Success Metrics

- [x] Area, overlapping-area and stacked-area charts are one call chain each.

## Risk Assessment

- **Low**: the aligned row type surprises users of `layers_mut`. _Mitigation_:
  documented with an example; `pick` and `tooltip` still speak in `T`.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the two goldens)
- [x] `mask wasm-browser`: not needed, builder only
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/builder/area.rs`: `area`, `AreaChart<T>`, `AreaPoint` and
  the pure `stack_areas` (union of x, signed stacks, nulls as 0, duplicate x
  summed, `-0.0` normalised). Exported from `builder` and the crate root (lines
  added only). Setters follow Decisions; `plot_setters!()` gives the title,
  subtitle, tick-format, theme, grid and legend setters.
- Layers are `Selection<AreaPoint, Area>`; the builder keeps the rows in an
  `Arc<[T]>` and tooltips and `pick` map back through `AreaPoint::row`.
- Tests: 4 unit tests (`stack_areas`), 10 integration tests in
  `crates/gup-core/tests/builder_area.rs` (byte-identical single area, `color`
  pixels and legend, stack edges with no gap or overlap, sorting, outline, pick
  and tooltips, last colour call, goldens), 5 doctests (3 in gup-core, 2 in the
  `gup` crate root).
- Goldens: `tests/golden/gup_core/builder_area.png` and
  `builder_area_stacked.png`, both read: the stacked bands meet with no seam,
  the baseline is the x axis, the legend lists Web, Mobile, Partner from the
  bottom layer up, in palette colours; the overlapping chart shows the two
  translucent areas blending where they overlap, with opaque outlines, and one
  legend entry per series (the outline layers add none).
- `mask merge-check --dogfood`: PASS (gup-core 503 s). WASM scatter gz 343472 B
  (pass), over wgpu 311112 B.

## Retrospective

**Completed**: 2026-10-27

### Key Technical Learnings

#### Stacking that cannot meet in the middle

- **Challenge**: B missing a row at x makes B's band a sliver between its
  neighbours there, so "the top of the stack at an odd x" is not the sum of the
  layer values; my first test expectations were wrong, not the builder.
- **Solution**: the test computes the expected tops from the aligned data
  (`tops`), mid-way between points.
- **Pattern**: assert stacked edges from the same alignment rule the builder
  documents (missing counts 0), never from the raw rows.

#### Layers over a derived row type

- **Challenge**: a layer's rows are `AreaPoint`, so key colouring cannot read
  the user's key from the row.
- **Solution**: each layer is one series, so its colour encoding returns that
  series' label constant; one shared `ScaleRef<Categorical>` keeps the legend to
  one entry per series even with outline `Line` layers on the same scale.
- **Pattern**: when rows are derived, encode per layer, not per row.

### Architectural Decisions

#### Missing accessors left to the layer's own error

- **Decision**: with no `x` or `y`, the layers just lack that channel (the
  points hold NaN), so resolve reports the usual missing-encoding config error.
- **Reasoning**: matches the other builders' messages without a second check.
- **Trade-off**: the error names `Area::X`, not the builder method.

### Development Workflow Insights

- Compound shell commands are refused in a worktree-isolated agent; use the edit
  tools and plain separate commands.
- The branch was behind main when started (no GUP-546); fast-forwarded.

### Follow-up Stories

None. GUP-576 and GUP-577 are unused.
