---
implementer: opus
implementer-reason:
  changes what a chunk keeps on the CPU for a columnar layer and every reader of
  CPU columns (picking, the edge solve, vector output, another context); design
  work with a measured target
touches:
  [
    crates/gup-core/src/column.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/examples/scale_bench.rs,
    crates/gup-core/tests/columns.rs,
  ]
---

# GUP-664: Shared Columns as the CPU Copy of a Columnar Layer

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-08

## Context

GUP-628 made columnar layers (`Plot::add_columns`): the user's `Arc`'d columns
are written straight into the column store's chunks, with no closure and no f64
copy. Two costs of the row design remain, both measured in GUP-628's findings
(RFC-001, "GUP-628 findings"):

1. **The chunk CPU copy.** Each chunk still allocates its CPU bytes (8 B a row
   for two `F32Relative` columns) and fills them before the upload. At 10M rows
   that is 76 MiB of the 261 MiB peak, and about 60 of the 93 ms first resolve
   (23 ms of page faults on the fresh buffer, about 28 ms of f64-to-f32
   conversion, the rest stats and validity). For a columnar layer the shared
   column already is a CPU copy: the chunk's bytes could be produced into the
   upload's staging memory and dropped (or never kept), and every later reader
   could decode from the shared column instead.
2. **Readers after release.** Under `Retain::Auto` above 10M rows (and under
   `GpuOnly`) a columnar layer releases its `()` rows and its CPU chunk columns,
   so `pick_row` finds nothing and re-encoding a channel is an error ("its rows
   were dropped"), although the shared columns are still held by the layer's
   encodings. `row_fields` already reads the shared columns (GUP-628), the other
   readers do not.

GUP-632 (100M rows) needs both: at 100M rows the chunk copy is 760 MiB and the
layer is far above the `Auto` limit.

## User Story

> "As a developer drawing 100M points from columns, I want Gup to keep no second
> CPU copy of my columns and still pick and re-encode, so that memory is my
> columns plus the GPU's, and interaction works at any size."

## Acceptance Criteria

- [ ] A columnar layer keeps no chunk CPU bytes after its upload under every
      `Retain` policy, and its peak RSS at 10M rows
      (`scale_bench --columns f64     --resolve-only`-equivalent) drops by at
      least 70 MiB from GUP-628's 261 MiB, with the same image bytes (the AC2
      tests of `crates/gup-core/tests/columns.rs` still pass).
- [ ] `pick_row`, re-encoding a channel, resolving on a second context and SVG
      output work for a columnar layer above `Retain::AUTO_MAX_ROWS` rows (a
      test with a lowered limit or `GpuOnly` plus `max_chunk_rows`), reading the
      shared columns, without copying a whole column per pick.
- [ ] First resolve of f64 columns at 10M rows at most 45% of the rows path's,
      or the reason recorded with the phase numbers.
- [ ] Row layers are unchanged (same tests, same `scale_bench` rows numbers
      within noise).

## Technical Tasks

- [ ] A chunk source for shared columns: the store records which columns are
      shared (an index into the layer's inputs) and encodes their rows on demand
      (upload, pick, edge solve, vector output) rather than holding bytes.
- [ ] Upload from the shared column straight into the staging write (no CPU
      chunk buffer), keeping stats and validity as today.
- [ ] Readers (`cpu_values_where`, the edge cache, `read_stored`, picking's
      column read) fall back to the shared column when a chunk has no bytes.
- [ ] `scale_bench --columns` rows at 10M and 30M beside GUP-628's.

## Dependencies

### Prerequisite Stories

- GUP-628: Columnar Layers ✅

### Enables Stories

- GUP-632 (100M demonstration): the memory and picking it needs.

## Testing Strategy

- **Integration**: AC2's identity tests, plus picks and re-encoding above the
  release threshold.
- **Performance**: `scale_bench --columns f64|f32` at 10M and 30M.

## Success Metrics

- [ ] A 10M-point columnar scatter peaks at its columns plus under 20 MiB.

## Risk Assessment

- **Medium**: chunks without CPU bytes are a new state for every reader of the
  store. _Mitigation_: a single "bytes of chunk k, column c" accessor that every
  reader goes through, with the shared column behind it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-size` recorded
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
