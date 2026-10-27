---
implementer: sonnet
implementer-reason:
  two layout rules with the rule, signatures and the affected goldens fixed in
  Decisions; follows the existing nice and range code
touches: [crates/gup-core/src/plot.rs, crates/gup-core/src/builder/]
---

# GUP-561: Axis Ends Fit Bars and Bands

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress Planned **Created**: 2026-10-26

## Context

Two layout faults show on every bar and heatmap chart.

**Bars run past the last tick.** An automatic domain is made nice with a 10-tick
step (`Linear::nice`, `scale/linear.rs` lines 183–195), but the axis then draws
about `width / x_tick_spacing` ticks (`plot.rs` lines 1929–1936), a coarser
step. In `tests/golden/gup_core/builder_barh.png` the domain is 0–90 and the
ticks 0, 20 … 80, so the longest bar (about 90) ends past the last labelled tick
at the plot's edge. In `bar_value_labels_stacked.png` the domain is 0–65 with
ticks every 10, and South (61) ends past the last tick, 60. A scatter has the
same 10-step domain, but its domain is then widened by the marks' overhang, so
its ends are not ticks anyway; a bar's value axis has no overhang, and its far
end is the bar end.

**Band axes overrun the last band.** Every axis's far end (right, top) is inset
by `Theme::plot_padding` (4 px) unless widened (`ranges` in `plot.rs` lines
2068–2076), while the near end is not. The padding keeps overhanging marks off a
numeric axis line; a band or point scale makes its own room with its outer
padding. On a heatmap (`Band::padding(0.0)`) the axis lines run 4 px past the
last column and above the top row (`tests/golden/gup_core/heatmap.png`).

## Read First

- `crates/gup-core/src/plot.rs`: step 1–2, fitting and nice (lines 1862–1886);
  tick counts (lines 1929–1936); `ranges`/`set_ranges` and widening (lines
  2061–2151); thinning and the one relayout (lines 2212–2265); axis lines (lines
  2329–2350).
- `crates/gup-core/src/scale/linear.rs`: `nice` and `ticks` (lines 183–215),
  `tick_unit`, `nearest_125`.
- `crates/gup-core/src/scale/mod.rs`: `PositionScale::nice` (line 135),
  `DynPositionScale` (lines 360–400) and its `ScaleRef` impl of `nice` (line
  451), `tick_step` and `step_value` (lines 541–570).
- `crates/gup-core/src/builder/bar.rs` `BarChart::new` (lines 340–360);
  `crates/gup-core/src/builder/histogram.rs` around line 79.

## User Story

> "As a reader of a bar chart, I want every bar to end at or inside a labelled
> tick and the axis lines to end with the bands, so that I can read values off
> the axis and the chart looks finished."

## Acceptance Criteria

### AC1: `nice_to` rounds to the ticks' own step

- [x] Unit test in `scale/linear.rs`: for 10,000 seeded random extents (`lo` in
      −1000..1000, span in 1e-3..1e6, including `lo = 0`) and every count
      2..=12, after `nice_to(count)` the domain contains the extent, and
      `ticks(count)`'s first value equals the domain's start and its last value
      the domain's end (relative error ≤ 1e-9).
- [x] With a `tick_unit` set, the same holds (ticks on multiples of the unit).
- [x] A domain of `(0, 0)` gets the same padding `nice` gives it.

### AC2: Bars end inside the last tick

- [x] New test file `crates/gup-core/tests/bar_axis_ends.rs`: for vertical
      simple, grouped (with a negative value), stacked, horizontal simple and
      horizontal stacked bar charts and a histogram, with values from a seeded
      generator (largest bar 1..1000), at every size `w` in 240..=800 step 80
      and `h` in 160..=560 step 80: the largest bar end is ≤ the last value tick
      (`Layout::y_ticks`, or `x_ticks` for `barh`), the most negative is ≥ the
      first, and 0 is a tick. Zero failures.
- [x] The same charts with `.value_labels()` hold too (their axis is then
      widened past the rounded end, which keeps the bars inside it).

### AC3: Band and point axes have no far padding

- [x] New test in `tests/heatmap_png.rs` (or `bar_axis_ends.rs`): for a heatmap
      at 640×400, from `Layout`: the last x band's far edge (last x tick + half
      the spacing between the last two x ticks) equals the plot rect's right
      edge within 0.5 px, and the top y band's top edge equals the plot rect's
      top within 0.5 px.
- [x] `tests/golden/gup_core/heatmap.png` re-blessed and read: the x axis line
      ends at the right edge of hour 23's column and the y axis line at the top
      of Monday's row, with no 4 px stub.
- [x] A point-scale axis (`tests/ordinal_png.rs`) keeps its marks off the plot
      edge (its existing assertions pass; update only a number that encodes the
      4 px far pad, and list each one in the Implementation Summary).

### AC4: The goldens that change, and only those

- [x] Re-bless with `GUP_BLESS=1`. Only goldens whose chart has a `Band` or
      `Point` axis, or is a bar or histogram builder chart, may change. Expected
      among them: `builder_bars`, `builder_barh`, `builder_owned_keys`,
      `bar_value_labels`, `bar_value_labels_stacked`, `grouped_bars`,
      `rect_bars`, `histogram`, `heatmap`, `heatmap_diverging`, `band_months`,
      and `tutorials/01_grouped_bars`. Any other golden changing is an
      escalation.
- [x] Read each changed PNG and record one line per image in the Implementation
      Summary. Required: `builder_barh.png`'s last x tick is 100 and the New
      Zealand bar ends before it; `bar_value_labels_stacked.png`'s last tick is
      70 and South ends before it; `histogram.png`'s count axis ends on a
      labelled tick (by the rule, 300 for its tallest bin of about 242 at count
      8: 260 rounds with step 20, whose span asks for step 50); every bar lies
      inside the plot area.
- [x] Tests that assert a band position or an axis line end numerically are
      updated only where the number encodes the removed 4 px; list them.

### AC5: Browser scenes and dogfood

- [x] `mask wasm-browser` passes (its scatter page has a `Band` axis).
- [x] `./dogfood/run_all.sh` passes with every check's status unchanged
      (`t2_bars` is a bar chart).

## Decisions

- **Rule 1, rounding a value axis to its ticks.** The value axis of every bar
  and histogram builder chart, when automatic, is rounded out to multiples of
  the step its own ticks use, not the 10-tick step. Rejected: (a) extending the
  axis line and ticks past the domain, because a tick outside the domain lies
  outside the plot rect and its clip; (b) applying the rule to every automatic
  axis, because scatter and line axes are widened past their rounded ends for
  overhang anyway, and it would change most goldens for no visible gain.
- **API** (all new items):

  ```rust
  // scale/mod.rs, public, defaulted (Pow, Log, Symlog, Time keep `nice`):
  pub trait PositionScale {
      /// Extend the domain to multiples of the step `ticks(count)` uses, so
      /// both ends are ticks. Defaults to `nice`.
      fn nice_to(&mut self, count: usize) { self.nice() }
  }
  // scale/mod.rs, crate-private, with a `ScaleRef` impl like `nice`'s:
  trait DynPositionScale { fn nice_to(&self, count: usize); }
  // scale/linear.rs, private: the step `ticks` uses (tick_step, then the
  // tick_unit rounding); `ticks` and `nice_to` both call it.
  impl Linear { fn step(&self, lo: f64, hi: f64, count: usize) -> f64; }
  // plot.rs, crate-private:
  impl Plot { pub(crate) fn round_to_ticks(&mut self, axis: usize) -> &mut Self; }
  ```

  `Plot` gains `tick_ends: [bool; 3]` (default all false), set by
  `round_to_ticks` (which bumps `revision`).

- **`Linear::nice_to(count)`**: pad a zero-width domain as `nice` does; then up
  to 4 passes of `step = self.step(lo, hi, count)`,
  `lo = step_value((lo / step + 1e-9).floor(), step)`,
  `hi = step_value((hi / step - 1e-9).ceil(), step)`, stopping when a pass
  changes nothing. The `1e-9` guards keep a domain already on a multiple from
  growing by one step through rounding error (the same guards as `ticks`).
- **Resolve order**: move the tick-count computation (`y_count`, `counts`) above
  step 1–2 (it reads only `width`, `height` and the theme). In step 1–2, an
  automatic axis `i` with `tick_ends[i]` calls `nice_to(count_i)` instead of
  `nice()`. In the thinning branch (`if counts != before`), before
  `layout(counts)`, each automatic `tick_ends` axis whose count changed calls
  `nice_to` with its new count; this only ever grows the domain, so widening's
  room is kept.
- **Who sets it**: `BarChart::new` calls `plot.round_to_ticks(1)` for vertical
  and `round_to_ticks(0)` for horizontal bars; the histogram builder calls
  `round_to_ticks(1)`. It is per plot axis, so it survives `y_scale(..)`/
  `x_scale(..)`. GUP-547's area builder should call it too (noted there when it
  is written; not this story's job).
- **Rule 2, no far padding on key axes.** In `ranges`, an axis whose scale
  `reads_keys()` (`Band`, `Point`) uses `far = insets[i].1` without `pad`, the
  same as its near end. Band and point scales are always automatic, so the axis
  line code needs no change. Update `Theme::plot_padding`'s doc to "Inset of a
  numeric axis's far end …".

## Escalation Triggers

- AC1 finds an extent where 4 passes do not reach a fixpoint.
- AC2 fails for some size even after the thinning re-round (thinning twice can
  in principle change the step again): report the failing cases; do not add more
  relayout passes.
- A golden outside AC4's rule changes, or a bar or band moves by more than a
  re-rounded domain or 4 px explains.
- `mask wasm-browser` fails on a pixel check.
- Moving the tick-count computation changes any non-bar golden.

## Out of Scope

- Bar value labels' format (GUP-560).
- Integer tick steps for integer data (GUP-562).
- The area builder (GUP-547).

## Technical Tasks

- [ ] `PositionScale::nice_to`, `DynPositionScale::nice_to`, `Linear::step` and
      `Linear::nice_to`, with AC1's tests.
- [ ] `Plot::tick_ends`/`round_to_ticks`; move the counts; call `nice_to` in
      step 1–2 and after thinning.
- [ ] Bar and histogram builders call `round_to_ticks`.
- [ ] Drop `pad` from key axes' far end; doc line on `plot_padding`.
- [ ] `tests/bar_axis_ends.rs` (AC2), the heatmap test (AC3).
- [ ] Re-bless and read the goldens; browser and dogfood runs.

## Dependencies

### Prerequisite Stories

- GUP-543: Horizontal Bars and Group Padding ✅ — `barh`.
- GUP-548: Histogram Builder ✅ — the histogram's count axis and `tick_unit`.
- GUP-549: Heatmap Builder ✅ — the heatmap golden.

### Enables Stories

- GUP-562: Integer Tick Steps for Integer Data — reuses `Linear::step`.
- GUP-536: Tutorial Tour of Chart Types — its bar and heatmap images are taken
  after this.

## Testing Strategy

- **Unit tests**: AC1 in `scale/linear.rs`.
- **Integration tests**: AC2 sweep, AC3 heatmap geometry.
- **Visual validation**: AC4 goldens, each read.
- **Browser**: `mask wasm-browser` (AC5).

## Success Metrics

- [ ] Zero AC2 failures over 48 sizes × 6 chart kinds.
- [ ] Heatmap axis lines flush with the cells.

## Risk Assessment

- **Medium**: golden churn across bar and band charts. _Mitigation_: AC4's rule
  bounds which images may change, and each is read.
- **Low**: the larger value domain shortens bars slightly (90 of 100 instead of
  90 of 90), and the fixpoint can overshoot by one coarse step (the histogram's
  242 reaches 300, not 250, because 0–250 asks for step 20 again and would end
  between ticks). _Mitigation_: that is the intended trade; the bar now ends
  before a labelled tick. Do not try to find a tighter end: escalate instead.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`, plus
      `scripts/with_cargo_env.sh cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (AC4's goldens)
- [ ] `mask wasm-browser` (layout change)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Implementation Summary

Added `PositionScale::nice_to` (default `nice`), `DynPositionScale::nice_to`,
`Linear::step` and `Linear::nice_to`; `Plot::tick_ends`/`round_to_ticks`; tick
counts moved above step 1-2; bar and histogram builders call `round_to_ticks`;
key axes lose the far `plot_padding` (`ranges`); `plot_padding` doc updated.
Tests: `scale/linear.rs` (AC1, 10,000 extents x 11 counts, with and without
`tick_unit`), `tests/bar_axis_ends.rs` (576 cases: 48 sizes x 6 kinds x labels
on/off, zero failures), `heatmap_png.rs` (flush axes within 0.5 px). No existing
numeric assertion needed changing (no test encoded the 4 px).

Deviation: widening (value labels) can change the tick step after the domain was
rounded, so bars ended past the last tick (barh 240x400 with labels: 748 vs last
tick 500). `round_out` runs `nice_to` again right after widening and after
thinning for tick-ending axes; it only grows the domain, adds no relayout pass.
Areas (`gup::area`) are unchanged: the story leaves them out of scope.

Goldens re-blessed (each read):

- `builder_barh`: last tick 100, New Zealand (about 90) ends before it.
- `bar_value_labels_stacked`: axis 0-70, South (61) ends before 70.
- `histogram`: count axis 0-300, tallest bin (242) below the 300 tick.
- `heatmap`, `heatmap_diverging`: axis lines flush with last column and top row.
- `bar_value_labels`: axis -20..60, labels clear.
- `builder_bars`: grouped 0-55, stacked 0-180, bars on the baseline.
- `builder_owned_keys`, `grouped_bars`, `rect_bars`: value axis ends on 60.
- `band_months`: Dec band ends flush with the plot edge.
- `tutorials/01_grouped_bars`: axis ends on 7; Q3 South ends exactly at 7.

Results: `mask merge-check --dogfood` PASS (incl. wasm-browser, dogfood);
`cargo test --doc -p gup` 54 passed; wasm scatter gz 344098 B (budget 341760
+2%, pass). `perf_budget --skip-wasm`: exact rows unchanged; first run
`zoom.cpu.median_ms` 0.839 vs 0.828 (noise), second run clean.

## Retrospective

**Completed**: 2026-10-27

### Key Technical Learnings

#### Widening changes the tick step

- **Challenge**: rounding to the ticks' step before widening is not enough;
  label headroom widens the domain, and `tick_step` of the wider span can pick a
  coarser step whose last tick lies below the bars.
- **Solution**: re-run `nice_to` after widening (grow only).
- **Pattern**: any later domain growth must re-round for tick-ending axes.

### Architectural Decisions

#### Area charts not rounded

- **Decision**: left unchanged (story says out of scope).
- **Future**: `gup::area` can call `round_to_ticks(1)`; GUP-562 reuses
  `Linear::step`.

### Follow-up Stories

None.
