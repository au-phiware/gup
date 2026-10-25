---
implementer: sonnet
implementer-reason:
  a bug with an exact reproduction, a ranked list of suspects and a fixed
  acceptance image; the fix rule is stated, only the cause must be found
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/tests/,
    tests/golden/,
  ]
---

# GUP-529: A Visible Gap Between Marks and the Axes

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-25) **Created**: 2026-10-25

## Context

The story was filed as "circles on a shared automatic scale are clipped"
(tutorial 2's layered chart, tutorial 4's brush chart). Measured on main
(2026-10-25) nothing is clipped: widening works for a builder layer plus a raw
`Selection` in either order, and `is_auto()` stays true. Tutorial 2: plot rect x
36, y 59, 518×291; `x(1)` = 42, `y(22)` = 65, so the edge discs span exactly 36
and 59, tangent to the axis lines. Tutorial 4's Bellbird spans rows 338..349
with the plot bottom at 350. Automatic axes were widened by the overhang alone
(`plot_padding` is added only on explicit axes' far ends), so discs touch the
axis lines.

**Orchestrator decision (2026-10-25)**: reframe to a visible gap. For an
automatic axis, widen by `over + plot_padding` when `over > 0` and by 0 when
`over == 0`, at both ends, so discs never touch an axis line while a bar's
baseline and lines still sit on their axes. Explicit domains are unchanged. The
tests require at least 2 px of background between a disc's edge and the plot
rect; the golden cap is 40, and no bar-only, line-only or area golden may
change.

## User Story

> "As a newcomer layering a line and circles on shared scales, I want every
> circle drawn whole, so that my first layered chart looks finished."

## Read First

- `crates/gup-core/src/plot.rs`, step 5 of resolve ("Ranges, inset so marks on
  the domain edge stay inside"): `overhang()` per layer, `over`, `widen`,
  `set_ranges`, `kept`.
- `crates/gup-core/src/selection.rs`: `fn overhang` for `Selection` (around the
  `ConstValue::F32` match) and the trait method's docs.
- The scale's `is_auto`, `widen` and `current_domain` (grep `fn widen` and
  `fn is_auto` under `crates/gup-core/src/scale/`).
- How `plot.line(..)` (builder on `Plot`) binds to the plot's x and y scales
  (grep `fn line` in `crates/gup-core/src/plot.rs` and `builder/line.rs`).

## Acceptance Criteria

- [x] **AC1, reproduction first**: a failing test in `crates/gup-core/tests/`
      (new file `shared_scale_overhang.rs`) renders the tutorial's chart (same
      data, 640×400, line + circles on `plot.x(Linear::new())` /
      `plot.y(Linear::new())`, `Px(6.0)` radius) and asserts every disc is
      whole: for each data point, the pixel 5 px above, below, left and right of
      its centre (from the scales' `eval`) lies inside the plot rect. Commit it
      failing (WIP, `scripts/checkpoint.sh` if the hook refuses) before any fix.
- [x] **AC1b, second reproduction**: in the same file, the tutorial 4 bird
      scatter (data and builder calls as in the tutorial, no brush, 640×400)
      asserts every disc is whole the same way. Also commit it failing first.
- [x] **AC2, cause**: the story's Implementation Summary names the exact cause
      (file, function, why the overhang or widening was lost) in two to four
      sentences.
- [x] **AC3, fix**: AC1 passes. The rule after the fix: every automatic axis is
      widened by the largest overhang of any layer bound to it, whatever kind of
      layer (builder or raw `Selection`), and whatever order the layers were
      added in; explicit domains keep GUP-540's (or GUP-528's, if merged) inset.
- [x] **AC4, order independence**: the same test with the circles added before
      the line also passes.
- [x] **AC5, goldens**: re-bless `tests/golden/tutorials/02_layers.png`,
      `04_brush.png` and any other golden that changes; at most 12 may change in
      total. Look at each new PNG with the Read tool; every circle whole, ticks
      still at the domain's rounded ends.
- [x] **AC6, gates**: `mask merge-check --dogfood` passes;
      `cargo test --doc -p     gup` passes (the tutorials are doctests).

## Decisions

- Fix the cause, not the symptom: no tutorial change (no explicit domain, no
  extra padding in the tutorial).
- If the cause is that one of the two layers fits or fixes the shared scale's
  domain so `is_auto()` is false afterwards, the fix is that fitting a shared
  automatic scale never turns it explicit; widening applies after all layers
  have been fitted.
- If the cause is that `overhang()` reports 0 for this layer, fix `overhang()`
  and add a unit test in `selection.rs` for a constant `Px` radius on a raw
  `Selection`.

- If widening is sized against a plot rect that the following relayout changes,
  iterate: re-widen against the new plot rect until the domain stops growing (at
  most 3 passes; assert it converges in the test), or size the widening against
  the final rect. Either is acceptable; prefer the one that keeps tick values
  unchanged.
- If tick rounding after widening shrinks the room, the rounding must never give
  back less than the overhang.

## Escalation Triggers

- The cause is none of those named in Decisions.
- The fix changes more than 12 goldens, or any bar/line-only golden.
- The fix needs a public API change.
- `scripts/perf_budget.sh --skip-wasm` changes an `exact` row.

## Out of Scope

- Explicit-domain insets (GUP-540, GUP-528).
- Band/point axes.

## Definition of Done

- [x] ACs met; `mask all-fix` clean; story ✅ in INDEX.md with Implementation
      Summary and Retrospective.

## Implementation Summary

- Cause: `plot.rs` step 5 passed `over[i]` (overhang only) to `Scale::widen`, so
  automatic axes got no `plot_padding` and edge marks were tangent to the axis
  lines (nothing was clipped). Fix: a `padded` helper (`over + plot_padding` if
  `over > 0`, else 0) is passed to `widen` for x, y and the right axis.
- `crates/gup-core/tests/shared_scale_overhang.rs`: tutorial 2 chart in both
  layer orders and the tutorial 4 scatter, each asserting 2 px of background
  between every disc's edge and the plot rect.
- Tests adjusted for the wider domains: `plot::tests::rescale_reuses_...` warms
  the glyph atlas first (new tick digits); `hello_world.rs` points at row 30 (a
  tooltip above a point at the top edge leaves the plot rect); `zoom_uploads.rs`
  guide-instance budget 1,503,048 to 1,506,360 B (`PERF_BUDGETS.md` line 493
  needs re-recording by the orchestrator).
- 30 goldens re-blessed: 25 in `tests/golden/gup_core/`, 5 in
  `tests/golden/tutorials/`. No bar-only, line-only or area golden changed;
  `secondary_y` (bars plus a line) changed: the outer bars now clear the axes.

## Retrospective

**Completed**: 2026-10-25

- A bug report's premise should be measured first: an ASCII dump of the rendered
  pixels around one disc showed "whole but tangent" in minutes.
- Widening more thins the tick set (tutorial 2's x labels become even months),
  which can invalidate tests that assumed certain glyphs were already in the
  atlas.
- Tooltips are clamped to the canvas, not the plot, so a point near the top edge
  puts the tooltip's border outside the plot rect.
