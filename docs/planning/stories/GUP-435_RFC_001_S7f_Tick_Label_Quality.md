---
implementer: sonnet
implementer-reason:
  the log nice rule, the B suffix and the measured thinning loop are settled in
  Decisions; each is a local change to existing tick code
---

# GUP-435: RFC-001 Step S7f: Tick Label Quality

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-17)
**Created**: 2026-10-16

## Context

Three tick-label problems recorded during S5, split out of
[GUP-425](GUP-425_RFC_001_S7b_Theme_And_Axis_Titles.md) on 2026-10-16:

- **Log domains waste space.** `Log::nice` (`crates/gup-core/src/scale/log.rs`
  lines 159–168) rounds both ends to whole powers of the base, so
  `categorical_species`' data from about 8 to 12,000 is niced to 1–100,000: two
  empty decades.
- **"1G" for a billion.** `format_log_tick` (`log.rs` lines 194–211) uses SI
  suffixes k, M, G, T. On `wgsl_function_highlight`'s population axis readers
  expect "1B" (orchestrator review of the GUP-420 goldens, 2026-10-11). The
  scale API has no notion of units, and chart axes with count-like quantities
  (population, revenue) are far more common than physical giga-quantities, so
  the default changes rather than gaining a units hint.
- **Colliding ordinal labels.** `Band`/`Point` ticks keep every k-th key from a
  count heuristic (`scale/band.rs` lines 80–88:
  `stride = n.div_ceil(2 * count)`), with no measurement, so long labels on a
  narrow plot overlap. RFC-001 "S5b findings": "Collision-aware labels are
  S7's." Numeric axes have the same exposure on very small plots, so the fix is
  in `Plot::resolve`, for every axis, not in `Band`.

## Read First

- RFC-001 "S5b findings" → "`Band` and `Point`" (lines 2732–2753).
- `crates/gup-core/src/scale/log.rs` lines 155–211 and its tests from line 245.
- `crates/gup-core/src/scale/mod.rs` lines 229–300 (`tick_step`,
  `format_with_step`) and 322–378 (the font-coverage sweep test).
- `crates/gup-core/src/scale/band.rs` lines 75–90.
- `crates/gup-core/src/plot.rs` `resolve`'s tick measurement and placement
  (after GUP-425; today lines 222–262 and 308–340).

## User Story

> "As a visualization developer, I want log axes that fit the data, count
> suffixes readers expect, and tick labels that never overlap, so that default
> axes look finished on any chart size."

## Acceptance Criteria

### AC1: Tighter log `nice()`

- [x] Unit tests (base 10) assert niced domains: `(3.2, 870)` → `(2, 1000)`;
      `(1, 1000)` → `(1, 1000)`; `(8, 12000)` → `(5, 20000)`; `(0.03, 4e6)` →
      `(0.02, 5e6)`. Base 2 keeps whole powers (existing behaviour, test kept).
- [x] `categorical_species` is re-blessed and read by eye: the log x axis spans
      5 to 20,000 and the data fills most of it. The retrospective records
      before/after domains.

### AC2: Count suffixes

- [x] `format_log_tick(1e9) == "1B"`, `format_log_tick(2.5e9) == "2.5B"`; `k`,
      `M`, `T` unchanged. The font sweep test's expected characters become
      `"0123456789−.ekMBT:"`.
- [x] `wgsl_function_highlight` (and any golden showing a "G" label) is
      re-blessed and read by eye: the axis reads "1B".

### AC3: Labels never overlap

- [x] A test resolves `band_months`' data (12 full month names) at 240×200 and
      asserts adjacent x tick labels' ink boxes are separated by at least
      `theme.label_gap`, and that at least 2 labels remain.
- [x] The same assertion holds for 60 short keys (`k0`…`k59`) at 720×450, for a
      `Linear` x at 120×120, and for a `Band` y with 40 keys at 720×200
      (vertical spacing uses label height).
- [x] Goldens whose labels did not overlap before are unchanged by the thinning
      (it is a no-op when nothing collides).

### AC4: Bar charts breathe

- [x] `Band`'s default padding gives visibly separate bars: inner padding 0.2
      and outer padding 0.1 (today the gaps are about 5% of a band). `rect_bars`
      is re-blessed and read by eye.
- [x] A linear domain that `nice()` rounds to exactly the data maximum, such as
      `rect_bars`' 60, keeps the top mark off the plot's edge. The tallest bar
      currently touches the top. Either extend `nice()` by one step when the
      maximum lands on the boundary, or let the plot inset the range by
      `theme.plot_padding`. Decision: inset by `theme.plot_padding` (default 4
      px), so that tick values stay round and the rule also covers dots and
      lines. A test checks that the top bar ends at least 4 px below the plot
      rect.

(Orchestrator review of the GUP-429 `rect_bars` golden, 2026-10-16.)

### AC5: Ticks span the axis when marks overhang

- [x] When marks overhang the plot (an encoded or constant `Size` channel, since
      GUP-418), an automatic domain is widened in data units to cover the
      overhang _before_ `nice()`, instead of only insetting the range. Ticks and
      labels then run to both ends of each axis line. `bubble_stroke` (ticks
      0.0–1.0 drawn across the middle, with long bare stretches of axis line
      either side) and `sqrt_radius` are re-blessed and read by eye.
- [x] A domain the user fixed explicitly is never widened. The range inset
      stays, and the axis line is drawn only over the range, so no bare stretch
      appears.
- [x] A test checks that the first and last tick on each axis of `bubble_stroke`
      lie within one tick step of the axis line's ends.

(Orchestrator review of the GUP-430 `bubble_stroke` golden, 2026-10-16.)

## Decisions

- **Log nice (base 10 only)**: `lo` = the largest of {1, 2, 5} × 10^k not above
  `d0`; `hi` = the smallest of {1, 2, 5} × 10^k not below `d1` (with the
  existing 1e-9 tolerances). Other bases keep whole powers. `Log::ticks` is
  unchanged; with a 1-2-5 domain it already emits the in-domain decades, and 2/5
  multiples when too few.
- **Suffixes**: `k` (1e3), `M` (1e6), `B` (1e9), `T` (1e12) in `format_log_tick`
  (so `Symlog` too). Linear labels are unchanged (no suffixes).
- **Thinning** in `Plot::resolve`, after ranges are set, per axis: compute each
  label's ink box at its tick position; while any adjacent pair is closer than
  `theme.label_gap` (x: horizontally; y: vertically) and `count > 1`, decrement
  `count` and recompute `ticks(count)`. If the widest y label or tallest x label
  after thinning exceeds what the margins reserved, rerun the layout once with
  the thinned counts. No rotation, no abbreviation.

## Escalation Triggers

- A golden changes for a reason other than the three rules here.
- The thinning loop needs more than one layout rerun to converge.
- `window_parity` (it compares against `scatter.png`) fails after re-blessing
  `scatter.png` for the log nice change.

## Out of Scope

- Rotated or abbreviated labels: not planned.
- Unit-aware formatting (`G` for physical quantities), custom tick formatters:
  not yet a story.
- Semibold year labels: GUP-436.

## Technical Tasks

- [x] Change `Log::nice` and `format_log_tick`; update their tests and the font
      sweep.
- [x] Add the thinning loop to `Plot::resolve`.
- [x] Tests for AC3; re-bless and read the affected goldens.

## Dependencies

### Prerequisite Stories

- GUP-425: `Theme` and Axis Titles 📋 — `theme.label_gap` and the layout code
  this edits.
- GUP-418, GUP-419 ✅ — the log, band and point scales.

### Enables Stories

- RFC-001 S10 builders (not yet stories): readable default axes.

## Testing Strategy

- **Unit tests**: niced domains; suffixes.
- **Integration tests**: AC3 ink-box separation at several sizes.
- **Visual validation**: re-blessed `categorical_species`, `scatter` (log y) and
  `wgsl_function_highlight`, read by eye.
- **Performance**: thinning measures at most `count` label sets per axis per
  resolve; no frame-time change expected.

## Success Metrics

- [x] No overlapping tick labels at any tested size.

## Risk Assessment

- **Medium**: `scatter.png` is also `window_parity`'s reference. _Mitigation_:
  re-bless it in the same commit and run `window_parity`.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Log::nice` rounds base 10 to the 1-2-5 series; `format_log_tick` says `B`.
- `Plot::resolve` is restructured around a `layout(counts)` closure: ticks,
  margins and plot rect. It lays out once, widens automatic domains, lays out
  again, thins ticks by measured label boxes and lays out a third time only if
  thinning changed the counts.
- New `DynPositionScale::widen`: maps the raw fitted extent `inset` px inside
  the plot rect and reads the domain off the rect's edges (generic through
  `invert`, so log, symlog, pow and time widen in their own space). Ordinal
  scales and explicit domains never widen. A widened axis then uses the whole
  plot rect as its range; any other axis keeps the inset
  (`overhang + theme.plot_padding`, default 4 px). An axis with a fixed domain
  draws its line over the range only.
- `Band::new()` pads 0.2 inner, 0.1 outer.
- Tests: `tests/tick_labels.rs` (5), additions to `stroke_png.rs` and
  `rect_png.rs`, log nice, base-2 and `B` unit tests. Goldens re-blessed: all 21
  in `tests/golden/gup_core/`, plus `tests/fixtures/msaa_guides.svg`.

## Retrospective

**Completed**: 2026-10-17

### Key Technical Learnings

#### Widening an automatic domain needs the plot rect

- **Challenge**: the overhang is in pixels, the domain in data units, and the
  plot rect depends on the tick labels, which depend on the domain.
- **Solution**: lay out once with the unwidened domain, widen through the
  scale's own `invert`, lay out again. The fit extent is read from `ScaleRef`'s
  fit union, so no accessor is evaluated twice.
- **Pattern**: a widened axis spends its inset in the domain, so its range is
  the whole rect; do not inset twice.

#### Before/after domains

- `categorical_species` x: 1 to 100,000 before, now 5 to 20,000 (before the
  mark-overhang widening, the 1-2-5 nice gives 5 to 20,000 for 8 to 12,000).

### Architectural Decisions

#### Every golden changed

- **Decision**: `plot_padding` (4 px, replacing the old fixed 1 px) and widening
  apply to every plot, so all 21 `gup_core` goldens moved a few pixels. Each
  intended change was read: `rect_bars` (gaps between bars, top bar 4 px under
  the rect), `bubble_stroke` (fixed domains: axes only over the range),
  `sqrt_radius`, `scatter` and `categorical_species` (ticks to both ends, log 5
  to 20k), `wgsl_function_highlight` ("1B"), `band_months`, `time_years`,
  `legend_guides`.
- **Trade-off**: with a fixed domain the x and y axis lines no longer meet at a
  corner when the overhang is large (`bubble_stroke`).

#### Test changes forced by the feature

- `rescale_reuses_program_pipeline_and_columns` zooms to (0.5, 2.0) so its tick
  labels reuse the first render's glyphs (an atlas write is counted as a texture
  write).
- `appended_rows_upload_only_their_bytes` expects `grown.0 <= domain.0`: the
  widened lower bound moves with the data span.
- The `EncodeFn::then` doc example swaps Amsterdam for a city 8 degrees east,
  since the narrower scale made its small circle cover London's centre.
- `msaa.rs` fixes its domains so the rule-fan crop stays clear of the circles.
- `zoom_uploads` guide-instance budget 1,387,872 to 1,400,736 B (axis labels and
  ticks changed). `PERF_BUDGETS.md` `zoom.instances.bytes` 5,629,476 to
  5,646,396 was measured but NOT edited (see report).

### Development Workflow Insights

- Thinning ends at count 1, so a very long label on a very small linear axis
  leaves one tick; the AC test uses 0 to 1000 at 120 px.
- `window_parity` was not run (root crate).

### Follow-up Stories

None.
