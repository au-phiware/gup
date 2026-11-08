---
implementer: opus
implementer-reason:
  changes the column store's CPU layout (shared and owned columns in one store)
  that GUP-664 made all-or-nothing; touches the chunk writer, upload and every
  CPU reader
touches:
  [
    crates/gup-core/src/column.rs,
    crates/gup-core/src/encoding.rs,
    crates/gup-core/src/selection.rs,
  ]
---

# GUP-674: Shareable Key Codes

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-08

## Context

GUP-629 maps a `KeyColumn` into the scale's dictionary and gathers the codes
into an owned `Vec<u32>`. GUP-664 keeps a store either all shared columns or all
owned ones, so a columnar layer with a key column (every categorical layer has
numeric x or y beside it) takes the owned path for all its columns. At 10M rows
with 50 labels (`scale_bench --columns f64 --keys 50`), the first resolve is
about 205 ms against 69 ms for the numeric columns alone, and the peak RSS is
376 MiB against 184: the chunks keep a CPU copy of x, y and the codes, and the
mapped codes vector is alive beside them.

## User Story

> "As a developer with categorical columns at 100M rows, I want the key column
> to cost no more memory than its source codes, so that a coloured scatter loads
> as fast as an uncoloured one."

## Acceptance Criteria

- [ ] A store can hold shared numeric columns and a key column's codes without a
      CPU copy of the numeric columns (a CPU layout per column, or codes read
      through the label table at upload).
- [ ] The first resolve of `--columns f64 --keys 50` at 10M rows is within 25%
      of `--columns f64` alone, and its peak RSS is at most the numeric run's
      plus 4 B per row.
- [ ] Images, picking and announcements are identical to GUP-629's.

## Technical Tasks

- [ ] Decide the layout: per-column CPU buffers in `Chunk`, or encode codes from
      `source code -> scale code` at upload time (the table is the dictionary's
      size), which keeps the code column shared.
- [ ] `Cells`, `SharedColumns::validity` (plane `p` is column `p` today) and
      `ColumnSlice::Codes` in shared stores.

## Dependencies

### Prerequisite Stories

- GUP-629: Key Columns ✅; GUP-664 ✅.

## Testing Strategy

- GUP-629's `keys` tests unchanged; `scale_bench` numbers as above.

## Success Metrics

- [ ] See the acceptance criteria.

## Risk Assessment

- **Medium**: touches the store's readers. _Mitigation_: byte-identical
  comparison with GUP-629's path.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test -p gup-core -- --test-threads=1` passes
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask perf-budget` table in the story
- [ ] Retrospective added to story document
