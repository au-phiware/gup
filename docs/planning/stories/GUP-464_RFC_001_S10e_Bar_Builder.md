---
implementer: sonnet
implementer-reason:
  follows GUP-462's builder pattern; baseline, grouping (GUP-461 sub-bands),
  stacking arithmetic and colour defaults are fixed in Decisions
---

# GUP-464: RFC-001 Step S10e: `gup_core::bar`, Grouped and Stacked

## Story Overview

2026-10-18

## Context

Dogfood task 2 draws sales by quarter and region as grouped and as stacked bars.
In the old builder, `group_by` and `stack_by` change nothing (tracked gaps
`NO_GROUPING` and `NO_STACKING`), and `.color(String)` renders grey
(`CATEGORICAL_GREY`). The task's workaround encodes (quarter, region) into one
band key and overlaps cumulative bars by hand.

In `gup-core`, a bar is a `Rect` with its x through a `Band`
(`x.encode_key(..)`) and its width from `x.band_width_px()`. The `Rect`
documentation shows this, but it takes eight lines and a `Derived` baseline.
GUP-461 adds `ScaleRef<Band>::sub_band` for side-by-side bars. Stacking is a
per-category running sum, which the builder computes on the CPU before upload.
The GPU draws ordinary rects with two y edges. This story follows GUP-462's
builder pattern.

## Read First

- [GUP-462](GUP-462_RFC_001_S10c_Chart_Builders_And_Scatter.md) Decisions, and
  the merged `crates/gup-core/src/builder/{mod,scatter}.rs`;
  [GUP-463](GUP-463_RFC_001_S10d_Line_Builder.md) Decisions (series split,
  `Plot::layers_mut`).
- [GUP-461](GUP-461_RFC_001_S10b_Offset_And_Sub_Bands.md) Decisions (`sub_band`,
  `sub_band_width_px`).
- `crates/gup-core/src/marks.rs` `Rect` docs (two-edge mode, `x_end`/`y_end`).
- `crates/gup-core/tests/rect_png.rs` and the `grouped_bars` test from GUP-461.
- `dogfood/src/suite.rs` `t2()` (lines 592–722): the checks GUP-465 must pass.
- `src/chart_builder/builders/bar.rs` method list: `grep -n "pub fn"` only.

## User Story

> "As a developer reporting sales by quarter and region, I want
> `gup_core::bar(rows).x(..).y(..).group(..)` (or `.stack(..)`) to draw a
> correct grouped or stacked bar chart with a legend, so that I don't fake the
> layout with composite keys."

## Acceptance Criteria

### AC1: API

- [x] `gup_core::bar` and `BarChart<T>` exist as in Decisions, with doctests.
      Closures need no annotations.

### AC2: Simple bars, byte for byte

- [x] `bar(rows).x(|r| r.month.as_str()).y(|r| r.total)` renders byte-identical
      to the equivalent hand-built `Plot`, in `tests/builders.rs`: one
      `plot.rects`, x through `Band` with `encode_key`, width `band_width_px()`,
      `.y(..)`, `.y_end(|_| 0.0)`.
- [x] The y domain includes 0 when every value is positive (bars start at the x
      axis) and when every value is negative (bars hang from 0).

### AC3: Grouped

- [x] 4 quarters × 4 regions at 800 × 500 with `.group(|r| r.region.as_str())`:
      along a row through every bar, the dogfood `count_runs` rule
      (reimplemented in the test) counts exactly 16 runs.
- [x] Each region's bars have one Okabe-Ito colour, in first-seen region order,
      and there is one legend listing the four regions.
- [x] Byte-identical to a hand-built plot of 4 `Rect` layers using
      `x.sub_band(i, 4)` and `x.sub_band_width_px(4, 0.1)`.

### AC4: Stacked

- [x] The same data with `.stack(|r| r.region.as_str())`:
  - exactly 4 runs along a row near the baseline;
  - in Q1's band, each region's colour at the vertical middle of its segment;
  - each stack's top edge within 1 px of `y.eval(sum of Q1)`.
- [x] A negative value stacks downward from 0, separately from the positive ones
      (pixel check below the axis).
- [x] Two rows with the same (quarter, region) draw as one segment of their
      summed height.

### AC5: Golden

- [x] A golden, `gup_core/builder_bars`, 800 × 500: the grouped chart on the
      left half and the stacked chart on the right half, rendered as two
      `ImageTarget` renders composed side by side in the test, each with a
      title, `.grid(Axes::Y)` and a legend. The implementer reads it: 16 grouped
      bars with gaps, 4 stacks with all 4 colours, quarter labels under the
      bands, value ticks on y.

### AC6: Escape hatches

- [x] `layers_mut()` returns one layer per group or stack key, in first-seen
      order, or one layer for simple bars.
- [x] `pick(at)` at a bar's centre returns that row. A layer setter after the
      first render panics with GUP-462's message.

## Decisions

- **API**:

  ```rust
  pub fn bar<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> BarChart<T>;

  impl<T: Send + Sync + 'static> BarChart<T> {
      /// The category of each bar, on a band x axis, in first-seen order.
      pub fn x(self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self;
      /// The value, drawn from 0.
      pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn y_scale<S: PositionScale>(self, scale: S) -> Self;     // default Linear::new()
      pub fn padding(self, p: f64) -> Self;   // Band::padding; default Band::new()'s 0.2 / 0.1
      pub fn color(self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self;
      pub fn color_scale(self, scale: Categorical) -> Self;
      pub fn fill(self, color: Color) -> Self;
      /// Bars side by side within each band, one slot per key (first-seen order).
      pub fn group(self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self;
      /// Bars stacked within each band, bottom to top in first-seen key order.
      pub fn stack(self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self;
      pub fn opacity(self, opacity: f32) -> Self;
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self;
      // plot_setters!: title, x_title, y_title, theme, grid, legends
      pub fn layers_mut(&mut self) -> Vec<&mut Selection<T, Rect>>;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
      pub fn pick(&mut self, at: geom::Point) -> Option<&T>;
  }
  impl<T: Send + Sync + 'static> Chart for BarChart<T> { /* as GUP-462 */ }
  ```

- **x axis**: `bar` sets `plot.x(Band::new())` at construction and keeps the
  handle. `padding` replaces it with `plot.x(Band::new().padding(p))`, which is
  allowed until materialisation. There is no `x_scale`: bars need a band.
- **Baseline**: `y_end` is a plain closure through the plot's y scale. It
  returns `0.0` for simple and grouped bars, and the stack base for stacked
  bars. Being a column, the baseline joins the y domain fit, so 0 is always in
  the domain. Bars on a log y scale are out of scope.
- **Grouped**: the rows are partitioned by group key, moving rows, into k
  layers. Layer i sets x to `x.sub_band(i, k).encode_key(xkey)` and width to
  `x.sub_band_width_px(k, 0.1)`.
- **Stacked**: partition by stack key into k layers (bottom to top). Before
  creating them, for each layer and x key, sum that layer's finite values at
  that x key. Positive sums stack upward from 0 and negative sums downward from
  0, accumulated over the layers in order.
  - Each layer gets two tables, `HashMap<Arc<str>, (f64, f64)>` of (base, top)
    per x key, captured by its `y` (top) and `y_end` (base) closures.
  - Rows sharing (x, stack) keys all draw the summed bar.
  - A null value contributes nothing, and its row is not drawn (NaN top).
- **Colour**: an explicit `.color` or `.fill` wins. Otherwise grouped and
  stacked bars colour by their group or stack key through one shared
  `ScaleRef<Categorical>` (default Okabe-Ito, or `color_scale`). Simple bars use
  `Rect`'s default fill.
- **Exclusive**: `group` and `stack` are exclusive (the last call wins), and so
  are `color` and `fill`.
- **Dropped from the old builder**: `orient`/`horizontal` (vertical only;
  horizontal bars are a follow-up), `bar_width` (the band decides), `stroke`
  (use `layers_mut()`), and `gap` (use `padding`).

## Escalation Triggers

- Any AC2–AC3 byte-identity fails.
- The `y_end` zero column does not bring 0 into an automatic y domain.
- A sub-band chain loses the band axis's tick labels (GUP-461's trigger,
  surfacing here).

## Out of Scope

- Horizontal bars; value labels on bars (no text mark yet); diverging-stack
  offsets other than "positives up, negatives down".
- Dogfood task 2 itself: GUP-465.

## Technical Tasks

- [x] `builder/bar.rs`: `bar`, `BarChart`, simple, grouped and stacked
      materialisation, `Chart` impl.
- [x] Stack tables and their unit tests (signs, duplicates, nulls).
- [x] Integration tests and golden (AC2–AC6).

## Dependencies

### Prerequisite Stories

- GUP-462: RFC-001 S10c Chart Builders and Scatter 📋 — the pattern.
- GUP-461: RFC-001 S10b Offset and Sub-Bands 📋 — grouped placement.
- GUP-463: RFC-001 S10d Line Builder 📋 — `Plot::layers_mut` and the series
  split it adds.

### Enables Stories

- GUP-465: dogfood task 2.

## Testing Strategy

- **Unit**: stack tables.
- **Integration**: byte-identity, run counts, colours per segment, sign handling
  (`tests/builders.rs`).
- **Visual**: `gup_core/builder_bars.png`, read by the implementer.

## Success Metrics

- [x] Dogfood task 2's grouped and stacked charts need no workaround.

## Risk Assessment

- **Low**: one layer per group or stack key means k pipelines' worth of draws.
  _Mitigation_: same glue, so one pipeline; k draws is trivial at bar-chart
  sizes.

## Definition of Done

- [x] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: `gup_core/builder_bars.png`
- [x] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

`gup_core::bar` and `BarChart<T>` in `crates/gup-core/src/builder/bar.rs`,
exported from the crate root. Simple bars are one `Rect` layer (x through
`Band::encode_key`, width `band_width_px()`, `y_end` a zero column); `group`
makes one layer per key using `sub_band(i, k)` and `sub_band_width_px(k, 0.1)`;
`stack` makes one layer per key with `(base, top)` tables from `stack_tables`
(positives up, negatives down, per-category sums). All layers share one band
domain (first-seen order over all rows) and one colour `ScaleRef`. Tests: 2 unit
(stack tables) and 11 in `tests/builders.rs` (`mod bars`), plus the doctest;
golden `tests/golden/gup_core/builder_bars.png` (800 x 500, grouped left,
stacked right).

Evidence: gup-core tests all pass; `mask old-path-loc` 28,910 (unchanged);
`mask wasm-size` scatter 320,243 (budget 325,467, pass); `mask wasm-browser`
PASS; `mask perf-budget` every check passed (uptime load 2.10, 2.96, 3.11).
Golden read: 16 grouped bars with gaps between bands, four stacks with all four
colours, baseline flush on the axis, legend and quarter labels present; zoomed
crops of the baseline and of the segment joins show no gap and no floating bar.

## Retrospective

**Completed**: 2026-10-19

### Key Technical Learnings

#### Shared band domain across layers

- **Challenge**: each group or stack layer sees only its own rows, so a layer
  missing a category could reorder the band axis.
- **Solution**: the builder computes first-seen categories over all rows and
  sets that domain on every layer's x encoding.
- **Pattern**: when one scale serves several layers, fix its dictionary up
  front.

### Architectural Decisions

#### The value accessor is held as `Fn(&T) -> f64`

- **Decision**: `y` stores `to_f64`, so the stack tables can sum it on the CPU.
- **Reasoning**: stacking needs the numbers before upload; same trade as
  scatter's `color_value`.
- **Trade-off**: a non-f64 `D` is uploaded as f64-converted f32.

### Development Workflow Insights

- Grouped bars have a 10 percent slot padding (Decision), so the gap between
  neighbours in a group is about 1 to 2 px at 400 px wide. It reads fine, but a
  `group_padding` setter would be a cheap follow-up if wanted.
- Rows sharing (x, key) overdraw the same summed bar; with opacity below 1 they
  would look darker. Not handled.
- Tick marks sit at the band centre, so pixel checks on the axis must step off
  the centre column.

### Follow-up Stories

None.
