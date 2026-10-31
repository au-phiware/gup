---
implementer: sonnet
implementer-reason:
  quartile statistics with a named rule plus a builder of Rect, Segment and
  Circle layers on a band axis; every rule and mark mapping fixed below
touches: [crates/gup-core/src/builder/mod.rs, crates/gup-core/src/lib.rs]
---

# GUP-554: The Box Plot Builder

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress **Created**: 2026-10-24

## Context

A box plot compares distributions across categories (response times per service,
scores per group) and is a standard chart in science and engineering. The review
closes N2 with "Then box plot." Gup has the marks: a `Rect` for the box on a
band axis, `Segment`s for the median and whiskers, `Circle`s for outliers. What
is missing are the statistics and a builder; GUP-548 adds the `transform` module
this story extends.

The definition matters because box plots differ between tools. This story uses
the most common one (Tukey's): quartiles by linear interpolation between order
statistics (the default "type 7" of R and NumPy), whiskers to the most extreme
data within 1.5 IQR of the box, and points beyond drawn individually.

## Read First

- GUP-548's `crates/gup-core/src/transform.rs` (module layout and docs).
- `crates/gup-core/src/builder/bar.rs` (band axis, key domains, colours).
- `crates/gup-core/src/scale/band.rs` lines 200–300 (`band_width_px`,
  `sub_band`) and `scale/offset.rs` (`Offset::by`).
- `crates/gup-core/src/marks.rs` (`Rect`, `Segment`, `Circle` channels).

## User Story

> "As an engineer comparing latency across services, I want
> `gup::boxplot(rows).x(|r| r.service).y(|r| r.ms)`, so that I see each
> service's median, spread and outliers side by side."

## Acceptance Criteria

### AC1: `transform::box_stats`

- [x] `BoxStats` and `box_stats` as in Decisions, documented.
- [x] Unit tests against values computed by hand (and matching NumPy's default
      `percentile`): `[1, 2, 3, 4]` gives q1 1.75, median 2.5, q3 3.25;
      `[1..=9, 100]` reports 100 as the only outlier and the upper whisker at 9;
      a single value gives every statistic equal to it; NaN is dropped; empty
      input gives `None`.

### AC2: The builder

- [x] `gup::boxplot(rows) -> BoxPlotChart<T>` with the methods in Decisions,
      exported from the crate root; a doctest from the `gup` crate.
- [x] Per category (first-seen order on a band x axis): a box from q1 to q3, a
      median line across the box, whiskers from the box to each whisker end with
      caps half the box width, outliers as circles (pixel tests on one category
      with known statistics, positions within 1 px).
- [x] `color(key)` colours each box by a key with a legend; `fill(c)` sets one
      colour; the default is `#0072B2`.
- [x] `tooltip(|s: &BoxSummary| ..)` shows on hover over the box (test).
- [x] The y axis domain includes every outlier and whisker (test).

### AC3: Visual

- [x] Golden `gup_core/boxplot` (640×400): five categories of 200 deterministic
      values each (an LCG with Box–Muller, two with injected outliers). The
      worker reads it: boxes centred in bands, medians visible against the fill,
      whisker caps aligned, outliers distinct.

## Decisions

- **Statistics** (`transform.rs`):

  ```rust
  pub struct BoxStats {
      pub n: usize,
      pub q1: f64, pub median: f64, pub q3: f64,
      /// The smallest value at or above q1 − 1.5 IQR, and the largest at or
      /// below q3 + 1.5 IQR.
      pub whisker_lo: f64, pub whisker_hi: f64,
      /// Values outside the whiskers, ascending.
      pub outliers: Vec<f64>,
  }
  pub fn box_stats(values: impl IntoIterator<Item = f64>) -> Option<BoxStats>;
  ```

  Quantile `p` of sorted `v` (length `n`): `h = (n − 1) p`,
  `v[⌊h⌋] + (h − ⌊h⌋)(v[⌊h⌋ + 1] − v[⌊h⌋])`.

- **Builder API**:

  ```rust
  pub fn boxplot<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> BoxPlotChart<T>;

  impl<T: Send + Sync + 'static> BoxPlotChart<T> {
      // plot_setters!()
      pub fn x<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn y_scale<S: PositionScale>(self, scale: S) -> Self;
      pub fn padding(self, p: f64) -> Self;
      pub fn color<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn color_scale(self, scale: Categorical) -> Self;
      pub fn fill(self, color: Color) -> Self;
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      pub fn tooltip(self, text: impl Fn(&BoxSummary) -> String + Send + Sync + 'static) -> Self;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
  }

  pub struct BoxSummary { pub key: Arc<str>, pub stats: BoxStats }
  ```

  ```rust
  let chart = gup::boxplot(requests).x(|r| r.service.clone()).y(|r| r.ms);
  ```

  Why: the same `x`/`y` as `bar` (category, value); the summary type is what a
  tooltip about a box needs.

- **Marks** (one layer each, over `BoxSummary` rows or outlier rows): box `Rect`
  (x band, `width` = band width × 0.6, `y` q3, `y_end` q1, fill at
  `fill_opacity` 0.3, stroke the colour at 1 px); median `Segment` across the
  box (x through the band then `Offset::by(±box_width / 2)`), width 2; whisker
  `Segment`s (vertical, width 1) and caps (horizontal, half the box width);
  outlier `Circle`s, radius 2.5, the colour. `color(key)` colours all of a
  category's marks through one `Categorical`.
- **Rows**: the builder computes summaries at materialise; `plot_mut` exposes
  the layers; there is no `layers_mut` (four layer types).

## Escalation Triggers

- The median segment's `Offset` chain on a band `ScaleRef` does not encode keys
  (it should, as `sub_band` does): report with a repro.
- `mask wasm-size` scatter grows by more than 4 KB gz.

## Out of Scope

- Horizontal box plots, notches, violin plots, swarm overlays, grouped boxes.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] `box_stats` with unit tests; commit.
- [x] `builder/boxplot.rs`; pixel tests; commit.
- [x] Colours, tooltip, domain test, golden; commit.

## Dependencies

### Prerequisite Stories

- GUP-548: histogram and bin transform 📋 — the `transform` module.
- GUP-537: owned category keys 📋 — `x` and `color` take owned keys through
  `builder::erase_key`; `BoxSummary::key` is the label.

### Enables Stories

- GUP-536: the chart tour shows a box plot.

## Testing Strategy

- **Unit tests**: `box_stats` against hand-computed values.
- **Integration tests**: mark positions, colours, tooltip, domain.
- **Visual validation**: `gup_core/boxplot`, read by eye.

## Success Metrics

- [x] A box plot with outliers is one call chain, with statistics matching
      NumPy's defaults.

## Risk Assessment

- **Low**: four layers per chart. _Mitigation_: layer count is fixed (not per
  category), so draw calls stay at four plus outliers.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`gup_core/boxplot`)
- [x] `mask wasm-browser`: not needed, builder only
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `transform::box_stats` and `BoxStats` (type-7 quartiles, Tukey whiskers;
  non-finite values dropped), with unit tests against NumPy values.
- `gup::boxplot` / `BoxPlotChart` / `BoxSummary` in `builder/boxplot.rs`: layers
  are boxes (Rect), whiskers, caps, medians (Segments, the last two through
  `band.then(Offset::by(..))`) and outliers (Circles, only if any).
  `color(key)`, `fill`, `tooltip`, `pick`, `plot_mut`, `into_plot`.
- `crates/gup-core/tests/boxplot_png.rs` (6 tests: marks within a pixel, domain,
  colours, tooltip and pick, order, golden `gup_core/boxplot`), a `gup` crate
  doctest and a `gup-core` doctest.
- Evidence: golden read by eye: five boxes centred in their bands, medians
  darker than the 0.3-opacity fill, caps aligned and half a box wide, outliers
  as distinct discs (search 210/232, checkout 12/262). `mask merge-check` PASS;
  `cargo test --doc -p gup` 56 + 4 passed. WASM scatter gz 345615 B (budget
  341760, +3855 B vs the recorded figure, inside the 4 KB trigger and the pass
  band).

## Retrospective

**Completed**: 2026-10-31

### Key Technical Learnings

#### Hit order hides tooltips on shared edges

- **Challenge**: a tooltip test hovering `pick_anchor` (the box's top-edge
  midpoint) did nothing: the whisker segment starts exactly there and wins the
  hit.
- **Solution**: hover inside the box. **Pattern**: anchors of a layer under
  another layer's mark are not hoverable.

#### Unfitted `Categorical::eval`

- **Challenge**: `Categorical::okabe_ito().eval(0.0)` is grey, not the first
  colour, until the scale has keys.
- **Solution**: tests name the palette colours.

### Architectural Decisions

#### Value axis keeps the plain linear rule

- **Decision**: GUP-561's "round up to a tick strictly above the max" is not
  applied to the box plot: its axis is not anchored at zero and no mark sits at
  the data end other than outlier discs. Revisit if a disc clips.
- **Trade-off**: the top outlier may sit close to the plot edge.

#### A category's colour key is its first row's

- **Decision**: `color(key)` per category takes the first row's key.

### Development Workflow Insights

- The first session ended on a usage limit before any commit; the checkpoint ref
  preserved the tree.

### Follow-up Stories

None.
