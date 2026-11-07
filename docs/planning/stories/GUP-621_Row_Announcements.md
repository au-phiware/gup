---
implementer: sonnet
implementer-reason:
  the text format, field rules and API are settled in Decisions; the value read
  follows the SVG path's CPU-column read, and three builders add a small hook
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/chart.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/scale/mod.rs,
    crates/gup-core/src/scale/time.rs,
    crates/gup-core/src/builder/,
    crates/gup-core/tests/keyboard.rs,
  ]
---

# GUP-621: Row Announcements

## Story Overview

**Initiative**: Track N4: Accessibility **Status**: ✅ Complete (2026-11-07)
**Created**: 2026-11-04

## Context

After GUP-619 a keyboard user can focus a row, but a screen reader has nothing
to say about it: the tooltip is pixels, and many layers have no tooltip. The
browser host (GUP-623) will announce the focused row through an `aria-live`
region, and the hidden data table (GUP-624) needs the same values as cells. Both
need one source of truth: the row's values, as text, in the axes' own formats.

The values are available. A layer keeps its CPU columns under `Retain::Auto` up
to 10M rows, and the SVG path already reads a row's raw channel values from them
(`Selection`'s vector path, `selection.rs` around lines 1380–1420:
`cpu_snapshot`, `VChan::Column`). Key columns hold dictionary codes whose labels
the scale's dictionary has. Builders know more than the columns for three chart
types: a stacked bar's value is not its top edge, a histogram bin is a range
plus a count, and a box is a five-number summary.

## Read First

- `crates/gup-core/src/selection.rs`: the `Layer` trait (line 1191),
  `tooltip_text` (line 2468) and the vector path around lines 1380–1420.
- `crates/gup-core/src/column.rs`: `cpu_snapshot` (line 1349), `CpuColumns`, and
  how a chunk's relative column stores `value − origin`.
- `crates/gup-core/src/describe.rs` lines 100–125 (`sig3`).
- `crates/gup-core/src/scale/mod.rs`: `PositionScale` (around line 110) and
  `DynPositionScale` (line 385).
- `crates/gup-core/src/chart.rs` (the trait, 135 lines).
- GUP-619's and GUP-615's story files, "Decisions".
- Builders: `bar.rs` `split` and `materialise` (around lines 590–700),
  `histogram.rs` `HistogramBin`, `boxplot.rs` `BoxSummary` and its layers.

## User Story

> "As a screen-reader user, I want the focused data point read out as its values
> with their axis names, so that I learn what the point is without seeing it."

## Acceptance Criteria

### AC1: Row fields

- [x] `Plot::row_fields(&self, row: RowId) -> Option<Vec<(String, String)>>`
      returns `(name, value)` pairs by the rules in Decisions; `None` for a row
      that does not exist or whose layer kept neither rows nor CPU columns.
- [x] Tested in `crates/gup-core/tests/keyboard.rs` (or a new
      `tests/row_text.rs`) for: a scatter with axis titles (names are the
      titles), one without (names `x` and `y`), a `Time` x axis (an ISO date), a
      colour key (a `Series` field), a heatmap (two keys and the value), a
      stacked bar segment (its own value, not its top), a histogram bin (`Bin`
      `10–20` and `Count`), a box (the five numbers and the outlier count), and
      an x axis with `x_tick_format(|v| format!("${v:.0}"))` (the value uses
      it).

### AC2: Row text and focus text

- [x] `Plot::row_text(&self, row) -> Option<String>`: the layer's tooltip with
      line breaks replaced by `"; "`, if it has one; else the fields joined as
      in Decisions.
- [x] `Plot::focus_text(&self) -> Option<&str>`: `row_text` of the focused row
      plus the position suffix, `None` without keyboard focus. Recomputed when
      focus moves and on each full resolve while focused (an append can change
      `n`).
- [x] `Chart::focus_text(&self) -> Option<&str>` exists with a default of
      `None`; `Plot` and all seven builders implement it by delegation.
- [x] A doctest on `Plot::focus_text` focuses the second of three rows with
      `Input::key(Key::Right)` twice and asserts the exact string, e.g.
      `"Mass: 0.9; Span: 0.95 (2 of 3)"`.

## Decisions

- **Field order**: series (if any), x, y, then the colour value (if the layer
  encodes colour from a numeric column, for example a heatmap or a scatter's
  `color_value`).
- **Names**: x is the plot's x axis title, else `"x"`; y is the title of the
  axis the layer's y is bound to (left or right), else `"y"`; the series and
  colour fields use the legend title of the colour encoding, else `"Series"` and
  `"Value"`.
- **Values**: read the row's raw input value of the mark's first `X`-role and
  first `Y`-role channels and its `Color`-role column from the CPU columns
  (relative columns plus their chunk origin, in f64). A key column gives its
  key's label. A number is formatted by the axis's effective tick format
  (GUP-615's `label_formats`) when one is set, else by a new
  `PositionScale::value_label(&self, v: f64) -> String` (default:
  `describe::sig3(v)`; `Time` overrides it with ISO 8601 in UTC, `YYYY-MM-DD`
  when the time is midnight, else `YYYY-MM-DDTHH:MM`, adding `:SS` only when
  seconds are non-zero). The colour value uses `sig3`.
- **Series**: the row's colour key label when the colour is a key column; else a
  crate-private `Selection::series_name(Arc<str>)` set by builders that split
  rows into one layer per key (line and area by `color`/`stack`, bar by
  `group`/`stack`, histogram by `color`); else no series field.
- **Builder hooks**: a crate-private
  `Selection::row_fields_with(Arc<dyn Fn(&T) -> Vec<(String, String)> + Send + Sync>)`
  replaces the generic fields when set. Bar:
  `[series?, (band axis name, key), (value axis name, value)]` with the row's
  own value from the builder's value accessor. Histogram:
  `[series?, ("Bin", "lo–hi"), ("Count", n)]` using an en dash and the x axis's
  format for `lo` and `hi`. Box plot boxes:
  `[(band axis name, key), ("Median", m), ("Q1", q1), ("Q3", q3), ("Whiskers", "lo–hi"), ("Outliers", n)]`;
  outlier discs: `[(band axis name, key), (value axis name, v)]`.
- **Joined text**: `"{name}: {value}"` joined with `"; "`. Position suffix
  `" ({k} of {n})"`, with `k` the 1-based place in the layer's focus order and
  `n` the number of focusable rows of that layer in view.
- **Retain**: a layer with rows but no CPU columns (`Retain::Rows`) evaluates
  the needed accessors for the one row from its rows; a layer with neither gives
  `None` (and its rows are not focusable anyway).

## Escalation Triggers

- A relative column's stored value plus its chunk origin does not reproduce the
  input value to within f32 precision of the chunk (the read is wrong).
- `PositionScale::value_label` as a trait method with a default breaks a
  user-facing impl (it is a provided method, so it should not).
- A builder's per-key layers do not map one-to-one to keys (then `series_name`
  needs another source).

## Out of Scope

- The browser's live region (GUP-623) and the data table (GUP-624).
- Announcing trends or summaries (the description, GUP-592, covers the chart).

## Technical Tasks

- [ ] `PositionScale::value_label` and the `Time` override.
- [ ] `Layer` methods for the generic fields and the builder hook;
      `series_name`.
- [ ] `Plot::row_fields`, `row_text`, `focus_text`; `Chart::focus_text` and its
      delegations.
- [ ] Builder hooks for bar, histogram and box plot.
- [ ] Tests (AC1, AC2) and the doctest.

## Dependencies

### Prerequisite Stories

- GUP-619: Keyboard Focus Over Rows 📋 — focus and the focus order.
- GUP-615: Value Labels Read the Axis Format 📋 — `label_formats`, the effective
  axis format.

### Enables Stories

- GUP-623 (announces `focus_text`), GUP-624 (the table's cells).

## Testing Strategy

- **Unit tests**: `Time::value_label` cases; the join and suffix.
- **Integration tests**: AC1's chart list, AC2's doctest.

## Success Metrics

- [ ] Every builder's focused row reads as names and values a sighted reader
      would read off the axes.

## Risk Assessment

- **Medium**: wide but shallow (seven builders, two traits). _Mitigation_: the
  generic path covers four builders; the hooks are one closure each.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, text only
- [x] `mask wasm-browser`: not needed yet (GUP-623 uses it); state so
- [x] WASM size: `mask wasm-size` scatter gz bytes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Plot::row_fields`, `row_text`, `focus_text`; `Chart::focus_text` (default
  `None`; Plot and the seven builders delegate). Focus text is refreshed at the
  end of `handle` and `resolve` (keyed by row and generation).
- `PositionScale::value_label` (default `sig3`; `Time` says ISO 8601 UTC);
  `DynPositionScale::{value_label, key_label}`.
- `Selection::series_name` and `row_fields_with`; bar, histogram and box plot
  hooks; line, area, bar and histogram name their series.
- Support: `DynColumnEncoding::row_value` (encoding.rs) reads one row from the
  rows when CPU columns are released; `DrawOrder::stored_of` (order.rs).
- Tests: `tests/row_text.rs` (15), `Time::value_label`, `stored_of`, two
  doctests. Rendered output unchanged (text only); `mask wasm-browser` not
  needed (GUP-623).
- WASM size (same session): main f9a55df5 scatter gz 366463 B; this branch
  369678 B (+3215 B gz, +16 KB raw). Attribution: `Layer::row_fields` 5.4 KB raw
  over 6 instantiations (after moving the logic into non-generic `compose_row`;
  it was 11.5 KB), about 7 KB raw of non-generic code, and 2.5 KB of rodata
  strings. No second float formatter: numbers go through `sig3`, `group_digits`
  and `typographic`, shared with the axis ticks. Not re-recorded.
- Numbers are said as the axis labels them: `number_label` (three significant
  digits, `,` groups when the axis ticks group, U+2212 minus), the same helpers
  `format_with_step` uses.

## Retrospective

**Completed**: 2026-11-07

### Architectural Decisions

#### Hook takes the axes

- **Decision**: `row_fields_with` takes
  `Fn(&T, &RowAxes) -> Vec<(String, String)>`, not `Fn(&T)`.
- **Reasoning**: a hook must name fields by the axis titles and format values
  with the axis format, both of which live in the plot and may change after the
  layer is built.
- **Trade-off**: differs from the Decisions text by one parameter
  (crate-private).

#### Small additions outside `touches`

- `encoding.rs` (`row_value`, needed for the Retain::Rows fallback through
  `&self`) and `order.rs` (`stored_of`, to read a size-ordered layer's column
  row) were edited; both additive.
- Key labels: an ordinal axis via its ticks at unlimited count; a colour key via
  its legend's swatches (code order), so no band.rs or categorical.rs edit.

### Key Technical Learnings

- `chunk_values` decodes a whole chunk per field read; fine per focus move, but
  a single-row decode would be cheaper at 10M rows.
- A stacked area reads its cumulative top edge (generic path); a hook like the
  bar's would give the own value.

### Follow-up Stories

None written (GUP-652 and GUP-655 unused).
