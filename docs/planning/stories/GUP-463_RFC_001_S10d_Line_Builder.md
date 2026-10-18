---
implementer: sonnet
implementer-reason:
  follows GUP-462's builder pattern; the series split, null values and new
  `Plot::layers_mut` are fixed in Decisions
---

# GUP-463: RFC-001 Step S10d: `gup_core::line` with Series and Nulls

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-18

## Context

Dogfood task 1 is the line chart users most often want: a long-format CSV (date,
ticker, price) with three series, missing prices, a date axis, a legend and a
title. The old `line()` builder splits series only through a categorical
`.color(..)`, needs domains by hand, and draws no text in PNGs.

`gup-core` draws one polyline per `Line` layer (GUP-432). A null row breaks the
line, and a null is a non-finite value. Calendar ticks come from `Time`
(GUP-418). What is missing:

- a builder that splits long-format rows into one `Line` layer per series, with
  colours from one shared `Categorical` (one legend, GUP-433's shared
  dictionary);
- a way to write `Option<f64>` prices without `unwrap_or(f64::NAN)`;
- a typed way to reach all of a plot's layers of one type (`Plot::layer_mut`
  returns only the first).

This story follows the pattern GUP-462 sets: module, materialisation rule,
`plot_setters!`, escape hatches.

## Read First

- [GUP-462](GUP-462_RFC_001_S10c_Chart_Builders_And_Scatter.md) Decisions
  (whole), and its merged `crates/gup-core/src/builder/{mod,scatter}.rs`.
- RFC-001 "GUP-432 findings" (lines 3163–3300): polylines, bridges, nulls.
- `crates/gup-core/src/encoding.rs` lines 590–620 (`ColumnValue`).
- `crates/gup-core/src/plot.rs` lines 505–545 (`line`, `add`, `layer_mut`).
- `crates/gup-core/tests/line_png.rs` lines 280–360 (null rows and chunk
  boundaries).
- `src/chart_builder/builders/line.rs` method list: `grep -n "pub fn"` only.

## User Story

> "As a developer with a long-format time series, I want
> `gup_core::line(rows).x(..).y(..).color(|r| r.ticker.as_str())` to draw one
> line per ticker with a legend and a date axis, so that I don't reshape my data
> or compute domains."

## Acceptance Criteria

### AC1: API

- [ ] `gup_core::line`, `LineChart<T>`, `Plot::layers_mut` and the two
      `ColumnValue` impls exist as in Decisions, with doctests. Closures need no
      annotations.

### AC2: Series and byte-identity

- [ ] In `tests/builders.rs`, 3 series × 200 rows, interleaved by date as a CSV
      would be, render byte-identical with
      `line(rows).x(..).y(..).color(|r| r.ticker.as_str())` and with the
      equivalent hand-built `Plot`: three `plot.line(..)` layers over the
      partitioned rows, each `.color(cat.encode_key(..))` through one shared
      `ScaleRef<Categorical>`.
- [ ] Series appear in first-seen key order, which is also the legend order.
      Rows keep their data order within each series.

### AC3: Golden

- [ ] A golden, `gup_core/builder_line`, at 800 × 450:
  - 3 series of daily values over 5 years, with x through
    `.x_scale(Time::new())` and Unix seconds;
  - y as `Option<f64>`, with a 10-day gap of `None` in one series;
  - `.grid(Axes::Y)`, `.legend_title("Ticker")` and a title.

  The implementer reads it: year ticks on x, three coloured lines, one visible
  gap, one titled legend, grid lines.

- [ ] Pixel check: on the gap series, the column at the middle of the gap has
      none of that series' colour inside the plot rect. Columns just either side
      have it.

### AC4: Escape hatches

- [ ] `layers_mut()` returns the 3 layers in series order. Setting `.width(..)`
      on one through it changes only that series (pixel check of thickness).
- [ ] `pick(at)` on a line returns the nearer endpoint row of the segment under
      `at` (the S8b rule).
- [ ] A layer setter after the first render panics with GUP-462's message.

### AC5: `Option` values everywhere

- [ ] `plot.circles(rows).x(|r| r.a).y(|r| r.maybe)` with `maybe: Option<f64>`
      compiles, and `None` rows are not drawn (pixel check). `Option<f32>` too.

## Decisions

- **API** (beside GUP-462's, same conventions):

  ```rust
  pub fn line<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> LineChart<T>;

  impl<T: Send + Sync + 'static> LineChart<T> {
      pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn x_scale<S: PositionScale>(self, scale: S) -> Self;
      pub fn y_scale<S: PositionScale>(self, scale: S) -> Self;
      /// One line per key, in first-seen order, coloured by key.
      pub fn color(self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self;
      pub fn color_scale(self, scale: Categorical) -> Self;     // default Okabe-Ito
      pub fn stroke(self, color: Color) -> Self;                // one colour for every line
      pub fn width(self, px: f32) -> Self;                      // default Line's 1.5
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self;
      // plot_setters!: title, x_title, y_title, theme, grid, legends
      pub fn layers_mut(&mut self) -> Vec<&mut Selection<T, Line>>;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
      pub fn pick(&mut self, at: geom::Point) -> Option<&T>;
  }
  impl<T: Send + Sync + 'static> Chart for LineChart<T> { /* as GUP-462 */ }

  impl Plot {
      /// Every layer of type `Selection<T, M>`, in the order added.
      pub fn layers_mut<T: Send + Sync + 'static, M: Mark>(&mut self)
          -> impl Iterator<Item = &mut Selection<T, M>>;
  }

  impl ColumnValue for Option<f64> { type Gpu = f32; /* None → NaN, a null */ }
  impl ColumnValue for Option<f32> { type Gpu = f32; /* None → NaN */ }
  ```

- **Split**: at materialisation the rows are partitioned by key, moving rows (no
  `Clone` bound), into one `Vec<T>` per key in first-seen order. Each `Vec`
  becomes `plot.line(..)`. Without `.color`, there is one layer. `.color` and
  `.stroke` are exclusive; the last call wins.
- **No sorting**: rows are drawn in data order within a series, as `Plot::line`
  draws them. The docs say to sort by x first.
- **Dropped from the old builder**: `interpolate`, `smooth`, `monotone`, `step`
  (curves are a follow-up), `sort_x` (sort the rows), and `connect_nulls(true)`
  (filter the null rows). Nulls always break the line.

## Escalation Triggers

- AC2 is not byte-identical.
- A position closure bound to a `Time` x scale (`x_scale(Time::new())`) fails to
  build or misplaces points. GUP-447 tested `Linear` and `Log`.
- The `Option` impls make an existing closure in the workspace ambiguous.

## Out of Scope

- Area charts and curves (T5, after S14).
- Appending to a line (`appender`): RFC-001 S12.

## Technical Tasks

- [ ] `ColumnValue` for `Option<f64>` and `Option<f32>`; AC5 test.
- [ ] `Plot::layers_mut`.
- [ ] `builder/line.rs`: `line`, `LineChart`, split, `Chart` impl.
- [ ] Tests and golden (AC2–AC4).

## Dependencies

### Prerequisite Stories

- GUP-462: RFC-001 S10c Chart Builders and Scatter 📋 — the builder pattern.
- GUP-432: RFC-001 S6f Line Mark ✅.
- GUP-460: RFC-001 S10a Grid Lines 📋 — via `plot_setters!`.

### Enables Stories

- GUP-465: dogfood task 1.

## Testing Strategy

- **Integration**: byte-identity against a hand-built plot; series order; gap
  pixels; per-layer edits; panics (`tests/builders.rs`).
- **Visual**: `gup_core/builder_line.png`, read by the implementer.

## Success Metrics

- [ ] Dogfood task 1's chart is about ten lines of user code, with no domains,
      SVG or legend workaround.

## Risk Assessment

- **Low**: many series (hundreds of keys) make many layers and pipelines.
  _Mitigation_: layers with one signature share one pipeline (the glue is the
  same). Documented as fine up to a few hundred series.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: `gup_core/builder_line.png`
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
