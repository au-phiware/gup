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

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-04

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

- [ ] `Column::Keys(KeyColumn)` and `KeyColumn` with the API of Decisions,
      re-exported at the root; `From<KeyColumn> for Column`.
- [ ] `KeyColumn::new` rejects a code ≥ `labels.len()` (other than `NULL_CODE`)
      and duplicate labels, each with an error naming the problem.
- [ ] `KeyColumn::from_labels(["a", "b", "a"])` gives codes `[0, 1, 0]` and
      labels `["a", "b"]`; `from_optional_labels` maps `None` to `NULL_CODE`.

### AC2: Through key scales

- [ ] `x_column`/`y_column` accept a `Column::Keys` on a Band or Point axis, and
      return an error at resolve for a numeric column on a key axis or a key
      column on a numeric axis, naming the channel and both kinds.
- [ ] `Categorical`'s `encode_column` accepts `Column::Keys`; a numeric column
      there is an error at resolve.
- [ ] Remapping: each distinct label is encoded once into the scale's dictionary
      (first-seen order of the _column's codes as they appear in rows_, so
      categories keep the row path's first-seen order), then each row's code is
      gathered; nulls stay null (`theme.null_color`, no mark on a key axis, as
      on the row path).
- [ ] Identical images: a 10,000-row scatter with a Point x axis of 12 labels
      and a `Categorical` colour of 5 labels, from rows with key closures and
      from `Columns`, byte-equal; also the legends and the band tick labels.
- [ ] A column whose `labels` list contains a label no row uses does not add it
      to the scale's domain (matches the row path, where unused keys never
      appear).

### AC3: Cost

- [ ] `scale_bench --columns f64 --keys 50` (new option: a 50-label colour key
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

- [ ] `KeyColumn` and `Column::Keys`.
- [ ] Key handling in `x_column`, `y_column` and `encode_column`.
- [ ] Tests (AC1, AC2), bench option and run (AC3).

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

- [ ] A columnar scatter coloured by category matches the row path pixel for
      pixel.

## Risk Assessment

- **Low**: follows GUP-628's pattern and the existing dictionary hook.
  _Mitigation_: byte-identical comparison.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, identity is tested
- [ ] `mask wasm-browser`: not needed, no browser path changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
