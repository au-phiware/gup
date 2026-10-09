# GUP-414: RFC-001 Step S4a: Column Store Chunking and Append

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-10

## Context

[GUP-395](GUP-395_Gup_Core_Vertical_Slice_Headless.md) (RFC-001 S0a) built
`crates/gup-core/src/column.rs` as a deliberately single-chunk `ColumnStore`:
one buffer, one f64 origin per relative column, one `ColumnStats` per column,
documented in its own module doc as "S0a scope: exactly one chunk. Chunking,
append, retention policies, validity bits and dictionary encoding are RFC-001
S4." `ColumnStore::from_columns` errors out (`"chunking arrives in RFC-001 S4"`)
if the row count does not fit `u32`, or if the resulting chunk exceeds
`max_buffer_size`. [RFC-001](../rfcs/RFC-001_Core_Architecture.md) §3 fixes the
real design: `chunk_rows = min(2^20, max_buffer_size / Σ column_stride)`, one
GPU buffer per chunk, one instanced draw per chunk with a per-chunk
dynamic-offset uniform (`row_base`, per-channel relative `base`), and
`append(rows)` writing only the tail of the current last chunk.

The plumbing already anticipates this: `render.rs`'s `Chunk` bind group
(group 2) is already declared with `has_dynamic_offset: true` and a 256-byte
`DYNAMIC_ALIGN`, S0a's and S0b's evidence already proves `F32Relative`'s
precision gain for Unix-timestamp-scale values in one chunk (0.74 px miss as
absolute f32, 1.0e-4 px as relative), and `Context`'s upload counters
(`Upload::Column`, checked by `context::tests::every_gpu_write_is_counted`)
already count every `queue.write_buffer` call gup-core makes — S0b's finding
explicitly calls out that "S4's tail appends are counted as `Upload::Column`" as
the proposed adjustment this story fulfils. GUP-406's findings confirm S4 is
otherwise unaffected by the build-time shader composition move: a new column
format only changes the glue emitter's generated entry-point shape, never the
linker or the build step.

This story (S4a) covers the structural half of S4: multiple chunks, per-chunk
origins and stats, the multi-draw rendering path, and counted tail append, with
precision proved at a chunk boundary. The richer column semantics — validity
bits for nulls, dictionary-encoded categorical keys and the `Retain` policy —
are [GUP-415](GUP-415_RFC_001_S4b_Nulls_And_Dictionaries.md) (S4b), which
depends on this story's multi-chunk structure.

## User Story

> "As a gup-core implementer, I want the column store to split large data sets
> into GPU-buffer-sized chunks and append new rows by writing only their bytes,
> so that a selection can hold more rows than one buffer allows and grow without
> re-uploading what is already on the GPU."

## Acceptance Criteria

### AC1: Multi-chunk store structure

- [ ] `ColumnStore` splits evaluated accessor columns into chunks of
      `chunk_rows = min(2^20, max_buffer_size / Σ column_stride)` rows (the
      limit read from `Context::caps()`, overridable in tests so a small number
      of rows can be forced to span several chunks).
- [ ] Each chunk has its own GPU buffer, its own per-relative-column f64 origin
      (the chunk's first finite value in that column), and its own `ColumnStats`
      (min/max/non-finite count) in f64.
- [ ] `ColumnStore::rows()` returns the total row count across every chunk; a
      new accessor exposes per-chunk metadata (origin, stats, row count) for
      callers that need it (domain fitting, future culling).
- [ ] A store built from data that fits in one chunk behaves identically to
      today's single-chunk store (same bytes, same stats, same origin) — proved
      by a regression test.

### AC2: Multi-draw rendering path

- [ ] `Selection::prepare` writes one `Chunk` uniform entry per chunk (each
      256-byte aligned in one dynamic-offset buffer) holding that chunk's
      `row_base` and every relative channel's `chunk_base`.
- [ ] `LayerGpu` holds one draw entry per chunk (its own column buffer and
      ranges, instance count, and dynamic offset into the chunk uniform buffer)
      instead of assuming a single chunk.
- [ ] `Prepared::draw`'s `Draw::Marks` arm issues one `pass.draw` per chunk,
      with the correct bind-group dynamic offset and vertex-buffer ranges for
      that chunk.

### AC3: Multi-chunk rendering proof

- [ ] A test forces a chunk size small enough that the reference scatter's row
      count (`tests/common/scatter.rs`) spans at least 3 chunks.
- [ ] The PNG rendered through the forced multi-chunk store is checked by eye
      (read as a PNG or compared as a golden) against the same data rendered
      through the default, single-chunk-sized store: the two are pixel identical
      (or within the existing ΔE 0 tolerance used by `tests/targets.rs`). This
      is a structural/golden comparison, not a "produces non-white pixels"
      check.

### AC4: Precision at a chunk boundary

- [ ] A test builds a column of large-magnitude f64 values (Unix-second
      timestamps spanning multiple years, as a stand-in for a real `Time` scale,
      which is RFC-001 S5), forces chunking so a chunk boundary falls inside the
      data, and zooms the x domain to a one-second window straddling that
      boundary.
- [ ] The GPU-rendered pixel position of the points nearest the boundary matches
      `CpuMirror::eval` (`Linear`'s existing f64 CPU mirror) within 0.25 px,
      using the same GPU-vs-CPU compute-based comparison S0a's conformance
      evidence used.
- [ ] A negative control (the same values stored as absolute `F32`, not
      `F32Relative`) misses by more than 0.25 px, so the test proves the
      per-chunk origin is what keeps the boundary inside budget, not an accident
      of the specific numbers chosen.

### AC5: Counted tail append

- [ ] A method to append `K` new rows to a `Selection`/`ColumnStore` runs the
      stored accessors on the new rows only, and writes just their bytes into
      the tail of the current last chunk's columns via `queue.write_buffer` at
      each column's sub-range offset, going through the existing
      `Context::write_buffer` path so the write is counted as `Upload::Column`
      (per `context::tests::every_gpu_write_is_counted`'s existing contract).
- [ ] When the last chunk is full, append opens a new chunk with a fresh
      per-column origin and stats; it never re-uploads a previously-full chunk's
      bytes.
- [ ] f64 stats (and, through them, any auto-fitted domain) update incrementally
      from the appended rows; growing a domain costs a uniform write and guide
      re-resolve only.
- [ ] A test appends rows in several batches and asserts, via the upload
      counters, that the total column bytes written equal exactly the bytes of
      the appended rows (header/padding aside) — no batch causes a full chunk
      re-upload.
- [ ] `tests/zoom_uploads.rs`'s existing claim (0 column bytes, exactly 3
      uniform writes per frame, across 300 zoomed frames) still holds when the
      reference scatter is built with a forced small chunk size spanning
      multiple chunks.

### AC6: Browser

- [ ] `mask wasm-browser` still passes, including a run of the scatter harness
      with a forced small chunk size so the multi-chunk draw path (AC2) is
      exercised on WebGPU, not just natively.

### AC7: Old path frozen

- [ ] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [ ] Add a chunk-row-count parameter to `ColumnStore` construction, computed
      from `Context::caps().limits` per the §3 formula, with a test-only way to
      force a smaller value.
- [ ] Restructure `ColumnStore::from_columns` to split evaluated columns into
      `Chunk`s (buffer, per-column origin, per-column `ColumnStats`, row count),
      keeping the existing byte layout and alignment per chunk.
- [ ] Add per-chunk accessors (`ColumnStore::chunks()` or equivalent) and keep
      `rows()` as the total across chunks.
- [ ] Extend `GlueProgram`'s `Chunk` uniform handling (`render.rs`) to a
      dynamic-offset buffer with one 256-aligned entry per chunk; update
      `LayerUniforms::build`/`write_uniforms` to write every chunk's entry.
- [ ] Change `LayerGpu` from a single draw target to a `Vec` of per-chunk draw
      entries (buffer, column ranges, instance count, dynamic offset); update
      `Prepared::draw`'s `Draw::Marks` arm to iterate them.
- [ ] Update `Selection::prepare` to loop over the evaluated store's chunks when
      writing per-chunk uniform fields instead of assuming exactly one.
- [ ] Aggregate f64 stats across chunks for `fit_domain` (overall extent =
      min/max over every chunk's stats).
- [ ] Implement append: run stored accessors on new rows, extend the retained
      `rows: Vec<T>`, write the tail of the last chunk through the counted
      `Context::write_buffer` path, opening a new chunk when the last one is
      full.
- [ ] Write the multi-chunk-vs-single-chunk golden-equivalence test (AC3).
- [ ] Write the chunk-boundary precision test with its negative control (AC4).
- [ ] Write the append byte-count test and extend `tests/zoom_uploads.rs` for a
      forced multi-chunk reference scatter (AC5).
- [ ] Extend the `mask wasm-browser` harness (or add a variant) to force a small
      chunk size (AC6).
- [ ] Re-run `mask old-path-loc` and confirm it is unchanged (AC7).

## Dependencies

### Prerequisite Stories

- GUP-395 ✅ — RFC-001 S0a: the single-chunk `ColumnStore`, `Context` and typed
  glue emitter this story generalises to many chunks.
- GUP-401 ✅ — RFC-001 S3: `RenderTarget`/`Renderer`/`Prepared` and the
  counted-upload contract (`every_gpu_write_is_counted`) append's tail writes
  must satisfy.
- GUP-410 ✅ — GPU error scopes already wrap the column-upload step of
  `Plot::resolve`; this story's multi-chunk upload and append paths must keep
  running inside that scope (soft — no API change expected, just continuity).

### Enables Stories

- GUP-415 (S4b) — validity bits, dictionary encoding and `Retain` build on this
  story's multi-chunk `Chunk`/`ColumnStore` structure.
- RFC-001 S12 (`append`/`appender`/`Window`, not yet written as a story) — the
  public `Selection`-level append API and eviction windows are built on this
  story's column-store-level append primitive.
- RFC-001 S5 (full scale family) — the `Time` scale's real precision proof
  (years of calendar time, not a stand-in Unix-second column) exercises this
  story's chunk-boundary machinery directly.

## Testing Strategy

- **Unit tests**: chunk splitting (row counts, origins, stats) for a forced
  small chunk size, including the single-chunk regression case; aggregated stats
  across chunks; append's tail-write byte accounting.
- **Integration tests**: the multi-chunk-vs-single-chunk golden-equivalence
  render (AC3); the chunk-boundary precision test with its negative control
  (AC4); the extended `zoom_uploads.rs` 0-column-bytes claim over a multi-chunk
  scene.
- **Visual validation**: AC3's PNG is read by eye (or compared pixel-exact to
  the existing single-chunk golden) by the implementer before marking the story
  done.
- **Browser**: `mask wasm-browser` run against a forced-small-chunk scene (AC6).
- **Performance**: no new perf budget is introduced; if draw-call count per
  layer becomes measurable with many chunks, note it for RFC-001 S9/T7 (chunk
  culling), but no new benchmark is required by this story.

## Success Metrics

- [ ] A selection whose row count exceeds one chunk renders correctly with no
      API change visible to `Selection::attr` callers.
- [ ] Append writes exactly the new rows' bytes, proved by the upload counters,
      with zero full-chunk re-uploads across a multi-batch append test.
- [ ] The chunk-boundary precision test passes within 0.25 px, and its
      absolute-`F32` negative control fails it.
- [ ] `mask wasm-browser` and `mask old-path-loc` are unaffected.

## Risk Assessment

- **Medium**: restructuring `LayerGpu` from one draw target to a `Vec` of
  per-chunk entries touches the hot path (`Prepared::draw`) that S0b measured at
  100K points / 60 fps with 0 column bytes during zoom. _Mitigation_: keep the
  per-chunk entries cheap to iterate (no allocation in `draw`, as today); re-run
  `zoom_bench` if the change is non-trivial, and treat any regression past the
  existing 2.4× p95 headroom as a blocker.
- **Low**: the dynamic-offset `Chunk` buffer's 256-byte alignment means a store
  with many chunks uses more uniform-buffer memory than one entry per chunk
  strictly needs; this is within WebGPU's dynamic-offset alignment rules and not
  expected to matter until chunk counts are very large (future LOD/T7
  territory).
- **Low**: forcing a small chunk size in tests needs a seam that does not leak
  into the public (not-yet-exported) API in a way future stories regret.
  _Mitigation_: keep the override `pub(crate)`/test-only, matching how other
  gup-core internals expose test hooks today (e.g. `PipelineCache`'s
  `#[cfg(test)]` accessors).

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read) for AC3
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
