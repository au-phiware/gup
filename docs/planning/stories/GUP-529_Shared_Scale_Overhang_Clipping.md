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

# GUP-529: Circles on a Shared Automatic Scale Are Clipped

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress **Created**: 2026-10-25

## Context

Tutorial 2's layered chart (`docs/tutorials/02_*.md`, the `02_layers` snippet;
image `tests/golden/tutorials/02_layers.png`) cuts its circles in half: the
discs at y = 22 lose their top half at the plot's top edge, and the discs at x =
1 and x = 12 sit on the plot's left and right edges. The plot creates two
automatic scales with `plot.x(Linear::new())` and `plot.y(Linear::new())`, adds
a `plot.line(..)` builder layer and a raw `Selection::<Month, Circle>` layer on
those scales, with `Circle::RADIUS` the constant `Px(6.0)`.

GUP-435 and GUP-494 widen automatic domains by the marks' overhang so ticks run
to both ends and no mark is clipped; GUP-540 insets explicit domains. Here
neither happened: the ticks run 1..12 and 12..22 exactly to the plot edges and
the 6 px discs are cut. This is the first chart a tutorial reader layers by
hand, so it must look right.

A second case needs no shared scale and no raw `Selection`: tutorial 4's brush
chart (`docs/tutorials/04_interactions.md`, image
`tests/golden/tutorials/04_brush.png`) is a plain `gup::scatter(birds)` with
`.radius(6.0)` and automatic domains. The Bellbird at (0.03, 0.20) is cut in
half by the bottom of the plot: the y domain's low end sits at about 0.19 with
the data minimum 0.20 on a round tick, so the widening there is far less than
the 6 px radius. The x axis, whose data minimum is not on a round tick, has
room. Suspect: widening sized against the plot rect before the relayout that
follows it ("the margins may change"), or tick rounding after widening.

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

- [ ] **AC1, reproduction first**: a failing test in `crates/gup-core/tests/`
      (new file `shared_scale_overhang.rs`) renders the tutorial's chart (same
      data, 640×400, line + circles on `plot.x(Linear::new())` /
      `plot.y(Linear::new())`, `Px(6.0)` radius) and asserts every disc is
      whole: for each data point, the pixel 5 px above, below, left and right of
      its centre (from the scales' `eval`) lies inside the plot rect. Commit it
      failing (WIP, `scripts/checkpoint.sh` if the hook refuses) before any fix.
- [ ] **AC1b, second reproduction**: in the same file, the tutorial 4 bird
      scatter (data and builder calls as in the tutorial, no brush, 640×400)
      asserts every disc is whole the same way. Also commit it failing first.
- [ ] **AC2, cause**: the story's Implementation Summary names the exact cause
      (file, function, why the overhang or widening was lost) in two to four
      sentences.
- [ ] **AC3, fix**: AC1 passes. The rule after the fix: every automatic axis is
      widened by the largest overhang of any layer bound to it, whatever kind of
      layer (builder or raw `Selection`), and whatever order the layers were
      added in; explicit domains keep GUP-540's (or GUP-528's, if merged) inset.
- [ ] **AC4, order independence**: the same test with the circles added before
      the line also passes.
- [ ] **AC5, goldens**: re-bless `tests/golden/tutorials/02_layers.png`,
      `04_brush.png` and any other golden that changes; at most 12 may change in
      total. Look at each new PNG with the Read tool; every circle whole, ticks
      still at the domain's rounded ends.
- [ ] **AC6, gates**: `mask merge-check --dogfood` passes;
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

- [ ] ACs met; `mask all-fix` clean; story ✅ in INDEX.md with Implementation
      Summary and Retrospective.
