---
implementer: sonnet
implementer-reason:
  two small example edits and their goldens; the choices are in Decisions
touches:
  [
    examples/composite_mixed_data.rs,
    examples/gallery/scatter_bubbles.rs,
    tests/golden/gallery,
  ]
---

# GUP-733: Gallery and Example Polish

## Story Overview

**Initiative**: Strategic Review 2026-11 (polish before 0.1) **Status**: ✅
Complete 2026-11-12 **Created**: 2026-11-12

## Context

Two things stood out reviewing the gallery. `composite_mixed_data` draws the fit
line in the points' blue, so the fit does not stand out, and the chart has no
axis titles. `scatter_bubbles` (also the README hero, golden `gallery/hero`)
maps `size` through the builder's default `Pow::sqrt()` onto 2 to 12 px radius;
the smallest countries come out near 3 px, almost invisible.

## Read First

- `examples/composite_mixed_data.rs` (whole).
- `examples/gallery/scatter_bubbles.rs` (whole).
- `crates/gup-core/src/builder/scatter.rs` lines 186–196 (`size_scale`).

## User Story

> "As a reader of the gallery, I want the fit to read as a fit and every bubble
> to be visible, so that the examples I copy look right."

## Acceptance Criteria

### AC1: The fit stands out

- [x] The fit line uses `OKABE_ITO_DISTINCT[1]` (by index); the example has axis
      titles; the code stays short.

### AC2: Visible bubbles

- [x] `scatter_bubbles` sets a larger minimum radius through `size_scale`; the
      smallest bubble is clearly visible and the largest does not swamp others.
- [x] Goldens fed by these examples are re-blessed and read by eye.

## Decisions

- **Fix in the example.** A 2 px minimum is a sensible library default for dense
  scatters; only this chart wants larger bubbles. No library change.

## Escalation Triggers

- The library default is judged wrong for every user.

## Out of Scope

- Library defaults; other gallery charts.

## Dependencies

- GUP-658 ✅ (`OKABE_ITO_DISTINCT`), GUP-659 ✅.

## Testing Strategy

`cargo test -p gup -- --test-threads=1` (gallery goldens),
`mask smoke-examples`, `mask merge-check`, `mask all-check`.

## Success Metrics

- [x] Fit and points differ in colour; smallest bubbles are legible.

## Risk Assessment

- **Low**: larger bubbles may overlap more. _Mitigation_: read the golden.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] Tests and lint pass
- [x] Rendered output verified by eye
- [x] Story status updated in file and INDEX.md

## Implementation Summary

`composite_mixed_data`: the fit takes `.color(OKABE_ITO_DISTINCT[1])`
(vermillion) and the plot gets "Dose" and "Response" titles; read by eye: an
orange line through blue points reads as a fit. `scatter_bubbles`:
`.size_scale( Pow::sqrt().range(Px(4.0), Px(16.0)))`; goldens `gallery/hero` and
`gallery/scatter_bubbles` re-blessed and read: the smallest bubbles are about 4
px in radius and plainly visible, the largest (Asia, about 16 px) do not swamp
their neighbours. No library change; no other golden changed.

## Retrospective

**Completed**: 2026-11-12

- The default 2 to 12 px range is fine for dense scatters; only this sparse
  chart wants a larger minimum, so it is set in the example.
- The Line colour setter is `.color(Color)`, not `.fill`/`.stroke`.
