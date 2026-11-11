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
**Status**: ✅ Complete (2026-11-11) **Created**: 2026-11-09

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

- [x] An automatic (fitted, not explicit, not `include_zero`-anchored at the
      data edge) value axis of a line or area layer leaves at least the plot
      padding (in pixels, the same constant the gap rule uses) between its
      lowest and highest rows and the plot rect's edges. The x extent rule for
      line/area (rows end at the plot edges, GUP-594) is unchanged.
- [x] `include_zero()` with all-positive data still puts zero exactly on the
      axis (a baseline is meant to sit there); an explicit `domain(a, b)` is
      unchanged.
- [x] A test reproduces the user's case (a line whose minimum is a round number
      that Nice rounding would keep as the domain minimum) and asserts the gap ≥
      the padding; GUP-699's `|gap| < 1` assertion is updated.
- [x] Changed goldens are counted before re-blessing (STOP and report if more
      than ~25), re-blessed and read by eye; each is listed with what changed.
- [x] CHANGELOG `[0.1.0]` Changed line.

## Decisions

- Reuse the gap rule's padding constant; don't add a new knob.
- Value axes only; the line's x extent keeps ending at the first and last rows.

## Escalation Triggers

- The change moves Nice rounding so that tick labels change on most goldens.
- Bars or areas with a zero baseline lose their baseline on the axis.

## Out of Scope

- Making `include_zero` the default for line charts.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask merge-check`, `mask all-check`,
      `cargo test -p gup -- --test-threads=1`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Overhang.clear_y` (selection.rs): a Line clears both ends of its value axis,
  an Area only its top edge (its baseline stays on the axis).
- plot.rs: the widen step pads a no-overhang end by `plot_padding` when the
  layer clears it; an end that is exactly 0 on an `include_zero` scale is
  skipped (`PositionScale::anchors_zero`, default false, Linear overrides,
  unit-tested). Explicit domains and x extents are untouched.
- The user's case: gap 0 px -> 4 px (the padding). The live_view assertion is
  now `gap >= padding - 0.5`.
- 24 goldens re-blessed (the first report said 29 by miscounting source files),
  all read: gallery area_stacked, dark_multiline, line_monotone; gup_core
  builder_area, builder_line, line_series, line_steps, line_translucent,
  secondary_y, subtitle, subtitle_dark; tour 02_line, 04_dual_axis,
  04_time_axis, 05_dark, 05_high_contrast, 05_palette, 06_curves, 10_append,
  10_window; tutorials 05_appended, 07_area, 07_dark, 07_line. In each the line
  sits one padding clear of the axis, area baselines still sit on the axis at 0,
  dark and high-contrast match their light twins, both y axes of dual_axis and
  secondary_y clear (secondary_y measured 8 degC at 396 px against a plot bottom
  of 400 px).
- WASM scatter gzip: 378,922 B at the start commit, 379,222 B on the branch
  (+300 B).

## Retrospective

**Completed**: 2026-11-11

### Key Technical Learnings

#### Reusing the widen step

- **Challenge**: a line has no overhang, so the gap rule gave it nothing.
- **Solution**: a per-layer "clear" flag per end, fed into the existing
  `padded()` helper; no new constant or Nice change, so ticks barely moved.
- **Pattern**: extend the existing fit hook rather than adding a second pass.

### Architectural Decisions

#### Area clears its top only

- **Decision**: Area keeps its baseline on the axis and clears the top edge.
- **Reasoning**: the escalation trigger forbids losing a zero baseline.
- **Trade-off**: an area whose baseline is a data column is not cleared at its
  bottom.

#### `anchors_zero` on `PositionScale`

- **Decision**: a defaulted public method.
- **Trade-off**: a public trait addition for one flag.

### Development Workflow Insights

- A rendered image can mislead on gaps (a line at 8 looked on the axis);
  printing the pixel settled it.
- Count golden PNGs (`git status --short | grep -c png`), not changed files.

### Follow-up Stories

None.
