---
implementer: sonnet
implementer-reason:
  extends GUP-628's Column with a dictionary-coded variant and remaps it into
  the scale's dictionary like the owned-key path does; API settled in Decisions
touches:
  [
    crates/gup-core/src/columns.rs,
    crates/gup-core/src/encoding.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/tests/columns.rs,
  ]
---

# GUP-629: Key Columns

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-08) **Created**: 2026-11-04

## Context

GUP-628 adds numeric columns (`Column::F64`, `Column::F32`) and columnar layers.
Category data (a species, a region, a sensor id) still needs rows and a key
closure: Band and Point axes and `Categorical` colours read dictionary codes,
which the row path builds by running a key accessor per row through the scale's
dictionary (`KeyEncoded`, `encoding.rs` around line 1205;
`Dictionary::encode_borrowed`, `column.rs` line 252). Columnar sources already
hold categories dictionary-coded (Arrow `Dictionary` arrays, Polars
`Categorical`), so the natural column is codes plus labels; mapping it into the
scale's dictionary costs one lookup per distinct label and a gather per row, not
a hash per row.

## Read First

- GUP-628's story file ("Decisions") and its RFC-001 findings section.
- `crates/gup-core/src/columns.rs` (GUP-628).
- `crates/gup-core/src/encoding.rs`: `KeyEncoded` and its
  `DynColumnEncoding::evaluate` (around lines 1200–1230), `DictionarySlot` (line
  747).
- `crates/gup-core/src/column.rs`: `Dictionary` (around lines 195–260),
  `NULL_CODE` (line 146).

## User Story

> "As a developer with categorical columns, I want to colour and position marks
> by a coded column, so that categories at scale need no per-row closure or
> string hashing."

## Acceptance Criteria

### AC1: The column

- [x] `Column::Keys(KeyColumn)` and `KeyColumn` with the API of Decisions,
      re-exported at the root; `From<KeyColumn> for Column`.
- [x] `KeyColumn::new` rejects a code ≥ `labels.len()` (other than `NULL_CODE`)
      and duplicate labels, each with an error naming the problem.
- [x] `KeyColumn::from_labels(["a", "b", "a"])` gives codes `[0, 1, 0]` and
      labels `["a", "b"]`; `from_optional_labels` maps `None` to `NULL_CODE`.

### AC2: Through key scales

- [x] `x_column`/`y_column` accept a `Column::Keys` on a Band or Point axis, and
      return an error at resolve for a numeric column on a key axis or a key
      column on a numeric axis, naming the channel and both kinds.
- [x] `Categorical`'s `encode_column` accepts `Column::Keys`; a numeric column
      there is an error at resolve.
- [x] Remapping: each distinct label is encoded once into the scale's dictionary
      (first-seen order of the _column's codes as they appear in rows_, so
      categories keep the row path's first-seen order), then each row's code is
      gathered; nulls stay null (`theme.null_color`, no mark on a key axis, as
      on the row path).
- [x] Identical images: a 10,000-row scatter with a Point x axis of 12 labels
      and a `Categorical` colour of 5 labels, from rows with key closures and
      from `Columns`, byte-equal; also the legends and the band tick labels.
- [x] A column whose `labels` list contains a label no row uses does not add it
      to the scale's domain (matches the row path, where unused keys never
      appear).

### AC3: Cost

- [x] `scale_bench --columns f64 --keys 50` (new option: a 50-label colour key
      column) at 10M rows: record first-resolve time against the rows path with
      a `String` key closure and with a `&'static str` key closure.

## Decisions

- **Type**:

  ```rust
  #[derive(Clone, Debug)]
  pub struct KeyColumn { /* codes: Arc<[u32]>, labels: Arc<[Arc<str>]> */ }
  impl KeyColumn {
      pub fn new(codes: impl Into<Arc<[u32]>>, labels: impl Into<Arc<[Arc<str>]>>) -> Result<Self>;
      pub fn from_labels<S: AsRef<str>>(labels: impl IntoIterator<Item = S>) -> Self;
      pub fn from_optional_labels<S: AsRef<str>>(labels: impl IntoIterator<Item = Option<S>>) -> Self;
      pub fn codes(&self) -> &[u32];
      pub fn labels(&self) -> &[Arc<str>];
      pub fn len(&self) -> usize;
      pub fn is_empty(&self) -> bool;
  }
  ```

  `NULL_CODE` (already public in `column`) marks a null row.

- **First-seen order**: walk the codes in row order and encode each label into
  the scale's dictionary the first time its code appears; this reproduces the
  row path's order exactly (the identical-image test depends on it).
- **Remap table**: a `Vec<u32>` of `labels.len()` scale codes, filled lazily as
  codes are first seen; unused labels never reach the dictionary.

## Escalation Triggers

- The scale's dictionary cannot be reached from a `DynColumnEncoding<()>` impl
  the way `KeyEncoded` reaches it (the hook is typed by the accessor).
- The identical-image test differs.

## Out of Scope

- Arrow dictionary arrays (GUP-630 converts them into `KeyColumn`).
- String columns stored as strings (only coded).

## Technical Tasks

- [x] `KeyColumn` and `Column::Keys`.
- [x] Key handling in `x_column`, `y_column` and `encode_column`.
- [x] Tests (AC1, AC2), bench option and run (AC3).

## Dependencies

### Prerequisite Stories

- GUP-628: Columnar Layers 📋.

### Enables Stories

- GUP-630: Arrow dictionary and string arrays become `KeyColumn`s.

## Testing Strategy

- **Unit tests**: constructors and validation.
- **Integration tests**: identical images, legends, tick labels; error messages.
- **Performance**: AC3.

## Success Metrics

- [x] A columnar scatter coloured by category matches the row path pixel for
      pixel.

## Risk Assessment

- **Low**: follows GUP-628's pattern and the existing dictionary hook.
  _Mitigation_: byte-identical comparison.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, identity is tested
- [x] `mask wasm-browser`: not needed, no browser path changes
- [x] WASM size: `mask wasm-size` scatter gz bytes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `KeyColumn` (codes plus labels, validated: a code past the labels and a
  repeated label are errors naming the row or label) and `Column::Keys`, in
  `columns.rs`, re-exported at the root. `from_labels` and
  `from_optional_labels` code in first-seen order.
- `encode_column` is no longer bounded to `Input = f32`: `ColumnEncoded<S>` has
  a second `IntoEncoding` (`KeyMarker`, for `Input = u32`) next to the numeric
  one. The key one builds a crate-private `KeysEncoding`, which holds the
  `Column`, the scale as a `Box<dyn DynEncodeFn>` (new `dyn_fit_dictionary`) and
  the scale's `Dictionary` (found through `DictionarySlot`, as `KeyEncoded`
  does), so one type serves a `Categorical` colour and a Band or Point axis.
- `x_column`/`y_column` on a key axis: `bind_axes` builds a `KeysEncoding` from
  the plot's scale (`axis_column_encoding`); a numeric column on a key axis, or
  a key column on a numeric one, is an error naming the channel and both kinds.
  A kind mismatch on `encode_column` is reported by the layer's `column_inputs`
  the same way.
- Remap: a table of the scale's code per label, filled lazily while walking the
  rows (so first-seen order is the row path's and an unused label never reaches
  the dictionary), then one gather per row; nulls stay `NULL_CODE`.
- Three hidden, defaulted `DynColumnEncoding` methods (`columnar`,
  `column_value`, `kind_mismatch`) replace uses of `shared().is_some()` for
  "needs no rows" and "read a row by data row", so a released key layer still
  re-evaluates and announces its rows.
- Tests: `tests/columns.rs` `keys` module (4 tests: a byte-equal 10,000-row
  scatter with a 12-label Point x and a 5-label Categorical colour, nulls in
  both, labels not in first-seen order and one unused label, comparing the
  image, tick and legend text and both scales' keys; constructors; kind
  mismatches; a released layer's `row_fields`). The `compile_fail` snapshots
  `bare_number` and `closure_on_color` were re-blessed (a new `IntoEncoding`
  impl, as the test's docs say). Doc test on `KeyColumn`. `gup-core` tests: 923
  passed, 0 failed.
- `scale_bench --keys N [--key-closure str|string]`.

### AC3: first resolve at 10M rows, 50 labels

`scale_bench` (release, seed 1, sorted, 1920 x 1080, `Retain::Auto`; Intel HD
630). Three runs each, same session, load average 5-7 from other agents; rows
are `(x, y, category)`.

| path                                    | first_resolve ms (3 runs) | peak RSS MiB |
| --------------------------------------- | ------------------------- | ------------ |
| f64 columns, no key (reference)         | 88.6, 68.7, 68.7          | 184          |
| f64 columns + `KeyColumn`               | 240.7, 205.4, 200.7       | 376          |
| rows, `&'static str` key (`encode_key`) | 485.7, 462.7, 426.6       | 567          |
| rows, `String` key (owned key)          | 675.4, 685.3, 657.7       | 567          |

Median: the key column is 205 ms, 44% of the `&'static str` closure (463) and
30% of the `String` closure (675).

A key layer takes the copy path: it is 130 ms and 192 MiB above the numeric
columns alone (the chunks keep a CPU copy of x, y and the codes, plus the mapped
codes vector), because a store is all shared or all owned (GUP-664).

### WASM

`mask wasm-size`, same session: main (5f503867) scatter 376,604 B gz, branch
377,833 (+1,229); `per_row_type` 780 to 1,525. Within the recorded +2% and under
the 1.5 KB bar, so not trimmed.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Key columns cannot be shared the way numeric columns are

- **Challenge**: GUP-664 stores a layer's columns in place, so a numeric column
  costs no CPU copy. A key column's source codes are not the scale's: the scale
  numbers labels in the order its rows first use them, and the dictionary can be
  shared by other layers.
- **Solution**: the mapped codes are an owned `Vec<u32>`, so a layer with a key
  column takes the owned path for every column (GUP-664's "all shared or all
  owned"). The cheap parts stay cheap: the gather is a table lookup per row, and
  `column_value` reads a row's code through the label table with no per-row
  copy, so a released layer still announces rows and re-evaluates (it needs no
  rows).
- **Pattern**: a column whose stored form depends on mutable shared state (a
  dictionary) cannot be read in place; make the dependency a small table and
  keep the per-row work to a gather.

#### One `KeysEncoding` for colours and axes

- **Challenge**: the axis path erases the scale (`Box<dyn DynEncodeFn>`), while
  `ColumnEncoded<S>` is generic.
- **Solution**: `ColumnEncoded<S>`'s key `IntoEncoding` boxes its function; the
  axis path boxes the plot's scale. `DynEncodeFn` gained `dyn_fit_dictionary`.
- **Pattern**: when two entry points differ only in how the function is held,
  erase it once, at the entry.

### Architectural Decisions

#### Kind errors are reported by the layer, not the encoding

- **Decision**: `DynColumnEncoding::kind_mismatch` returns `(expected, given)`;
  `column_inputs` builds the message with the channel's name.
- **Reasoning**: `into_encoding` cannot fail and the encoding does not know its
  channel.
- **Trade-off**: `encode_column` accepts any column at compile time; the error
  comes at resolve, as the story's AC says.

### Development Workflow Insights

- The worktree sandbox refuses compound shell commands that it cannot prove stay
  in the worktree (some mentions of the repository path or tool names); split
  commands and write helper scripts with the Write tool.
- A new `IntoEncoding` impl re-blesses two `compile_fail` snapshots, as the
  test's header predicts.

### Follow-up Stories

1. **GUP-674: Shareable key codes** — let a store hold mapped key codes beside
   shared numeric columns (a CPU layout that is per column), so a keyed columnar
   layer keeps the no-copy path (now 130 ms and 192 MiB above numeric at 10M
   rows). Written up in `GUP-674_Shareable_Key_Codes.md`.
