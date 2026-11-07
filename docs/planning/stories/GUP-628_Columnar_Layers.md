---
implementer: opus
implementer-reason:
  a new data path into the core (no per-row closures, no row type), touching
  Selection, encodings and the column store; the public shape is fixed below,
  the internals and the f32 fast path are design work with a measured rule
touches:
  [
    crates/gup-core/src/columns.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/encoding.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/examples/scale_bench.rs,
    crates/gup-core/tests/columns.rs,
  ]
---

# GUP-628: Columnar Layers

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-08) **Created**: 2026-11-04

## Context

Every row enters Gup as a value of the user's type `T` and every channel is a
closure over it (`Selection<T, M>`, `crates/gup-core/src/selection.rs` line 169;
`DynColumnEncoding::evaluate(&mut self, rows: &[T]) -> Result<ColumnData>`,
`encoding.rs` line 1095). At scale this costs three times over: the user builds
a `Vec<T>` (16+ bytes a row), each closure runs per row into a
`ColumnData::Values(Vec<f64>)` (8 bytes a row per channel), and the store then
converts to f32 chunks. Data that already lives in columns (a simulation's
arrays, an Arrow batch, a Polars frame) has to be zipped into rows first. The
review's track N5 starts with "columnar input reachable from `Plot`".

`ColumnStore::from_columns` exists (`column.rs` line 1049) but nothing connects
a store to a `Plot` layer. GUP-627 measures where the time and memory go at 1M
to 100M rows; this story uses those numbers.

The public shape below is fixed so that GUP-629 (key columns) and GUP-630
(Arrow) can be written against it. The internals, and whether an f32 input skips
the f64 intermediate, are this story's design work.

## Read First

- GUP-627's retrospective (the phase table and the paragraph naming the largest
  phase).
- `crates/gup-core/src/selection.rs`: `Selection` (line 169), `new` (line 338),
  `evaluate` (lines 757–835), `Retain` (lines 40–100).
- `crates/gup-core/src/encoding.rs`: `IntoEncoding` (line 1027), `Encoding`
  (line 1069), `DynColumnEncoding` (around line 1090) and its impls (lines
  1160–1330), including `AxisEncoding`.
- `crates/gup-core/src/column.rs`: `ColumnData` (line 285), `ColumnStore::new`,
  `append` (lines 1034–1180).
- `crates/gup-core/src/plot.rs`: `add` (around line 956), `circles` (line 855).
- RFC-001 §3 "Data model" (lines 149–258) and "S4a findings" (lines 2049–2200).

## User Story

> "As a developer with data in columns, I want to give Gup the columns directly,
> so that tens of millions of points load without building a row struct or
> running a closure per row."

## Acceptance Criteria

### AC1: Public API as decided

- [x] `Columns`, `Column`, `Plot::add_columns`, `Selection::<(), M>::x_column`,
      `y_column` and `ScaleRef::encode_column` exist with the signatures of
      Decisions, re-exported at the crate root (`Columns`, `Column`), with doc
      comments and a doctest that draws a scatter from two `Vec<f64>` columns
      and colours it through `Sequential::viridis().encode_column(..)`.
- [x] Errors: columns of unequal length (`Columns::with`), a missing name
      (`Columns::get` returns `None`; `Index` panics with the name), and a
      column longer or shorter than the layer (an error at resolve naming the
      channel).

### AC2: Same pictures

- [x] A test renders the same 10,000-row scatter (x, y, a numeric colour) from
      rows and closures and from `Columns`, and the two images are identical
      (byte-equal on the same device in the same run), as are their domains and
      `pick_row` results at 20 points.
- [x] The same for a line layer (`add_columns::<Line>`).
- [x] Appending is out of scope for columnar layers in this story; calling
      `appender()` on one is an error with a message saying so.

### AC3: Faster and smaller, by a measured rule

- [x] `scale_bench --columns f64|f32` (new option) builds the layer from
      `Columns` instead of rows. Record, at 10M and 30M rows (and 100M if
      GUP-627 could run it), first-resolve time and peak RSS for rows, f64
      columns and f32 columns, beside GUP-627's numbers.
- [x] Decision rule for the f32 path: if f32 columns are converted through an
      f64 intermediate and that conversion is more than 25% of first resolve at
      10M rows, add a path that writes f32 input straight into chunks (`F32`
      format directly; `F32Relative` subtracting the chunk origin in f32 only
      when the chunk's span keeps the error below the store's existing bound,
      else through f64). Record the rule's outcome.
- [x] Targets: f64 columns' first resolve at 10M at most half the rows path's,
      and peak RSS at most the rows path's minus the user rows' bytes. If a
      target is missed, record why with the phase numbers; missing it is not a
      failure of the story if the cause is outside this design (for example the
      GPU upload).

### AC4: Findings

- [x] A "GUP-628 findings" section appended to RFC-001: the design, the AC3
      table, the f32 rule's outcome, and proposed adjustments for GUP-629,
      GUP-630, GUP-631 and GUP-632.

## Decisions

- **Container** (`crates/gup-core/src/columns.rs`):

  ```rust
  /// Named columns of equal length.
  #[derive(Clone, Debug, Default)]
  pub struct Columns { /* len, Vec<(Arc<str>, Column)> */ }

  /// One column. Numbers: NaN is null. (`Keys` arrives in GUP-629.)
  #[derive(Clone, Debug)]
  #[non_exhaustive]
  pub enum Column { F64(Arc<[f64]>), F32(Arc<[f32]>) }

  impl Columns {
      pub fn new() -> Self;
      /// Add a column; `Err` if its length differs from the first column's.
      pub fn with(self, name: impl Into<Arc<str>>, column: impl Into<Column>) -> Result<Self>;
      pub fn len(&self) -> usize;
      pub fn is_empty(&self) -> bool;
      pub fn get(&self, name: &str) -> Option<&Column>;
      pub fn names(&self) -> impl Iterator<Item = &str>;
  }
  impl std::ops::Index<&str> for Columns { type Output = Column; }
  impl From<Vec<f64>> for Column {}  impl From<Vec<f32>> for Column {}
  impl From<Arc<[f64]>> for Column {} impl From<Arc<[f32]>> for Column {}
  impl Column { pub fn len(&self) -> usize; pub fn is_empty(&self) -> bool; }
  ```

  `Arc` so a column can feed several layers and channels without a copy.

- **Layer**:
  `Plot::add_columns<M: Mark>(&mut self, columns: &Columns) -> &mut Selection<(), M>`.
  The rows are `()` — a `Vec<()>` of `columns.len()` costs no memory — so every
  existing `Selection` method, `Retain`, culling, picking and SVG work
  unchanged. `pick_row` gives a `RowId` whose `row` is the column index;
  `pick::<()>` is useless and documented as such.
- **Positions**:
  `Selection::<(), M>::x_column(&mut self, column: &Column) -> &mut Self` and
  `y_column`, binding the mark's first `X`-role and `Y`-role channels to the
  plot's x and y axes exactly as the generated `.x(|r| ..)` setters do through
  `attr_axis`.
- **Other channels**:
  `ScaleRef<S>::encode_column(&self, column: &Column) -> ColumnEncoded<S>` (and
  the same on bare `EncodeFn` scales, as `encode` is), where `ColumnEncoded<S>`
  implements `IntoEncoding<(), V, ColumnMarker<f64>>`, so
  `layer.attr(Circle::FILL, viridis.encode_column(&cols["v"]))` works with the
  existing `attr`.
- **Evaluation**: a `DynColumnEncoding<()>` impl whose `evaluate` ignores `rows`
  and returns the column's data. Whether it can hand the store an `Arc<[f64]>`
  without a copy, or `ColumnData` needs a borrowed or shared variant, is the
  implementer's call, recorded in the findings.
- **Not now**: appends to columnar layers; builders taking `Columns`
  (`gup::scatter(columns)`); key columns (GUP-629); Arrow (GUP-630).

## Escalation Triggers

- Rows of `()` break an invariant somewhere (for example a check that `T` has a
  size, or code that reads rows for tooltips and panics); report rather than
  special-case `()` in more than one place.
- `IntoEncoding<(), ..>` for `ColumnEncoded` conflicts with an existing blanket
  impl (coherence), forcing a different public shape.
- The identical-image test (AC2) differs by any pixel.
- The design needs a change to `ColumnStore`'s public API beyond adding methods.

## Out of Scope

- Key and string columns (GUP-629); Arrow (GUP-630); LOD (GUP-631).
- Appending to columnar layers; builders over columns.

## Technical Tasks

- [x] `columns.rs`, exports, docs.
- [x] `add_columns`, `x_column`, `y_column`, `encode_column`, the
      `DynColumnEncoding<()>` impl.
- [x] AC2 tests in `crates/gup-core/tests/columns.rs`.
- [x] `scale_bench --columns`, AC3 runs and the f32 rule.
- [x] RFC-001 findings.

## Dependencies

### Prerequisite Stories

- GUP-627: Scale Benchmark: Ingest and Memory 📋 — the baseline numbers and the
  bench this story extends.

### Enables Stories

- GUP-629 (key columns), GUP-630 (Arrow), GUP-631 (LOD spike at 100M), GUP-632
  (100M demonstration).

## Testing Strategy

- **Unit tests**: `Columns` construction and errors.
- **Integration tests**: AC2's identical images, domains and picks.
- **Performance**: AC3's runs (manual, release, development machine).

## Success Metrics

- [ ] A 10M-point scatter loads from two `Vec<f64>` without a row type, at most
      half the first-resolve time of the rows path. (50-52% measured: see the
      Implementation Summary; the rest is the chunk CPU copy, GUP-664.)

## Risk Assessment

- **Medium**: `Selection` is the largest file and the most shared; a new impl of
  a core trait for `()` could ripple. _Mitigation_: the escalation triggers, and
  AC2's byte-identical check.
- **Medium**: the f64 intermediate may dominate, and removing it touches the
  chunk writer. _Mitigation_: the f32 rule only adds a path; the existing one
  stays the reference.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not needed, identity with the rows path
      is tested
- [x] `mask wasm-browser`: not needed, no browser path changes; state so
- [x] WASM size: `mask wasm-size` scatter gz bytes (new code is unused by the
      scatter; expect a small or no change)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Columnar layers as decided: `Columns`/`Column`
(`crates/gup-core/src/columns.rs`, new), `Plot::add_columns::<M>` (`plot.rs`),
`Selection::<(), M>::x_column` and `y_column` (`selection.rs`),
`EncodeFn::encode_column -> ColumnEncoded<Self>` and its
`IntoEncoding<(), V, ColumnMarker<f64>>` impl (`encoding.rs`). `Columns` and
`Column` are at the crate root (`Columns` also in the prelude), `ColumnEncoded`
at the root beside `Encoded`. The doctest on `add_columns` draws a scatter from
two `Vec<f64>` columns coloured through
`Sequential::viridis().encode_column(..)`.

- **No copy**: a hidden, defaulted `DynColumnEncoding::shared()` exposes a
  layer's shared column; `Selection::evaluate` passes it to the store as a
  borrowed `ColumnSlice` through the new crate-private
  `ColumnStore::append_slices` (`column.rs`). Public `ColumnStore` and
  `ColumnData` are unchanged. Plot-axis bindings are an `AxisSource` (closure or
  column).
- **Errors**: `Columns::with` refuses a column of another length; `Index` panics
  naming the column and listing the names; resolve fails naming the channel
  (`Circle::FILL`, `Circle::X`) when a column's length is not the layer's;
  `append` on a columnar layer, and the resolve that drains its `appender()`,
  fail saying a columnar layer cannot be appended to.
- **Chunk writer**: stats and validity bits are computed a word (32 rows) at a
  time with four min/max lanes (`scan`), for every layer: the write of two 10M
  columns went from 125 to 90 ms. f32 input is widened exactly in 256-value
  stack blocks into the one f64 writer.
- **Bench**: `scale_bench --columns f64|f32` builds the columns straight into
  `Arc<[_]>` and prints `f32.widen.ms`.
- **Tests**: `crates/gup-core/tests/columns.rs` (6): byte-identical images,
  domains, 20 picks and their `row_fields` for a 10,000-row scatter (f64, and
  f32 against its widened twin) and a line; channel-named length errors; append
  and appender errors; a columnar row announced under `GpuOnly` with released
  CPU columns. Unit tests: `columns.rs` (3), `column.rs` (the blocked scan
  against `Acc::push`; f32 against f64 bytes across appends and chunks). Two
  `compile_fail` stderr files list the new `IntoEncoding` impl.
  `cargo test -p gup-core` passes (all targets and doctests).

**AC3** (medians of 3, release, Intel HD 630, load 4-5 from other agents; MiB;
the full table and phase numbers are in RFC-001 "GUP-628 findings"):

| rows | rows path (GUP-627) | rows path      | f64 columns         | f32 columns         |
| ---- | ------------------- | -------------- | ------------------- | ------------------- |
| 10M  | 272.7 ms, 415       | 177.5 ms, 412  | 93.0 ms (52%), 261  | 94.7 ms (53%), 185  |
| 30M  | 810.5 ms, 1183      | 634.7 ms, 1180 | 347.0 ms (55%), 724 | 336.8 ms (53%), 495 |
| 100M | not run             | not run        | not run             | not run             |

100M was not run: `MemAvailable` was 6-18 GB, under the 20 GB the run needs.

- **f32 rule**: through an f64 intermediate the conversion was 28% of first
  resolve at 10M (51-55 of 191 ms), above 25%, so f32 is written straight into
  chunks (94.7 ms and 185 MiB instead of 191 ms and 337 MiB). Each value is
  widened exactly and the origin subtracted in f64, so it stores the f64 path's
  bytes; the f32-arithmetic variant was not needed.
- **Targets**: peak RSS is 1.6 MiB (10M) and 1.8 MiB (30M) over "rows minus user
  rows" (wgpu's upload staging, which the rows path's freed f64 vectors hide).
  Time is 50-52% of the rows path at 10M over two batches (target 50%). The rest
  of the columnar first resolve is the chunk CPU copy (page faults 23 ms,
  conversion about 28 ms, stats 15 ms) and the upload (10 ms): GUP-664.

**WASM** (`mask wasm-size`, same session): main 371,427 B gz, branch 373,205 B
gz (+1,778 B; per-row-type 1,525 to 1,821 B, under its 2,600 limit). The first
version was +2,090 B; one writer instead of an f64 and an f32 copy trimmed it.

**Not run**: `mask wasm-browser` on its own (it passed inside
`mask merge-check`), 100M rows (memory), rendered output by eye (identity with
the rows path is tested byte for byte).

## Retrospective

**Completed**: 2026-11-08

### Key Technical Learnings

#### The f64 intermediate was not the largest cost

- **Challenge**: the story expected the f64 vectors to dominate; without them
  the columnar first resolve was still 130 ms at 10M.
- **Solution**: temporary timings around `column_inputs`, `append_slices` and
  `upload`, `/proc/self/stat` page-fault counts and a pre-faulting experiment
  split the chunk write into faults (23 ms), conversion (28 ms) and per-row
  stats and validity (about 60 ms). Doing the stats a validity word at a time
  cut the last to about 25 ms.
- **Pattern**: measure the phase you are about to remove and the ones beside it;
  instrument with throwaway timings and revert them before committing.

#### `Vec<T>` into `Arc<[T]>` copies

- **Challenge**: f64 columns' build-phase peak RSS was 76 MiB above the columns.
- **Solution**: `Arc<[T]>::from(Vec<T>)` reallocates (the counts live in the
  allocation); collecting a `TrustedLen` iterator straight into `Arc<[T]>`
  allocates once. The bench does that, and the `Columns` docs say so.
- **Pattern**: in peak-RSS comparisons, make sure the setup's own transients do
  not set the peak (GUP-627's "peak RSS is monotonic").

### Architectural Decisions

#### A hidden `shared()` on the encoding, not a new `ColumnData` variant

- **Decision**: `DynColumnEncoding::shared() -> Option<&Column>` (defaulted,
  `#[doc(hidden)]`), a crate-private borrowed `ColumnSlice` and `append_slices`.
- **Reasoning**: no change to the public `ColumnData` or `ColumnStore` API (an
  escalation trigger), no copy, and one place (`read_value`) where a `()` row's
  missing index matters.
- **Trade-off**: `evaluate` on a column encoding still copies when other paths
  call it (picking after the CPU columns were released).
- **Future**: GUP-664 routes those readers through the shared column.

#### `appender()` stays infallible

- **Decision**: `append` on a columnar layer is an error, and so is the resolve
  that drains its appender; `appender()` still returns an `Appender`.
- **Reasoning**: `appender()` returns `Appender<T>` for every layer; a fallible
  signature would change every row layer's API for a case the type cannot see.
- **Trade-off**: the error arrives at the next resolve, not at the call.

#### `x_column` finds its channel at compile time

- **Decision**: a `const fn` finds the mark's first `X`- or `Y`-role channel in
  an inline `const` block, so a mark without one fails to compile instead of
  ignoring the call.

### Development Workflow Insights

- The worktree sandbox refuses the cargo wrapper around a commit; a plain commit
  with `CARGO_BUILD_BUILD_DIR` and `CARGO_TARGET_DIR` set inline works, and the
  hook then builds in this checkout's target directory.
- A `prettier` failure in the hook aborts the commit after the Rust checks pass;
  `git log -1` after every commit caught one that did not happen.
- Benchmark noise from other agents was 10-40% (load average up to 17): compare
  variants back to back and quote the load.

### Follow-up Stories

1. **GUP-664: Shared Columns as the CPU Copy of a Columnar Layer** — no chunk
   CPU copy for columnar layers (about 76 MiB and 60 ms less at 10M), and
   picking, re-encoding and other contexts above `Retain::AUTO_MAX_ROWS`, which
   GUP-632 needs.

Proposed `PERF_BUDGETS.md` rows (for the orchestrator): columnar first resolve
at 10M rows (93 ms, +20%) and its peak RSS (261 MiB, +5%) from
`scale_bench --columns f64`; and `wasm.scatter.gz_bytes` re-recorded at 373,205
B when this merges.
