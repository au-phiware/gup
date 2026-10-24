---
implementer: sonnet
implementer-reason:
  a pure binning function with fixed rules plus a builder over Rect, following
  the bar builder; edges, defaults and the row type are fixed in Decisions
touches: [crates/gup-core/src/builder/mod.rs, crates/gup-core/src/lib.rs]
---

# GUP-548: The Histogram Builder and a Bin Transform

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

A histogram is the first chart most people draw of a numeric column, and Gup
cannot draw one without binning by hand: the review lists "histogram (no bin
transform)" among missing chart types. The drawing is already possible: a bin is
a `Rect` from its lower to its upper edge on a numeric x axis and from 0 to its
count on y (GUP-494 made numeric-axis bars fit their domains and baselines).
What is missing is a binning function with good default edges and a builder that
hides the transform.

Good edges matter more than any other histogram detail: bins should start and
end on round numbers (1, 2 or 5 times a power of ten), so the axis ticks fall on
bin edges. `scale::tick_step` already computes such a step for ticks.

## Read First

- `crates/gup-core/src/scale/mod.rs` lines 493–520 (`tick_step`).
- `crates/gup-core/src/builder/bar.rs` (the builder pattern, stacking with
  `stack_tables`, colours and legend).
- `crates/gup-core/src/scale/offset.rs` (`Offset::by`, `Offset::px`) and
  `EncodeFn::then` in `encoding.rs`.
- `crates/gup-core/tests/rect_png.rs` (numeric-axis bars:
  `numeric_axis_bars_fit_and_a_derived_baseline_is_in_the_domain`).
- `crates/gup-core/src/builder/mod.rs` (module docs and the builder list).

## User Story

> "As an analyst, I want `gup::histogram(rows).x(|r| r.value)` to show the
> distribution of a column with sensible bins, so that I can see its shape in
> one line."

## Acceptance Criteria

### AC1: `transform::bin`

- [ ] `gup_core::transform` (new public module) has `Bins`, `Bin` and `bin` as
      in Decisions, documented with doctests.
- [ ] Unit tests: `Bins::Auto` on 1,000 values spanning 0.3–9.7 gives 10 bins of
      width 1 from 0 to 10 (Sturges asks for ⌈log2 1000⌉ + 1 = 11 bins;
      `tick_step(0.3, 9.7, 11)` is 1); values on an inner edge go to the upper
      bin, the maximum to the last bin; NaN and infinities are dropped;
      `Bins::Width(2.5)` gives edges at multiples of 2.5;
      `Bins::Edges(vec![0.0, 1.0, 10.0])` gives two bins and drops values
      outside 0–10; all-equal input gives one bin of width 1 centred on the
      value; empty input gives no bins.

### AC2: The builder

- [ ] `gup::histogram(rows) -> HistogramChart<T>` with the methods in Decisions,
      exported from the crate root; a doctest from the `gup` crate asserting the
      bar count from `layers_mut()`.
- [ ] The x axis domain is exactly the first to the last edge, and its ticks
      fall on bin edges for `Bins::Auto` (test: every x tick value is an edge).
- [ ] Bars are separated by a 1 px gap when a bin is at least 4 px wide, and
      touch otherwise (pixel test at two widths).
- [ ] `color(key)` stacks each bin's counts by key in first-seen order with a
      legend (test: per-bin totals equal the unkeyed counts).
- [ ] The y axis title defaults to "Count"; `y_title(..)` replaces it.
- [ ] `tooltip(|b: &HistogramBin| ..)` and `pick(at) -> Option<&HistogramBin>`
      work (test).

### AC3: Visual

- [ ] Golden `gup_core/histogram` (640×400): 2,000 deterministic values from two
      overlapping normal-like humps (a fixed LCG plus Box–Muller in the test),
      stacked by hump. The worker reads it: round-number edges, ticks on edges,
      1 px gaps, legend, no clipped first or last bar.

## Decisions

- **The transform** (`crates/gup-core/src/transform.rs`):

  ```rust
  pub enum Bins {
      /// About ⌈log2 n⌉ + 1 bins (Sturges), on a 1–2–5 step.
      Auto,
      /// About this many bins, on a 1–2–5 step.
      Count(usize),
      /// Bins of exactly this width, edges at its multiples.
      Width(f64),
      /// These edges, ascending (n edges make n − 1 bins).
      Edges(Vec<f64>),
  }
  pub struct Bin { pub lo: f64, pub hi: f64, pub count: usize }
  pub fn bin(values: impl IntoIterator<Item = f64>, bins: &Bins) -> Vec<Bin>;
  ```

  Edge rule for `Auto` and `Count(k)`: `step = tick_step(min, max, k)`; edges
  from `floor(min / step) * step` to `ceil(max / step) * step`, edge `i` being
  `scale::step_value(k0 + i, step)` with `k0 = floor(min / step)` (never
  accumulated). Every bin is `[lo, hi)` except the last, `[lo, hi]`.

- **The builder**:

  ```rust
  pub fn histogram<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> HistogramChart<T>;

  impl<T: Send + Sync + 'static> HistogramChart<T> {
      // plot_setters!()
      pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn bins(self, bins: Bins) -> Self;
      pub fn x_domain(self, lo: f64, hi: f64) -> Self;
      pub fn color<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn color_scale(self, scale: Categorical) -> Self;
      pub fn fill(self, color: Color) -> Self;
      pub fn opacity(self, opacity: f32) -> Self;
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      pub fn tooltip(self, text: impl Fn(&HistogramBin) -> String + Send + Sync + 'static) -> Self;
      pub fn layers_mut(&mut self) -> Vec<&mut Selection<HistogramBin, Rect>>;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
      pub fn pick(&mut self, at: Point) -> Option<&HistogramBin>;
  }

  pub struct HistogramBin {
      pub lo: f64, pub hi: f64, pub count: usize,
      pub key: Option<Arc<str>>, pub base: f64, pub top: f64,
  }
  ```

  ```rust
  let chart = gup::histogram(trips).x(|t| t.minutes).bins(Bins::Width(5.0));
  ```

  Why: one accessor, as every builder; `bins` takes the one enum a user reads in
  the docs; `x_domain` restricts the binned range (values outside are dropped)
  and fixes the axis, the only scale control a histogram needs. Tooltips speak
  in bins because a bar is a bin, not a row.

- **Exports**: `histogram`, `HistogramChart` and `HistogramBin` from the crate
  root; `Bins` from the crate root and the prelude (so the example above needs
  no extra import); `Bin` and `bin` from `gup_core::transform` only.
- **Axes**: x is `Linear` with the explicit domain `[first edge, last edge]` (so
  `x_domain` and the edges agree); there is no `x_scale` setter (log-binned
  histograms are out of scope). y is `Linear`, automatic, baseline 0.
- **Gap**: each bar's `x` and `x_end` go through the x `ScaleRef` then
  `Offset::by(Derived)` of `+0.5` and `−0.5` px, where the `Derived` reads the
  bin's pixel width at resolve and returns 0 below 4 px.
- **Colours**: unkeyed bins use `Rect::DEFAULT_FILL`; keyed bins stack like
  `bar`'s `stack` (one layer per key, Okabe-Ito, legend).
- **Row values**: `x` accepts any `ColumnValue`; `None` and NaN rows are
  skipped.

## Escalation Triggers

- `tick_step`'s edges leave the last value outside the last bin for some input
  (a floating-point edge case): report the input; do not widen by an epsilon
  without recording it.
- The `Offset` chain on the plot's x `ScaleRef` breaks CPU picking or SVG output
  for the bars (they must use the same CPU mirror).
- `mask wasm-size` scatter grows by more than 4 KB gz.

## Out of Scope

- Density estimates, cumulative or normalised histograms, 2D binning (the
  heatmap builder takes categories, GUP-549), horizontal histograms, log bins.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] `transform.rs` with `bin` and its unit tests; commit.
- [ ] `builder/histogram.rs`, exports, doctest; commit.
- [ ] Gap, stacking, y title, tooltip and pick tests; golden; commit.

## Dependencies

### Prerequisite Stories

- GUP-494: numeric-axis bars ✅.
- GUP-537: owned category keys 📋 — `color` takes an owned key through
  `builder::erase_key`; `HistogramBin::key` is its label.
- GUP-540: explicit-domain edge inset 📋 (soft): bars on the explicit x domain
  have no x overhang, so nothing changes, but run after it (both affect
  explicit-domain layouts).

### Enables Stories

- GUP-554: the box plot adds its statistics to `transform`.
- GUP-536: the chart tour shows a histogram.

## Testing Strategy

- **Unit tests**: `bin` edge rules.
- **Integration tests**: axis edges, gaps, stacking, title, tooltip, pick.
- **Visual validation**: `gup_core/histogram`, read by eye.

## Success Metrics

- [ ] One call draws a histogram whose bins start and end on round numbers.

## Risk Assessment

- **Low**: Sturges under-bins very large inputs (1M values: 21 bins).
  _Mitigation_: `Bins::Count` and `Bins::Width` are one call away; the docs say
  so.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`gup_core/histogram`)
- [ ] `mask wasm-browser`: not needed, builder only
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
