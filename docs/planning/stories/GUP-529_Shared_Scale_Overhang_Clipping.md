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

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-25

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
- [ ] **AC2, cause**: the story's Implementation Summary names the exact cause
      (file, function, why the overhang or widening was lost) in two to four
      sentences.
- [ ] **AC3, fix**: AC1 passes. The rule after the fix: every automatic axis is
      widened by the largest overhang of any layer bound to it, whatever kind of
      layer (builder or raw `Selection`), and whatever order the layers were
      added in; explicit domains keep GUP-540's (or GUP-528's, if merged) inset.
- [ ] **AC4, order independence**: the same test with the circles added before
      the line also passes.
- [ ] **AC5, goldens**: re-bless `tests/golden/tutorials/02_layers.png` and any
      other golden that changes; at most 8 may change in total. Look at each new
      PNG with the Read tool; every circle whole, ticks still at the domain's
      rounded ends.
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

## Escalation Triggers

- The cause is neither of the two named in Decisions.
- The fix changes more than 8 goldens, or any bar/line-only golden.
- The fix needs a public API change.
- `scripts/perf_budget.sh --skip-wasm` changes an `exact` row.

## Out of Scope

- Explicit-domain insets (GUP-540, GUP-528).
- Band/point axes.

## Definition of Done

- [ ] ACs met; `mask all-fix` clean; story ✅ in INDEX.md with Implementation
      Summary and Retrospective.
