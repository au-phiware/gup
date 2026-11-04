---
implementer: sonnet
implementer-reason:
  a measurement with a decision rule, then a one-test edit; judgement is only in
  reading the measurement, which the rule bounds
touches: [crates/gup-core/tests/boxplot_png.rs]
---

# GUP-617: Tight Box Width Test

## Story Overview

**Initiative**: Polish after 0.1 **Status**: 🚧 In Progress **Created**:
2026-11-04

## Context

`marks_are_where_the_statistics_put_them` in
`crates/gup-core/tests/boxplot_png.rs` (lines 80–115) checks the box's width
against `0.6 × 0.8 × plot.width` with a 3 px tolerance, while every other mark
in the same test is held to 1 px (the box's centre, the median, the whiskers'
ends) or 2 px (the caps). The 3 px was a loosening during GUP-554, suspected to
come from how the ink is measured: the box is a 0.3-opacity fill inside a 1 px
opaque stroke (`builder/boxplot.rs` line 331; `Rect` strokes are drawn inside
the edge, `marks.rs` line 129), and the test counts every pixel darker than 0.05
along one row. A 3 px slack would hide a real 2 px error in the box width
(`BOX_WIDTH × band_width`), which is exactly the kind of numeric error that
passes a golden read by eye.

## Read First

- `crates/gup-core/tests/boxplot_png.rs` lines 39–115.
- `crates/gup-core/src/builder/boxplot.rs` lines 270–335 (box geometry:
  `BOX_WIDTH`, the `Derived` width from `band.read().band_width()`).
- `crates/gup-core/src/scale/band.rs` (`band_width`, padding defaults).

## User Story

> "As a maintainer, I want the box plot's width test to be as tight as its other
> mark tests, so that a pixel-level regression in box geometry fails CI."

## Acceptance Criteria

### AC1: Measure first

- [ ] Record in the story's retrospective, for the test's chart at its size: the
      band scale's pixel `band_width` and range as resolved (read through the
      chart's `plot_mut()` and the band handle, or `Layout`), the expected box
      width `BOX_WIDTH × band_width`, the first and last ink columns on the
      measured row with each one's `dark` value, and the measured width.

### AC2: Apply the rule

- [ ] Apply the decision rule below, and record which branch applied and why.
- [ ] The width assertion's tolerance is at most 1.0 px.
- [ ] The test passes on the local GPU and on lavapipe (`mask test` as CI runs
      it, or the CI run of the branch).

### AC3: Prove it bites

- [ ] Temporarily scale the box width by `1.0 + 3.0 / width` (3 px wider) in
      `builder/boxplot.rs`; the test fails; revert. Record the failure message
      in the retrospective.

## Decisions

Decision rule, applied to AC1's numbers:

1. **The expectation is wrong** (the resolved `band_width` differs from
   `0.8 × plot.width`, for example because the band range is inset): compute
   `want` from the resolved band width (`BOX_WIDTH × band_width`) instead of
   from `plot.width`, and tighten to 1.0 px.
2. **The measurement is wrong** (the expectation is right, and the extra width
   is edge pixels with `dark` below 0.5, the antialiased edge of the opaque
   stroke): measure at half coverage, counting the columns with `dark ≥ 0.5` on
   the row, and require `|count − want| ≤ 1.0`.
3. **Both are right and the box is still off by more than 1 px**: this is a
   geometry bug; stop (Escalation Triggers).

## Escalation Triggers

- Rule branch 3: the box is drawn wider or narrower than
  `BOX_WIDTH × band_width` by more than 1 px. Do not fix `boxplot.rs` in this
  story; report the numbers.
- The width differs between the local GPU and lavapipe by more than 1 px.

## Out of Scope

- Other loose tolerances in other tests (no audit here).
- Changing the box plot's geometry or defaults.

## Technical Tasks

- [ ] Add temporary prints, run the test, record AC1.
- [ ] Edit the assertion per the rule; remove the prints.
- [ ] AC3's negative control.

## Dependencies

### Prerequisite Stories

- GUP-554: Box Plot Builder ✅ — the test.

## Testing Strategy

- **Integration tests**:
  `cargo test -p gup-core --test boxplot_png -- --test-threads=1`.

## Success Metrics

- [ ] The box width tolerance is ≤ 1.0 px and a 3 px error fails.

## Risk Assessment

- **Low**: one test. _Mitigation_: AC3 proves the tightened test still catches
  an error, and AC2 runs it on the CI rasteriser.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, no image changes
- [ ] `mask wasm-browser`: not needed, test-only change
- [ ] WASM size: unchanged: no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
