---
implementer: sonnet
implementer-reason: small formatting change in one pure function
touches: [crates/gup-core/src/describe.rs, crates/gup-core/tests/describe.rs]
---

# GUP-600: Describe Composite Marks and Same-Named Layers

## Story Overview

**Initiative**: Strategic Review 2026-11 (N4 follow-up) **Status**: ✅ Complete
(2026-11-03) **Created**: 2026-11-02

## Context

GUP-592 lists layers by mark name, so a box plot reads "12 data rows in 4 layers
(rect 2, segment 4, segment 4 and segment 2)". Its rows are summaries, not data
rows, and its layers are an implementation detail.

## User Story

> "As a reader using a screen reader, I want the chart description to read
> naturally, so that I understand it on one hearing."

## Decisions

1. Builders that know their chart kind supply a summary: a crate-private `Plot`
   field (`kind_summary: Option<String>`) set when the builder materialises,
   used by `describe` in place of the "N data rows in K layers (...)" clause. A
   user-set `Plot::description(text)` still overrides everything.
   - Box plot: "Box plot of 2 groups: api, median 54, middle half 40 to 70,
     whiskers 20 to 95, 1 outlier; web, ..." with numbers through the value axis
     tick format (or 3 significant digits), at most 8 groups then "and N more".
   - Histogram: "Histogram of 1,200 values in 14 bins from 10 to 80" (stack
     groups if present: "..., stacked by Session: Morning, Evening").
   - Other builders keep the generic clause with Decision 2.
2. Generic clause: layers of the same mark kind merge ("30 data rows: circle 20,
   line 10"); the per-layer list is dropped when there is one kind; internal
   mark kinds (rect/segment) from builders that supply a summary are never
   listed.
3. Vertical band/point axes list categories top to bottom; horizontal ones left
   to right (unchanged). (GUP-601)
4. No new public API beyond GUP-592. WASM growth at most 1,000 B gz. GUP-600 and
   GUP-601 are implemented together.

## Acceptance Criteria

- [x] Layers with the same mark name are merged in the parenthesis
      (`segment 10`).
- [x] A box plot reads as its source rows and boxes, e.g. "40 data rows in 2
      boxes", not by internal layers (needs a per-builder description hook).
- [x] Unit tests for both.

## Technical Tasks

- [x] Change `describe()` and its unit tests.
- [x] Update `tests/describe.rs`.

## Dependencies

### Prerequisite Stories

- GUP-592 ✅

## Testing Strategy

Unit tests in `describe.rs` and `tests/describe.rs`; no goldens change.

## Success Metrics

- [x] The three descriptions in GUP-592's summary read naturally.

## Risk Assessment

- **Low**: wording only. _Mitigation_: `description(text)` overrides it.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] `mask all-fix` clean; `mask wasm-size` within +1,000 B gz
- [x] Retrospective added

## Implementation Summary

GUP-600 and GUP-601 were implemented together in one commit
(`crates/gup-core/src/describe.rs`, `plot.rs`, `builder/boxplot.rs`,
`builder/histogram.rs`, `tests/describe.rs`). A crate-private
`Plot::kind_summary` set by the box plot and histogram builders replaces the row
clause; the generic clause merges marks ("30 data rows: circle 20, line 10");
keyed y axes list top to bottom. Numbers in summaries use three significant
digits (`sig3`), not the axis tick format (the format is only known at resolve;
the summary is built at materialise).

Descriptions now:

- Box plot: "Latency. Box plot of 2 groups: api, median 57, middle half 28.5 to
  85.5, whiskers 0 to 114; web, median 60, middle half 31.5 to 88.5, whiskers 3
  to 117. X axis: 2 categories, api and web. Y axis: 0 to 120."
- Stacked histogram: "Trips. Histogram of 60 values in 6 bins from 0 to 30,
  stacked by Session: Morning and Evening. X axis: 0 to 30. Y axis "Count": 0
  to 11. Colour legend "Session": Morning and Evening."
- Heatmap: "Visits. Heatmap, 12 data rows. X axis: 4 categories, 0, 1, 2 and 3.
  Y axis: 3 categories, Mon, Tue and Wed. Colour legend "Visits": 0 to 30."
- Two-layer right axis: "Climate. Chart, 30 data rows: circle 20, line 10. X
  axis: 0 to 20. Y axis: 0 to 20. Right y axis "Rain (mm)": 0 to 9."

## Retrospective

**Completed**: 2026-11-03

- A summary built at materialise goes stale if the user adds layers through
  `plot_mut()`; accepted (use `description(text)`).
- Reversing keyed y labels in `describe` (not in tick generation) leaves the
  drawn picture untouched.
- No follow-ups.
