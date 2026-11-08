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

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-08) **Created**: 2026-11-08

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

- [x] A columnar layer keeps no chunk CPU bytes after its upload under every
      `Retain` policy, and its peak RSS at 10M rows
      (`scale_bench --columns f64     --resolve-only`-equivalent) drops by at
      least 70 MiB from GUP-628's 261 MiB, with the same image bytes (the AC2
      tests of `crates/gup-core/tests/columns.rs` still pass). _A store of
      shared columns never holds chunk bytes or validity bits; 264 to 185 MiB
      (-79) at 10M, same-session A/B; the identity tests pass._
- [x] `pick_row`, re-encoding a channel, resolving on a second context and SVG
      output work for a columnar layer above `Retain::AUTO_MAX_ROWS` rows (a
      test with a lowered limit or `GpuOnly` plus `max_chunk_rows`), reading the
      shared columns, without copying a whole column per pick. _Met with one
      bound: the CPU pick index takes a released columnar layer's rows while its
      visible (culled) rows are at most `Retain::AUTO_MAX_ROWS`; above that the
      plot picks it on the GPU, as it does a released row layer (see
      Retrospective). `row_fields` reads one row, not a chunk._
- [x] First resolve of f64 columns at 10M rows at most 45% of the rows path's,
      or the reason recorded with the phase numbers. _82.0 of 212.2 ms: 39%._
- [x] Row layers are unchanged (same tests, same `scale_bench` rows numbers
      within noise). _223.8 vs 212.2 ms and 415 MiB both at 10M; 767 vs 766 ms
      and 1182 MiB both at 30M._

## Technical Tasks

- [x] A chunk source for shared columns: the store records which columns are
      shared (an index into the layer's inputs) and encodes their rows on demand
      (upload, pick, edge solve, vector output) rather than holding bytes. _It
      holds the `Column`s themselves (`Arc` clones)._
- [x] Upload from the shared column straight into the staging write (no CPU
      chunk buffer), keeping stats and validity as today.
- [x] Readers (`cpu_values_where`, the edge cache, `read_stored`, picking's
      column read) fall back to the shared column when a chunk has no bytes.
- [x] `scale_bench --columns` rows at 10M and 30M beside GUP-628's. _No bench
      change was needed._

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

- [x] A 10M-point columnar scatter peaks at its columns plus under 20 MiB. _2
      MiB above the process holding the columns (`rss.build`, which also holds
      about 30 MiB of driver and font state)._

## Risk Assessment

- **Medium**: chunks without CPU bytes are a new state for every reader of the
  store. _Mitigation_: a single "bytes of chunk k, column c" accessor that every
  reader goes through, with the shared column behind it.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] `mask wasm-size` recorded
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

A columnar layer's column store reads the layer's shared columns instead of
copying them (`crates/gup-core/src/column.rs`):

- **`ColumnStore::append_shared(rows, Vec<Column>)`** (crate-private) fills an
  empty store whose chunks take only origins and stats (one pass of `scan`
  without validity bits) and never hold a CPU copy. `append` on it is an error.
  A window's eviction moves the store's column base.
- **Upload** encodes each chunk's rows from the shared column straight into
  wgpu's staging memory through a new counted
  `Context::write_buffer_with(kind, buffer, offset, size, fill)`; validity bits
  are computed from the column when the store has a null. Shared chunks are
  always uploadable, so a second context needs no rows.
- **One reader.** Every CPU read goes through `ColumnStore::cells(k, column)`,
  which returns a chunk's bytes or the shared rows (`Cells`), decoded exactly as
  the GPU sees them (`as_stored`: origin plus the f32 offset, plus the low part
  for `F32x2Relative`). `cpu_values_where`, `chunk_values` (the edge cache), the
  SVG snapshot (`CpuColumns`) and a new single-row `ColumnStore::value` (used by
  `read_stored`, so `row_fields` decodes one row instead of a chunk, for row
  layers too) read through it.
- **Layer** (`selection.rs`): a non-generic `fill` chooses `append_shared` when
  every input is a shared column (no size order, no monotone controls, at least
  one column). A layer whose channels all read shared columns re-evaluates
  without its rows (re-encoding after `Retain` dropped them). SVG output reads
  the shared columns whatever the policy. `cpu_readable` (whether the plot picks
  on the CPU) keeps its meaning: for shared columns, as far as the policy keeps
  CPU columns. `hit_items` indexes a released columnar layer's rows while its
  visible rows are at most `Retain::AUTO_MAX_ROWS`.
- `Column::slice` (`columns.rs`); the cell encoder, the f32 widening loop and
  the write path are shared by owned and shared chunks.

**Tests**: `column::tests::shared_columns_upload_and_read_as_owned_ones` (a
shared store against an owned one: stats, origins, decoded values, single rows,
GPU bytes and validity with NaN and infinity, release, a second context);
`tests/columns.rs::a_released_columnar_layer_reads_its_shared_columns`
(`GpuOnly` with 1024-row chunks: picks, `row_fields` and image identical to a
rows layer, then a re-encoded channel, another context and the SVG document).
`cargo test -p gup-core` 906 passed.

**Measurements** (`scale_bench`, release, sorted, 1920 x 1080, Intel HD 630,
Vulkan; medians of 5 alternating base/branch pairs in one session, load 3-7;
base is 6e3739f0; ms and MiB):

| rows | path        | base first_resolve | branch first_resolve | base peak RSS | branch peak RSS |
| ---- | ----------- | ------------------ | -------------------- | ------------- | --------------- |
| 10M  | rows        | 223.8              | 212.2                | 415           | 415             |
| 10M  | f64 columns | 117.1              | 82.0 (39%)           | 264           | 185             |
| 10M  | f32 columns | 115.7              | 87.0 (41%)           | 188           | 109             |
| 30M  | rows        | 767.4              | 766.0                | 1182          | 1182            |
| 30M  | f64 columns | 409.3              | 306.5 (40%)          | 727           | 490             |
| 30M  | f32 columns | 446.2              | 357.1 (47%)          | 498           | 262             |

Percentages are of the branch's rows path. Peak RSS is now `rss.build` + 2 MiB
in every columnar row: the columns and nothing else. An earlier, quieter batch
(3 pairs, before the WASM trim) gave 73.9 vs 196.6 ms at 10M (38%) and 256 vs
648 ms at 30M (40%), RSS identical. 100M was not run: `MemAvailable` was 4-10
GB, under the 20 GB it needs.

**WASM** (`mask wasm-size`, same session, base sources checked out in the
worktree): base 373,119 B gz, branch 374,820 B gz (+1,701 B). The first version
was +3,014 B; moving the store fill out of the generic `evaluate`, one write
path instead of a write and a measure path, one `dyn` f32 widening loop and one
row decoder trimmed it. `wasm.per_row_type.gz_bytes` fell from 1,917 to 1,569.

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### Decode by formula, not by bytes

- **Challenge**: readers of a chunk without bytes must return exactly what the
  bytes would have decoded to, or the identity tests (image, picks, fields)
  break.
- **Solution**: `as_stored(format, origin, v)` repeats the writer's arithmetic
  (`origin + f64::from((v - origin) as f32)`, plus the low part) in the same
  order as the byte decoder, and validity is `v.is_finite()`, which is what
  `scan` sets. The unit test compares bit patterns of every value, single rows,
  GPU bytes and validity words against an owned store.
- **Pattern**: when replacing a stored form by recomputation, test the two paths
  bit for bit against each other, not against expected numbers.

#### A vacuous "all shared"

- **Challenge**: the full suite failed in `rect_png`: a layer of constants has
  no columns, so "every input is shared" held and `append_shared` stored 0 rows.
- **Solution**: share only when there is at least one column, and pass the row
  count instead of reading it from the first column.
- **Pattern**: `all` over a possibly empty set needs a non-empty guard; run the
  whole suite before trusting the focused tests.

#### Generic code is paid per row type in WASM

- **Challenge**: the first version added 3,014 B gz to the reference scatter,
  which never draws a columnar layer.
- **Solution**: `twiggy diff` of the two bound builds (names kept) showed the
  shared branch inside `evaluate` (inlined into six `fit_domains`
  instantiations), a separate measure path and one f32 widening loop per
  closure. A non-generic `fill`, one write path with an optional CPU copy and a
  `&mut dyn FnMut` widening loop brought it to +1,701 B.
- **Pattern**: keep new non-row-type logic out of `Selection<T, M>` methods;
  `twiggy diff` on the bound `_bg.wasm` finds the culprits in one step.

### Architectural Decisions

#### All shared or all owned, per store

- **Decision**: a store is filled either from shared columns only (no CPU copy)
  or as before. A layer with a size order, monotone controls or any accessor
  column keeps the owned path.
- **Reasoning**: per-column sharing would need a CPU layout separate from the
  GPU layout and would change what the public `Chunk::bytes()` means; today
  every columnar channel is a shared numeric column.
- **Trade-off**: a mixed layer (GUP-629's key columns beside numeric ones)
  copies everything, as before this story.
- **Future**: GUP-629 should either keep codes in a per-column CPU buffer or
  make keys shareable; `Cells` already reads per column.

#### CPU picking keeps its threshold

- **Decision**: `cpu_readable` (CPU or GPU picking for hover) follows the
  `Retain` policy for shared columns, and `hit_items` indexes a released
  columnar layer only while its visible rows are at most
  `Retain::AUTO_MAX_ROWS`.
- **Reasoning**: in GPU-pick mode the plot still builds the CPU index for
  anchors (`set_hover`, `sync_overlay`); indexing every row of a 100M layer
  there would cost gigabytes and seconds on the first hover, where a released
  row layer costs nothing. Below the bound, `pick_row` works for any policy.
- **Trade-off**: `pick_row`, the marquee brush and keyboard focus return nothing
  for a columnar layer with more than 10M visible rows (hover and click use the
  GPU pick).
- **Future**: GUP-667.

#### Upload through `write_buffer_with`

- **Decision**: a counted `Context::write_buffer_with` lets the chunk encoder
  write into wgpu's staging memory.
- **Reasoning**: no CPU buffer per chunk; the conversion moves into the upload
  and the page faults on fresh chunk buffers disappear. Staging did not show in
  peak RSS on this machine (branch peak = `rss.build` + 2 MiB).

### Development Workflow Insights

- Machine load from other agents changed absolute times by up to 20% between
  batches; five alternating base/branch pairs per configuration gave stable
  ratios where three did not.
- The worktree sandbox refuses compound commands that `cd` or loop around
  binaries; one plain command per configuration, with logs under
  `~/.cache/gup/gup664`, worked.
- Measuring the base WASM by checking the base commit's `crates/gup-core/src`
  out in the worktree and back was quick, but the base must be the branch's
  merge base (6e3739f0), not current `main`, which had moved.

### Follow-up Stories

1. **GUP-667: Interaction Above the CPU Pick Bound** — `pick_row`, the marquee
   brush and keyboard focus for layers whose visible rows exceed
   `Retain::AUTO_MAX_ROWS`, without a CPU index of every row. GUP-632's keys and
   brush at 100M need it.

Proposed `PERF_BUDGETS.md` rows (for the orchestrator): columnar first resolve
at 10M rows (82 ms in this session, +20%) and its peak RSS (185 MiB, +5%) from
`scale_bench --columns f64`, replacing GUP-628's proposal (93 ms, 261 MiB); and
`wasm.scatter.gz_bytes` re-recorded when this merges (374,820 B here, before
merging current `main`).
