---
implementer: sonnet
implementer-reason:
  a table assembled from GUP-621's row fields and written to the DOM beside
  GUP-623's live region; shape, cap and refresh rule settled in Decisions
touches:
  [
    crates/gup-core/src/describe.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/chart.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/builder/,
    crates/gup-core/src/web.rs,
    crates/gup-core/wasm-size/scatter/,
  ]
---

# GUP-624: Hidden Data Table in the Browser

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: 🚧 In Progress **Created**:
2026-11-04

## Context

A chart's text alternative (GUP-592) summarises it; keyboard focus with
announcements (GUP-619–623) reads one point at a time. Screen-reader users also
expect the data itself as a table they can browse with table navigation: the
standard accessible-chart pattern is a visually hidden `<table>` next to the
graphic. GUP-621's `Plot::row_fields` already gives every row's values as named
text, so the table is an assembly of those, and `CanvasApp` (after GUP-623)
already manages a hidden sibling of the canvas.

## Read First

- GUP-621's story file, "Decisions" (row fields: order, names, values).
- `crates/gup-core/src/describe.rs` lines 1–40 (`Description`, `alt_text`).
- `crates/gup-core/src/chart.rs` (the trait).
- `crates/gup-core/src/web.rs` after GUP-623: the live region's creation, update
  and removal, and `update_label`.
- `crates/gup-core/wasm-size/scatter/index.html` (the steps GUP-623 added).

## User Story

> "As a screen-reader user, I want the chart's data as a table I can browse, so
> that I can read every value, compare rows and check the chart's claims."

## Acceptance Criteria

### AC1: `DataTable`

- [x] `DataTable` (Decisions) is in `describe.rs` and re-exported at the crate
      root.
- [x] `Plot::data_table(&self, max_rows: usize) -> Option<DataTable>` builds it
      per Decisions; `None` before the first resolve.
- [x] `Chart::data_table(&self, max_rows: usize) -> Option<DataTable>` with a
      default of `None`; `Plot` and the seven builders implement it by
      delegation.
- [x] Tests: a two-series line chart (a `Series` column, x and y columns named
      by the axis titles, rows in layer then data order); a grouped bar chart; a
      box plot (one row per box, the five-number columns, outliers' rows after);
      the cap (`max_rows` 3 over 10 rows gives 3 rows and `omitted == 7`, and
      the caption says "first 3 of 10 rows").

### AC2: In the browser

- [x] `CanvasApp` inserts a visually hidden `<table data-gup-table>` (the same
      style as the live region) after the live region, with a `<caption>`, a
      header row of `<th scope="col">` and one `<tr>` per row of
      `chart.data_table(1_000)`; removed on drop.
- [x] It is rebuilt only when the chart's `describe()` alt text changes (which
      covers new rows, since the description counts them), not on zoom or hover.
- [x] Browser smoke steps: the table exists after the canvas; its caption starts
      with the chart's description title; it has 6 body rows; the first header
      is the first field name of row 0; appending a row through the harness
      (`app.append_row(..)`, as the existing append step does) gives 7 body rows
      after the next frames.
- [x] `mask wasm-browser` passes.

## Decisions

- **Type**:

  ```rust
  #[derive(Clone, Debug, Default, PartialEq, Eq)]
  pub struct DataTable {
      /// The description's title, plus " (first {k} of {n} rows)" when rows were omitted.
      pub caption: String,
      /// Field names, in first-seen order over the rows included.
      pub headers: Vec<String>,
      /// One entry per row, aligned with `headers` ("" where a row lacks a field).
      pub rows: Vec<Vec<String>>,
      /// Rows not included because of `max_rows`.
      pub omitted: usize,
  }
  ```

- **Rows**: every row of every focusable data layer (GUP-619's rule, but not
  limited to the current view), in plot order of layers and data order of rows,
  through `Plot::row_fields`; a row whose fields are `None` is skipped and not
  counted. The first `max_rows` are included. Tooltips are not used (they are
  free text, not cells).
- **Counts in captions**: written with thousands separators
  (`describe::push_count`).
- **Web cap**: 1,000 rows. A screen reader's table navigation is usable at that
  size; the description already states the full count.
- **Refresh trigger**: the alt text string, which `update_label` already
  computes each frame; compare and rebuild only on change. When the page named
  the canvas itself (`label_canvas == false`), compute the alt text anyway for
  this comparison.

## Escalation Triggers

- Building the table for 1,000 rows takes more than 5 ms in the release WASM
  build (measure in the browser step with `performance.now()` around a forced
  rebuild, if the harness can trigger one).
- The `describe()` text does not change on append for some chart (then the table
  goes stale; report which).

## Out of Scope

- A table in SVG output or in the window and egui hosts.
- Sorting or paging the table.

## Technical Tasks

- [x] `DataTable`, `Plot::data_table`, `Chart::data_table` and delegations.
- [x] The DOM table in `web.rs`, its refresh rule and removal.
- [x] Unit and browser tests.

## Dependencies

### Prerequisite Stories

- GUP-621: Row Announcements 📋 — `row_fields`.
- GUP-623: Browser Keyboard and Live Region 📋 — the hidden sibling pattern in
  `web.rs`.

## Testing Strategy

- **Unit tests**: `DataTable` assembly and the cap.
- **Integration tests**: the browser steps.

## Success Metrics

- [x] Every browser chart carries its data as a table a screen reader can
      navigate.

## Risk Assessment

- **Low**: assembly of existing values. _Mitigation_: the cap bounds DOM size.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, DOM only
- [x] `mask wasm-browser` passes (AC2)
- [x] WASM size: `mask wasm-size` scatter gz bytes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `DataTable` (`describe.rs`, re-exported), `Plot::data_table`,
  `Chart::data_table` (default `None`) and the seven builders' delegations.
  Cells come from `Plot::row_fields`, so columnar layers work (`row_fields`
  reads CPU columns while held; a layer with neither rows nor columns has no
  fields and is skipped).
- `web.rs`: a hidden `<table data-gup-table>` after the live region, rebuilt in
  `update_table` only when the alt text changes, removed on drop.
- Tests: `crates/gup-core/tests/data_table.rs` (7); browser steps `table`,
  `table-caption`, `table-rows`, `table-headers`, `table-appended`.
- Evidence: `mask wasm-browser` PASS with `table-appended-frames=1`. WASM
  scatter gz: main 373,296, branch 374,944 (+1,648 B).
- Not measured: the 1,000-row rebuild time (the harness cannot force a rebuild
  of that size).

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Counting omitted rows

- **Challenge**: "k of n" needs a total, but formatting every row to count is
  O(n) strings per append on large charts.
- **Solution**: rows past the cap are counted with `Layer::holds`, not
  `row_fields`; rows before it use `row_fields` and skip those with none.
- **Pattern**: a row that is held but unreadable would count as omitted; that
  only arises for layers with no CPU data, which are not readable anyway.

### Architectural Decisions

#### Alt text as the refresh trigger

- **Decision**: rebuild on a changed alt text, as specified.
- **Trade-off**: a changed value in an existing row (no count change) is not
  seen; rows only change by append or eviction, both of which change the
  description's count or the chart.

### Development Workflow Insights

- The agent sandbox refuses heredocs combined with other commands; write helper
  scripts with the Write tool.

### Follow-up Stories

None.
