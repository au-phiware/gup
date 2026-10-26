---
implementer: sonnet
implementer-reason:
  one overhang rule in an existing function plus goldens; the policy question
  (do straight lines change?) is settled in Decisions below
touches: [crates/gup-core/src/selection.rs]
---

# GUP-558: Step Risers at the Plot Edge Draw Whole

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-10-26

## Context

GUP-550 added step curves to `Line`. A step line's riser (its vertical piece)
sits at a row's x: `StepBefore` rises at each row, so its first riser is at the
first row's x, and `StepAfter` rises at the next row's x, so its last riser is
at the last row's x. When the x domain ends at that row, which is what an
automatic domain does, the riser lies on the plot's edge and the plot clips half
its width. The golden `gup_core/line_steps` shows it: the blue (`StepBefore`)
riser at x = 0 is drawn at half width.

A line's `width` channel has no role, so `Selection::overhang` counts nothing
for it and GUP-528's edge insets leave lines alone ("bars and lines are
unchanged", GUP-528 AC6). For a straight line that is the usual look, with the
line starting at the axis. A half-width vertical stroke looks like a rendering
fault.

## User Story

> "As an analyst drawing a step line, I want the first and last risers drawn at
> full width, so that the chart does not look broken at its edges."

## Acceptance Criteria

- [ ] A `StepBefore` line whose first row is at the x domain's start, and a
      `StepAfter` line whose last row is at its end, draw the riser at full
      width (pixel test: the ink across the riser matches an interior riser
      within 2/255).
- [ ] Straight lines (`Curve::Linear`) and `Step` (whose risers sit between
      rows) are unchanged: `line_series`, `builder_line` and every other golden
      pass without re-blessing.
- [ ] `gup_core/line_steps` is re-blessed, and the worker reads it and describes
      it.

## Decisions

- Only the x overhang of `StepBefore` and `StepAfter` layers changes:
  `width / 2` (a constant width, or the column's largest), through the existing
  `Overhang.x` path that GUP-528 insets from.
- Straight lines keep starting at the axis.

## Technical Tasks

- [ ] In `Selection::overhang`, add `width / 2` to `overhang.x` for a `Line`
      layer whose curve is `StepBefore` or `StepAfter`.
- [ ] Pixel test in `crates/gup-core/tests/line_curves.rs`; re-bless
      `line_steps`.

## Dependencies

### Prerequisite Stories

- GUP-550: `Curve` and step lines.
- GUP-528: data-aware edge insets.

## Testing Strategy

- Pixel test of riser ink at the domain ends; goldens unchanged except
  `line_steps`.

## Success Metrics

- [ ] No half-width riser at the plot's x edges.

## Risk Assessment

- **Low**: the inset moves the x axis's ends by a pixel or two for step lines
  only.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Rendered output verified by eye (`gup_core/line_steps`)
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added to the story document
