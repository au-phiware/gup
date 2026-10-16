---
implementer: sonnet
implementer-reason:
  the log nice rule, the B suffix and the measured thinning loop are settled in
  Decisions; each is a local change to existing tick code
---

# GUP-435: RFC-001 Step S7f: Tick Label Quality

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-16

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

- [ ] Unit tests (base 10) assert niced domains: `(3.2, 870)` → `(2, 1000)`;
      `(1, 1000)` → `(1, 1000)`; `(8, 12000)` → `(5, 20000)`; `(0.03, 4e6)` →
      `(0.02, 5e6)`. Base 2 keeps whole powers (existing behaviour, test kept).
- [ ] `categorical_species` is re-blessed and read by eye: the log x axis spans
      5 to 20,000 and the data fills most of it. The retrospective records
      before/after domains.

### AC2: Count suffixes

- [ ] `format_log_tick(1e9) == "1B"`, `format_log_tick(2.5e9) == "2.5B"`; `k`,
      `M`, `T` unchanged. The font sweep test's expected characters become
      `"0123456789−.ekMBT:"`.
- [ ] `wgsl_function_highlight` (and any golden showing a "G" label) is
      re-blessed and read by eye: the axis reads "1B".

### AC3: Labels never overlap

- [ ] A test resolves `band_months`' data (12 full month names) at 240×200 and
      asserts adjacent x tick labels' ink boxes are separated by at least
      `theme.label_gap`, and that at least 2 labels remain.
- [ ] The same assertion holds for 60 short keys (`k0`…`k59`) at 720×450, for a
      `Linear` x at 120×120, and for a `Band` y with 40 keys at 720×200
      (vertical spacing uses label height).
- [ ] Goldens whose labels did not overlap before are unchanged by the thinning
      (it is a no-op when nothing collides).

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

- [ ] Change `Log::nice` and `format_log_tick`; update their tests and the font
      sweep.
- [ ] Add the thinning loop to `Plot::resolve`.
- [ ] Tests for AC3; re-bless and read the affected goldens.

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

- [ ] No overlapping tick labels at any tested size.

## Risk Assessment

- **Medium**: `scatter.png` is also `window_parity`'s reference. _Mitigation_:
  re-bless it in the same commit and run `window_parity`.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
