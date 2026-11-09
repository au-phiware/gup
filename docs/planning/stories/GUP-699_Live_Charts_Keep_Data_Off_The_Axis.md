---
implementer: sonnet
implementer-reason:
  the design is settled in Decisions (one scale flag, three read-back methods
  that follow GUP-685's forwarding); the behaviours to prove already exist and
  the story pins them with tests
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/chart.rs,
    crates/gup-core/src/scale/linear.rs,
    crates/gup-core/src/builder,
    docs/tutorials/05_streaming_data.md,
    CHANGELOG.md,
  ]
---

# GUP-699: Live Charts Keep Their Data Off The Axis And Refit Without A Rebuild

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1)
**Status**: 🚧 In Progress **Created**: 2026-11-09

## Context

An outside user (a Game of Life app drawing gup charts into its own wgpu 27
renderer, gup pinned at `d4a66ab4`) drew a live `gup::line` with `Appender` and
`Window::LastRows`. Once the population settled near 500 the line sat on the x
axis, because the fitted y domain starts at the data minimum (the axis ran about
500 to 5000). There is no way to ask for zero. The user worked around it by
rebuilding the chart with `.y_scale(Linear::new().domain(0.0, peak * 1.25))` and
re-feeding 1,000 rows whenever the peak moved. A second report: the builders
cannot say what the current view is, nor whether the user has moved it, so the
host watched `Response.redraw` to avoid throwing away the user's zoom when it
rebuilt.

Checked against `main` (2026-11-09), the reports are partly right:

- **Right**: `Linear` has no include-zero option (`scale/linear.rs`:
  `Linear::new`, `domain`, `range`, `tick_unit` only). A line's y axis is fitted
  with `AxisFit::Nice` (`plot.rs` line 593, applied near line 2785), which
  rounds outward to the 10-tick step, so a minimum of 500 stays 500.
- **Wrong, "scales can't be changed after build"**: `Plot::y(scale)` (line 889)
  swaps the y scale at any time, and closures on y re-bind at the next resolve
  (`axes_generation`). `LineChart::plot_mut().y(..)` reaches it. The rebuild and
  re-feed were not needed even without this story.
- **Wrong, "refit needs a rebuild"**: an automatic scale refits on every
  resolve. `Linear::fit_domain` replaces the domain unless `explicit`, and
  `Window::LastRows` fits the retained chunks only, so an evicted peak leaves
  the domain. The user's rebuild existed only because their fix was an explicit
  `domain(0.0, ..)`, which stops refitting. After a zoom or pan the scale
  becomes explicit (`Plot::pan_from` and `zoom_axes` call `set_domain`), after
  `Plot::reset_view` (`Key::ResetView`) it is automatic again (`Home::Auto`
  calls `clear_domain`).
- **Partly wrong, "no padding"**: an automatic axis is already widened by the
  marks' overhang plus `Theme::plot_padding` (4 px) in `Scale::widen`
  (`scale/mod.rs` line 587, called near `plot.rs` line 3076), so a line is not
  exactly on the axis line. It is a few pixels clear of it, which reads as "on
  the axis" when the whole line sits at the minimum. Zero in the domain is the
  fix, not more padding.
- **Right, read-back**: `Plot::domains` is private; tests read the view by
  inverting the plot rect's edges (`tests/window.rs` `x_domain`,
  `tests/axis_extent.rs` `x_ends!`). `Plot::home` (is the view moved?) is
  private; only `Plot::reset_view` and `Key::ResetView` (which returns whether
  it moved) expose anything.

## Read First

- `crates/gup-core/src/scale/linear.rs`: lines 16 to 62 (`Linear`, `new`,
  `domain`), 127 to 138 (`fit_domain`), 165 to 178 (`clear_domain`,
  `set_domain`), 191 to 204 (`nice`).
- `crates/gup-core/src/plot.rs`: `AxisFit` (593), `fit_extent` (690), `domains`
  and `pan_from` and `record_home` and `reset_view` (1276 to 1370), the fit and
  nice step in `resolve` (2740 to 2800), `widen` and `kept` (3025 to 3100).
- `crates/gup-core/src/scale/mod.rs`: `widen` (587 to 605).
- `crates/gup-core/src/chart.rs`: the `Chart` trait, `pixel` and `value` (lines
  120 to 160): the pattern for default methods and how builders forward them
  (GUP-685; find the forwarding with `grep -rn "fn pixel" crates/gup-core/src`).
- `crates/gup-core/src/builder/histogram.rs` line 259:
  `HistogramChart::x_domain` is an existing builder setter (see Decisions for
  the name this forces).
- `crates/gup-core/tests/area_png.rs` lines 254 to 300 (`area_mark_golden`): the
  golden pattern to copy. `tests/window.rs` lines 262 to 300 (reading the view).
- `docs/tutorials/05_streaming_data.md`: the `Window::LastRows` section.

## User Story

> "As a developer with a live chart, I want to keep zero on its value axis and
> to ask what view the chart has and whether the user moved it, so that I never
> rebuild a chart to rescale it or to protect the user's zoom."

## Acceptance Criteria

### AC1: Measure the report's chart (no code change)

- [ ] A test builds the settled live line of AC5 without `include_zero`,
      resolves it at 640 by 400, and measures the gap in pixels between the
      line's lowest row (`Chart::pixel`) and the plot rect's bottom edge
      (`Layout::plot`). Assert the gap is positive and under 12 px, and record
      the number in the story's Implementation Summary. This pins the report's
      premise as it is: clear of the axis line, but only just.

### AC2: `Linear::include_zero`

- [ ] `Linear::include_zero(self) -> Self` exists (signature under Decisions).
      With an automatic domain the fitted extent grows to contain 0 before
      `nice`: rows with y from 500 to 5000 give a y view whose lower end is at
      most 0 and a tick at 0 in `Layout::y_ticks`; all-negative data gives an
      upper end of at least 0; a constant series (all 500) gives a non-empty
      domain.
- [ ] An explicit domain wins in either call order:
      `Linear::new().include_zero().domain(100.0, 200.0)` and
      `Linear::new().domain(100.0, 200.0).include_zero()` both give the view
      (100, 200) (a test for each).
- [ ] The flag survives zoom and `reset_view`: after `Key::ResetView` the y view
      again contains 0.
- [ ] Without `include_zero` nothing changes: `git status` after the full suite
      shows no `.png` or `.svg` modified.
- [ ] A doctest on `Linear::include_zero` (external-crate path,
      `use gup_core::prelude::*`) builds a line chart through
      `.y_scale(Linear::new().include_zero())` and asserts the y tick values
      start at 0.0.

### AC3: Read-back on the `Chart` trait

- [ ] `Chart` gains `x_view`, `y_view` and `view_is_home` as default methods
      (signatures under Decisions). `Plot` implements them and every builder's
      `Chart` impl forwards to its plot, exactly as GUP-685 forwarded `pixel`
      and `value` (all impls found by `grep -rn "impl.*Chart for"`).
- [ ] `Plot::y_right_view(&self) -> Option<(f64, f64)>` exists (`None` without a
      right axis).
- [ ] Before the first successful resolve `x_view` and `y_view` are `None`.
      After one they equal the values `Chart::value` gives at the plot rect's
      left and right edges (x) and bottom and top edges (y), within `1e-6` of
      the span (a test).
- [ ] A band or point axis returns its codes (`0.0` to `len`), as `Chart::value`
      does; the doc says so.
- [ ] `view_is_home` is `true` before any input, `false` after a wheel zoom or a
      drag, and `true` again after `Key::ResetView` or `Plot::reset_view` (a
      test through `Chart::handle`).
- [ ] A doctest on `Chart::view_is_home` (external-crate path).

### AC4: Live refit while home, hold while moved

One test file, `crates/gup-core/tests/live_view.rs`, a line chart built with
`.window(RowWindow::LastRows(..))` and an `Appender` (use the spelling of the
name that is in the tree when you start: `Window` now, `RowWindow` once GUP-701
has landed):

- [ ] Home, rising: append rows with a larger peak, resolve; `y_view().1` grows
      to cover the peak and `view_is_home()` is `true`.
- [ ] Home, falling: append until the peak has left the window (more than
      `n + 2 * 1024` rows past it), resolve; `y_view().1` falls below the old
      peak. (The rebuild in the report was never needed for this.)
- [ ] Moved: after a wheel zoom through `Chart::handle`, append a larger peak
      and resolve; `y_view()` and `x_view()` are bit-identical to their values
      right after the zoom, and `view_is_home()` is `false`.
- [ ] Reset: after `Key::ResetView` and a resolve, `y_view()` covers the new
      peak again and `view_is_home()` is `true`.

### AC5: A golden of the settled live line

- [ ] A golden test `live_line_settled` (in `tests/live_view.rs`, harness and
      layout metadata as in `area_mark_golden`) draws one default line, 640 by
      400: 8,000 rows appended in 500-row batches with a resolve after each, y
      (a population) rising over rows 0 to 2,000 to a peak of 5,000 then
      settling by row 2,100 to 500 plus a small deterministic wobble (a sine of
      amplitude 20), window `LastRows(1024)`, a y scale of `Linear::new()` with
      `include_zero()`, title "Population".
- [ ] Read the blessed PNG (`Read` the golden file) and confirm by eye: the line
      is a wobbling band in the lower fifth of the plot well above the x axis;
      the y axis starts at 0 with a labelled tick `0`; the title and tick labels
      are present; nothing is drawn outside the plot rect. Say what you saw in
      the Implementation Summary.

### AC6: Docs

- [ ] Tutorial 5 gains a short subsection (compile-checked like the rest of the
      tutorial) showing `.y_scale(Linear::new().include_zero())` on a live line
      and `chart.view_is_home()`, with one sentence each on: the axis refits
      while home, holds after the user moves it, and `Key::ResetView` returns it
      to fitting.
- [ ] CHANGELOG `[0.1.0]` Added: `Linear::include_zero`, `Chart::x_view`,
      `y_view`, `view_is_home`, `Plot::y_right_view`.
- [ ] Search for places that fix a live chart's y domain by hand to get a zero
      baseline, with the command below, and report each. Replace one only where
      it is a live chart whose domain is fixed for that reason.

  ```bash
  grep -rn "domain(0.0" docs examples dogfood gup-egui gup-bevy crates/*/examples
  ```

## Decisions

- **The option lives on the scale**: `Linear::include_zero`, not a builder
  method (`.y_zero(true)`). Reason: every builder already takes `x_scale` and
  `y_scale`, and `Plot::y(..)` takes a scale, so one method serves all seven
  builders and the raw `Plot` API; a builder method would need an arm in
  `plot_setters!` and a second one for the x axis. It sits beside
  `Linear::domain`.
- Signature:

  ```rust
  impl Linear {
      /// Grow an automatic domain to contain 0 before it is made nice.
      /// An explicit domain wins.
      pub fn include_zero(mut self) -> Self;
  }
  ```

  Implemented as a `zero: bool` field read in `Linear::fit_domain`: when not
  `explicit` and `zero`, the extent becomes `(lo.min(0.0), hi.max(0.0))` before
  it is stored. `Linear` only: `Log` has no zero, `Time` has no meaningful one,
  and `Pow` and `Symlog` are not asked for.

- **No new padding.** The existing overhang plus `plot_padding` widening stays
  as it is; AC1 records its size. Headroom above the peak (the user's
  `peak * 1.25`) is not offered: `nice` already rounds up to the next tick, and
  a headroom option is a separate decision.
- **No new refit API.** Refit while home is existing behaviour (AC4 pins it).
  There is no `refit()`.
- **Read-back names** (on `Chart`, default bodies, `Plot` implements, builders
  forward):

  ```rust
  fn x_view(&self) -> Option<(f64, f64)> { None }
  fn y_view(&self) -> Option<(f64, f64)> { None }
  fn view_is_home(&self) -> bool { true }
  // on Plot, inherent
  pub fn y_right_view(&self) -> Option<(f64, f64)>;
  ```

  Not `x_domain`/`y_domain` as the report asked: `HistogramChart` already has an
  inherent `x_domain(self, lo, hi) -> Self` setter, and an inherent method
  shadows a trait method, so `histogram.x_domain()` would fail with an arity
  error. "View" matches `view_axes` and `reset_view`.

- **`x_view` is the visible domain**, `Plot::domains()[0]`, so widening and zoom
  are included, "as of the last resolve" (`None` before one). It is the scale's
  `current_domain`, not a re-derivation from the plot rect.
- **`view_is_home` is `self.home.is_none()`**, the existing record that a zoom
  or pan happened since the last reset. A view set by `Plot::x(..)` or an
  explicit domain at build time is home.
- **Where code goes**: `Linear` in `scale/linear.rs`; the three methods in
  `chart.rs`; `Plot`'s implementations in `plot.rs` beside `pixel`.

## Escalation Triggers

- `Linear::fit_domain` is not called with the union of every layer's extent (for
  example once per layer in a way that makes `include_zero` order-dependent).
- A moved view (AC4 "Moved") still changes after an append: the zoom does not
  make the scale explicit for a line chart, or `widened` or `kept` re-fits it.
- A windowed line's y view does not fall when the peak leaves the window (AC4
  "Home, falling"): eviction does not reach the scale's fit.
- With `include_zero` the golden shows an extra tick or label below 0, or the
  axis ends in a gap (widening past 0): decide with the owner before suppressing
  it.
- Any existing golden changes with the flag off.
- A downstream `Chart` impl in this repository stops compiling.

## Out of Scope

- Headroom or padding options, `include_zero` for `Log`, `Time`, `Pow`, `Symlog`
  or colour scales.
- A pixel-to-data or data-to-pixel change (GUP-685, GUP-692 own that).
- Renaming `Window` (GUP-701) or `Point` (GUP-700).
- Right-axis builders: `y_right_view` is inherent on `Plot` only.

## Technical Tasks

- [ ] AC1 measurement test, recorded in the summary.
- [ ] `Linear::zero` field, `include_zero`, the `fit_domain` change, doctest.
- [ ] `Chart::x_view`, `y_view`, `view_is_home`; `Plot` impls and
      `y_right_view`; forwarding in every builder; doctests.
- [ ] `tests/live_view.rs` (AC2 to AC5), bless the golden, read it.
- [ ] Tutorial 5 subsection; CHANGELOG; the `domain(0.0` search.

## Dependencies

### Prerequisite Stories

- GUP-685: Data To Pixel And Back ✅ — the forwarding pattern for `Chart`
  default methods.
- GUP-692: One Way From Pixel To Data ✅ — `Plot::value`, used to check the
  view.

### Enables Stories

- GUP-702: the embedding guide can point at `view_is_home` for hosts that
  rebuild charts.

## Testing Strategy

- **Unit tests**: `Linear` with the flag (grow, negative, constant, explicit
  wins in both orders).
- **Integration tests**: `tests/live_view.rs`: rising, falling, moved, reset.
- **Visual validation**: the `live_line_settled` golden, read by eye.
- **Performance**: none; the flag is read once per fit.

## Success Metrics

- [ ] A live line with `Linear::new().include_zero()` never needs rebuilding to
      rescale, and a host can tell whether the user moved the view without
      watching `Response.redraw`.
- [ ] No existing golden changes.

## Risk Assessment

- **Low**: a field on a public struct and three default trait methods.
  _Mitigation_: defaults, flag off by default.
- **Medium**: widening past 0 may leave a tick or gap below the axis start.
  _Mitigation_: the golden is read by eye; see Escalation Triggers.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the golden PNG read, AC5)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm code changes
      (the flag only changes a fitted domain when set; the scenes do not set it)
- [ ] WASM size: `mask wasm-size` scatter gz bytes (three default trait methods
      and a flag; report the change)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
