---
implementer: sonnet
implementer-reason:
  the scales already map both ways (`eval`, `invert`); the home, names, and edge
  behaviour are decided below
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/chart.rs,
    crates/gup-core/src/builder,
    docs/tour,
    docs/tutorials/04_interactions.md,
    CHANGELOG.md,
  ]
---

# GUP-685: Data To Pixel And Back

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete (2026-11-08) **Created**: 2026-11-08

## Context

The feature tour (chapter 8, `docs/tour/snippets/08_interaction.rs`, region
`pixel`) opens with a hand-written `pixel(layout, x, y)` that interpolates
`Layout::x_ticks` and `y_ticks` to find where a data point is drawn. Tutorial 4
(`docs/tutorials/04_interactions.md`) has the same helper three times, plus an
inline copy for the Kea. The library has no public way to ask. Tests do the same
by hand (`tests/events.rs`, `append.rs`, `window.rs` and others use `x_ticks`).

The helper is also wrong beyond linear scales: interpolating between the first
and last tick is exact only for a linear scale. A log, symlog, pow or time axis
needs the scale's own map.

Assumption checked: `Layout` holds only tick pairs, no scale, so a method on
`Layout` cannot be correct for a log axis without snapshotting the scales into a
public struct (which `Layout`'s `PartialEq` and public fields make awkward).
`Plot` holds the scales as `Box<dyn DynPositionScale>` (`x`, `y`, `y_right`),
whose `eval(f64) -> f64` (data to pixel) and `invert(px) -> f64` are what the
zoom code uses, and whose range is set at each resolve. `Chart::layout()` is the
only chart-level accessor today, so a chart held as `impl Chart` (the tour)
needs the methods on the `Chart` trait, which has default-method precedent
(`describe`, `focus_text`, `data_table`).

## Read First

- `crates/gup-core/src/scale/mod.rs`: `DynPositionScale` (lines 403 to 450),
  `PositionScale::invert` (line 134). `scale/band.rs` lines 55 to 80 (`eval`
  takes a code; `invert` rounds to the nearest code and clamps).
- `crates/gup-core/src/chart.rs`: the `Chart` trait (line 55 on) and how
  builders forward (`impl Chart for ScatterChart`, `builder/scatter.rs` line
  358, `pick_row`).
- `crates/gup-core/src/plot.rs`: `Plot::pick_row` and `Plot::layout`
  (about 1658) for where `Plot` methods and doc tests sit.
- `docs/tour/snippets/08_interaction.rs` (the `pixel` region and its callers,
  lines 36 to 105) and `docs/tour/08_interaction.md`.
- `docs/tutorials/04_interactions.md` lines 60 to 80, 135 to 145, 370 to 380,
  425 to 435.

## User Story

> "As a developer driving or annotating a chart, I want to ask where a data
> point is drawn and what data a pixel stands for, so that I do not rebuild the
> axis maths from tick marks."

## Acceptance Criteria

### AC1: The API

- [x] `Chart` gains two default methods (signatures under Decisions); the
      defaults return `None`. `Plot` implements them, and every builder's
      `Chart` impl forwards to its plot (all eight impls found by
      `grep -rn "impl.*Chart for" crates/gup-core/src`).
- [x] `Plot` also has `pixel_right` and `value_right` for the secondary y axis
      (`None` without one).
- [x] Both return `None` before a successful resolve. After one they use the
      scales' ranges of that resolve.

### AC2: Behaviour per scale

- [x] Round trip, one test each, tolerance 0.01 px for pixels and a relative
      1e-9 for values: `value(pixel(x, y)) == (x, y)` for linear, log, symlog,
      pow and time x axes (time values are epoch seconds as the scale defines;
      follow whatever `Time::eval` takes).
- [x] Band or point x: `x` is the band code (the index of the key, as
      `PositionScale::key_code` gives), `pixel` returns the band centre, and
      `value` returns the nearest code. Test with three keys: `pixel` of codes
      0, 1, 2 equals the centres of the tick marks in `layout.x_ticks`, and
      `value` of a point between centres returns the nearer code.
- [x] Secondary y: `pixel_right(x, y)` on a plot with `y_right(Linear)` equals
      the tick position for a tick value of `layout.y_right_ticks`.
- [x] Outside the plot: a value beyond the domain (or a pixel outside the plot
      rect) is extrapolated, not clamped, for numeric scales (so the result can
      be outside `layout.plot`); a band `value` clamps to the first or last code
      (that is `invert`'s behaviour). A value the scale cannot map (log of 0 or
      a negative, NaN, a null band code) gives `None` from `pixel`.
- [x] Agreement with the old helper: on the tour's linear scatter, `pixel`
      equals the tick-interpolation within 0.01 px.

### AC3: The tour and the tutorial use it

- [x] `docs/tour/snippets/08_interaction.rs` drops the `pixel` helper and its
      `tour:begin pixel` region; callers use `chart.pixel(x, y).unwrap()` (the
      chart is resolved when it is called; keep the existing ordering). Chapter
      8's prose in `docs/tour/08_interaction.md` is updated where it quotes the
      helper (grep `pixel` in that file).
- [x] Tutorial 4's three `pixel` helpers and the inline Kea interpolation are
      replaced; its snippets compile (they are run by the tutorial test; find it
      with `grep -rn "04_interactions" crates tests`).
- [x] No golden changes: `git status` after the suite shows no `.png` modified.
      If one changes, a position moved by more than the old helper's error: read
      it and report.
- [x] Search for other hand interpolations
      (`grep -rn "x_ticks\[0\]\|along\b"     crates tests docs examples`).
      Replace those in docs and examples; leave tests that assert on
      `Layout::x_ticks` itself.

### AC4: Public API and docs

- [x] A doctest on `Chart::pixel` and on `Chart::value` (external-crate path,
      `use gup_core::prelude::*`), and a doctest on `Plot::pixel_right`.
- [x] CHANGELOG `[0.1.0]` Added line.

## Decisions

- **Home: the `Chart` trait, with `Plot` implementing it**, not `Layout`: the
  scale is not in `Layout`, and a snapshot would duplicate every scale type.
  Because the scale is live, a call after a pan but before the next `resolve`
  uses the new domain but the old range; the doc says "as of the last resolve;
  resolve first". Not worth a snapshot.
- Signatures:

  ```rust
  // on Chart, default None
  fn pixel(&self, x: f64, y: f64) -> Option<Point> { None }
  fn value(&self, at: Point) -> Option<(f64, f64)> { None }
  // on Plot, inherent, secondary y axis
  pub fn pixel_right(&self, x: f64, y: f64) -> Option<Point>;
  pub fn value_right(&self, at: Point) -> Option<(f64, f64)>;
  ```

  `Point` is `gup_core::geom::Point` (f32), already in the prelude. Names are
  `pixel` and `value` after the tour's helper and `PositionScale::invert`.

- **`pixel` takes data values as `f64`**, as every scale's `eval` does; a band
  key is its code. No `&str` overload: `key_code` gives the code, and a keyed
  overload is a later story if wanted.
- **No clamping for numeric scales**, so a caller can ask where a point outside
  the view would be (an off-screen annotation). `None` only where the scale
  yields NaN.
- **`value` is the scales' `invert`**; for the horizontal bar variant
  (`BarChart<T, Horizontal>`) the x and y scales are whatever the plot's x and y
  are (bars swap which holds categories), so no special case.
- Not added: `Layout` methods, a `Projection` type, batch variants.

## Escalation Triggers

- `DynPositionScale::eval` is not the scale's data-to-pixel map for some scale
  (for example it returns a unit-interval value, or the time scale takes a
  different unit from what its ticks report).
- A builder's `Chart` impl does not hold a `Plot` it can forward to.
- The round-trip fails for `Pow` or `Symlog` beyond the stated tolerance (a
  scale `invert` bug is its own story; report which).
- A tour or tutorial golden moves.
- Adding methods to `Chart` breaks a downstream impl in this repository
  (examples, tests, `crates/*`): defaults should prevent it.

## Out of Scope

- Keyed overloads, batch conversion, a `Projection` snapshot type.
- Replacing `Layout::x_ticks` or changing it.
- Fixing `invert` behaviour of any scale.

## Technical Tasks

- [x] `Chart::pixel` and `Chart::value`; `Plot` impl and the `_right` pair;
      forwarding in every builder.
- [x] Tests per AC2 in a new `crates/gup-core/tests/pixel_value.rs`.
- [x] Tour chapter 8 snippet and prose; tutorial 4.
- [x] Doctests; CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-669: Feature Tour ✅ — the helper this removes.

## Testing Strategy

- **Unit tests**: round trips per scale; band; secondary axis; outside the plot;
  `None` cases.
- **Integration tests**: the tour and tutorial snippet tests pass unchanged.
- **Visual validation**: none; goldens must not change.
- **Performance**: not applicable.

## Success Metrics

- [x] Neither the tour nor tutorial 4 contains a hand-written axis helper.
- [x] Round trips pass on five numeric scales and the band scale.

## Risk Assessment

- **Low**: a trait method on a public trait. _Mitigation_: default bodies.
- **Medium**: scale `eval` conventions differ (time units, band offsets).
  _Mitigation_: per-scale tests against the tick positions.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, no golden changes (AC3)
- [x] `mask wasm-browser`: not needed, no layout, input or wasm code changes
- [x] WASM size: `mask wasm-size` scatter gz bytes (the trait gains two methods,
      which dead-code elimination should drop; report)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Chart::pixel` and `Chart::value` (default `None`); `Plot` has inherent
  `pixel`, `value`, `pixel_right`, `value_right` and implements the trait; the
  seven builders forward (`Waiting` in `app.rs` is a test impl and keeps the
  defaults). `None` before a resolve; `pixel` is `None` when a scale gives a
  non-finite value.
- Tour chapter 8: the 10-line `pixel` helper region is gone; callers are
  `chart.pixel(x, y).unwrap()`. Snippet function names and signatures are
  unchanged. Tutorial 4: three helpers and the inline Kea interpolation removed.
  Two doc examples in `plot.rs` that interpolated ticks now use `plot.pixel`.
- `crates/gup-core/tests/pixel_value.rs`: 10 tests (linear, log, symlog, pow,
  time round trips; band; right axis; outside and `None`; trait forwarding;
  agreement with tick interpolation). No golden changed.
- Deviation: the relative 1e-9 on the value cannot hold through an `f32`
  `Point`; the tests check `value` then `pixel` to 0.01 px, and the data to a
  thousandth of what one pixel spans.
- WASM scatter gz: 378318 B on main (96ffe3d7), 378450 B on the branch (+132).
- Note: `Plot::invert` already existed (None outside the plot rect); `value`
  overlaps it and differs only by not clamping to the rect.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Pixels are f32

- **Challenge**: the AC asked for 1e-9 relative value round trips.
- **Solution**: test the pixel round trip at 0.01 px, and the value against one
  pixel's span.
- **Pattern**: a round trip through `Point` is bounded by `f32`, not the scale.

### Architectural Decisions

#### Methods on `Chart`, not `Layout`

- **Decision**: as the story set; `Plot` reads its live scales.
- **Trade-off**: between a pan and the next resolve, the range is stale.
- **Future**: `Plot::invert` could become a call to `value` plus a rect test.

### Development Workflow Insights

- Every scale round trip passed first time; the scales' `eval` and `invert`
  already agreed.

### Follow-up Stories

None written. Candidate (small): fold `Plot::invert` into `Plot::value`.
