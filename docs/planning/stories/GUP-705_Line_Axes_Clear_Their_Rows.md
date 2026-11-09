---
implementer: sonnet
implementer-reason:
  extends the existing gap rule (automatic axes widen by mark overhang plus
  padding) to marks without overhang; a contained change to the domain fit
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/scale/,
    crates/gup-core/tests/live_view.rs,
    tests/golden/,
    CHANGELOG.md,
  ]
---

# GUP-705: A Line's Automatic Y Axis Clears Its Lowest and Highest Rows

## Story Overview

**Initiative**: Strategic Review 2026-11 (first outside user, before 0.1)
**Status**: 🚧 In Progress **Created**: 2026-11-09

## Context

The first outside user's live population line settled at about 500 and "sat on
the x axis and vanished": the fitted y domain started at the data minimum.
GUP-699 added `Linear::include_zero()` and view read-back, and measured that
without it the lowest row of a line lands exactly on the axis line (gap 0 px).
Circles already get clearance from the gap rule (automatic axes widen by the
mark's overhang plus padding, GUP-529); a line has no overhang, so it gets none.
A line drawn on top of the axis line is hard to see and reads as "no data".

## User Story

> "As a developer with a line whose minimum is a round number, I want the line
> drawn clear of the axis, so that it never merges with it."

## Read First

- GUP-529 (the gap rule), GUP-594 (pinned and builder x extents), GUP-699's
  Implementation Summary (AC1 measured gap 0 px).
- The domain fit in `crates/gup-core/src/plot.rs` (overhang, `plot_padding`,
  `AxisFit` Nice/Ticks/Extent) and `crates/gup-core/tests/live_view.rs`.

## Acceptance Criteria

- [ ] An automatic (fitted, not explicit, not `include_zero`-anchored at the
      data edge) value axis of a line or area layer leaves at least the plot
      padding (in pixels, the same constant the gap rule uses) between its
      lowest and highest rows and the plot rect's edges. The x extent rule for
      line/area (rows end at the plot edges, GUP-594) is unchanged.
- [ ] `include_zero()` with all-positive data still puts zero exactly on the
      axis (a baseline is meant to sit there); an explicit `domain(a, b)` is
      unchanged.
- [ ] A test reproduces the user's case (a line whose minimum is a round number
      that Nice rounding would keep as the domain minimum) and asserts the gap ≥
      the padding; GUP-699's `|gap| < 1` assertion is updated.
- [ ] Changed goldens are counted before re-blessing (STOP and report if more
      than ~25), re-blessed and read by eye; each is listed with what changed.
- [ ] CHANGELOG `[0.1.0]` Changed line.

## Decisions

- Reuse the gap rule's padding constant; don't add a new knob.
- Value axes only; the line's x extent keeps ending at the first and last rows.

## Escalation Triggers

- The change moves Nice rounding so that tick labels change on most goldens.
- Bars or areas with a zero baseline lose their baseline on the axis.

## Out of Scope

- Making `include_zero` the default for line charts.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask merge-check`, `mask all-check`,
      `cargo test -p gup -- --test-threads=1`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
