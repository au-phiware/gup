# GUP-414: RFC-001 Step S4a: Column Store Chunking and Append

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-10)
**Created**: 2026-10-10

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

- [x] `ColumnStore` splits evaluated accessor columns into chunks of
      `chunk_rows = min(2^20, max_buffer_size / Σ column_stride)` rows (the
      limit read from `Context::caps()`, overridable in tests so a small number
      of rows can be forced to span several chunks).
- [x] Each chunk has its own GPU buffer, its own per-relative-column f64 origin
      (the chunk's first finite value in that column), and its own `ColumnStats`
      (min/max/non-finite count) in f64.
- [x] `ColumnStore::rows()` returns the total row count across every chunk; a
      new accessor exposes per-chunk metadata (origin, stats, row count) for
      callers that need it (domain fitting, future culling).
- [x] A store built from data that fits in one chunk behaves identically to
      today's single-chunk store (same bytes, same stats, same origin) — proved
      by a regression test.

### AC2: Multi-draw rendering path

- [x] `Selection::prepare` writes one `Chunk` uniform entry per chunk (each
      256-byte aligned in one dynamic-offset buffer) holding that chunk's
      `row_base` and every relative channel's `chunk_base`.
- [x] `LayerGpu` holds one draw entry per chunk (its own column buffer and
      ranges, instance count, and dynamic offset into the chunk uniform buffer)
      instead of assuming a single chunk.
- [x] `Prepared::draw`'s `Draw::Marks` arm issues one `pass.draw` per chunk,
      with the correct bind-group dynamic offset and vertex-buffer ranges for
      that chunk.

### AC3: Multi-chunk rendering proof

- [x] A test forces a chunk size small enough that the reference scatter's row
      count (`tests/common/scatter.rs`) spans at least 3 chunks.
- [x] The PNG rendered through the forced multi-chunk store is checked by eye
      (read as a PNG or compared as a golden) against the same data rendered
      through the default, single-chunk-sized store: the two are pixel identical
      (or within the existing ΔE 0 tolerance used by `tests/targets.rs`). This
      is a structural/golden comparison, not a "produces non-white pixels"
      check.

### AC4: Precision at a chunk boundary

- [x] A test builds a column of large-magnitude f64 values (Unix-second
      timestamps spanning multiple years, as a stand-in for a real `Time` scale,
      which is RFC-001 S5), forces chunking so a chunk boundary falls inside the
      data, and zooms the x domain to a one-second window straddling that
      boundary.
- [x] The GPU-rendered pixel position of the points nearest the boundary matches
      `CpuMirror::eval` (`Linear`'s existing f64 CPU mirror) within 0.25 px,
      using the same GPU-vs-CPU compute-based comparison S0a's conformance
      evidence used.
- [x] A negative control (the same values stored as absolute `F32`, not
      `F32Relative`) misses by more than 0.25 px, so the test proves the
      per-chunk origin is what keeps the boundary inside budget, not an accident
      of the specific numbers chosen.

### AC5: Counted tail append

- [x] A method to append `K` new rows to a `Selection`/`ColumnStore` runs the
      stored accessors on the new rows only, and writes just their bytes into
      the tail of the current last chunk's columns via `queue.write_buffer` at
      each column's sub-range offset, going through the existing
      `Context::write_buffer` path so the write is counted as `Upload::Column`
      (per `context::tests::every_gpu_write_is_counted`'s existing contract).
- [x] When the last chunk is full, append opens a new chunk with a fresh
      per-column origin and stats; it never re-uploads a previously-full chunk's
      bytes.
- [x] f64 stats (and, through them, any auto-fitted domain) update incrementally
      from the appended rows; growing a domain costs a uniform write and guide
      re-resolve only.
- [x] A test appends rows in several batches and asserts, via the upload
      counters, that the total column bytes written equal exactly the bytes of
      the appended rows (header/padding aside) — no batch causes a full chunk
      re-upload.
- [x] `tests/zoom_uploads.rs`'s existing claim (0 column bytes, exactly 3
      uniform writes per frame, across 300 zoomed frames) still holds when the
      reference scatter is built with a forced small chunk size spanning
      multiple chunks.

### AC6: Browser

- [x] `mask wasm-browser` still passes, including a run of the scatter harness
      with a forced small chunk size so the multi-chunk draw path (AC2) is
      exercised on WebGPU, not just natively.

### AC7: Old path frozen

- [x] No file outside `crates/gup-core` (and planning docs) changes;
      `mask old-path-loc` reports the same count as before this story.

## Technical Tasks

- [x] Add a chunk-row-count parameter to `ColumnStore` construction, computed
      from `Context::caps().limits` per the §3 formula, with a test-only way to
      force a smaller value.
- [x] Restructure `ColumnStore::from_columns` to split evaluated columns into
      `Chunk`s (buffer, per-column origin, per-column `ColumnStats`, row count),
      keeping the existing byte layout and alignment per chunk.
- [x] Add per-chunk accessors (`ColumnStore::chunks()` or equivalent) and keep
      `rows()` as the total across chunks.
- [x] Extend `GlueProgram`'s `Chunk` uniform handling (`render.rs`) to a
      dynamic-offset buffer with one 256-aligned entry per chunk; update
      `LayerUniforms::build`/`write_uniforms` to write every chunk's entry.
- [x] Change `LayerGpu` from a single draw target to a `Vec` of per-chunk draw
      entries (buffer, column ranges, instance count, dynamic offset); update
      `Prepared::draw`'s `Draw::Marks` arm to iterate them.
- [x] Update `Selection::prepare` to loop over the evaluated store's chunks when
      writing per-chunk uniform fields instead of assuming exactly one.
- [x] Aggregate f64 stats across chunks for `fit_domain` (overall extent =
      min/max over every chunk's stats).
- [x] Implement append: run stored accessors on new rows, extend the retained
      `rows: Vec<T>`, write the tail of the last chunk through the counted
      `Context::write_buffer` path, opening a new chunk when the last one is
      full.
- [x] Write the multi-chunk-vs-single-chunk golden-equivalence test (AC3).
- [x] Write the chunk-boundary precision test with its negative control (AC4).
- [x] Write the append byte-count test and extend `tests/zoom_uploads.rs` for a
      forced multi-chunk reference scatter (AC5).
- [x] Extend the `mask wasm-browser` harness (or add a variant) to force a small
      chunk size (AC6).
- [x] Re-run `mask old-path-loc` and confirm it is unchanged (AC7).

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

- [x] A selection whose row count exceeds one chunk renders correctly with no
      API change visible to `Selection::attr` callers.
- [x] Append writes exactly the new rows' bytes, proved by the upload counters,
      with zero full-chunk re-uploads across a multi-batch append test.
- [x] The chunk-boundary precision test passes within 0.25 px, and its
      absolute-`F32` negative control fails it.
- [x] `mask wasm-browser` and `mask old-path-loc` are unaffected.

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read) for AC3
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

The full record, with numbers, is RFC-001's "S4a findings (2026-10-10, GUP-414)"
section.

### Delivered

- **Chunked store** (`crates/gup-core/src/column.rs`). `ColumnStore` holds
  `Chunk`s of `chunk_rows = min(2^20, max_buffer_size / Σ stride)` rows, rounded
  down to 64-row blocks (`ColumnStore::chunk_rows_for`). Each chunk has its own
  buffer, f64 origin per relative column and stats. `ColumnStore::chunks()`
  exposes them (`row_base`, `rows`, `capacity`, `columns`, `bytes`). `rows()` is
  the total, a `u64`, and `stats(k)` merges every chunk.
- **Append** (`ColumnStore::append`, `Selection::append` (`pub(crate)`)).
  Appending evaluates only the new rows and fills the last chunk, doubling its
  capacity and moving rows GPU-side with `copy_buffer_to_buffer`, then opens new
  chunks. `upload` writes only the missing rows, column by column, as
  `Upload::Column`.
- **Draw path** (`render.rs`, `selection.rs`, `scene.rs`). `LayerGpu` is an
  `Arc`'d `Encodings` half plus a per-chunk half: one dynamic-offset `Chunk`
  uniform buffer (entries 256 apart, written in one write) and a `ChunkDraw` per
  chunk. `Prepared::draw` issues one instanced draw per chunk.
  `MarkBatch::chunks()` is new, and `MarkBatch::instances()` is now a `u64`.
- **Test seam.** `Selection::max_chunk_rows` (`#[doc(hidden)] pub`), used by the
  tests, the wasm harness and `zoom_bench --chunk-rows`.
- **Bug fixed.** A selection with only constant channels drew 0 instances.

### Deviation

- **AC3: not pixel-identical on every adapter.** The multi-chunk render is
  pixel-identical to the single-chunk render on Intel/Mesa and on SwiftShader,
  but on lavapipe 7 (3 chunks) and 9 (4 chunks) edge pixels differ by 1/255.
  Per-chunk origins round x differently, by about 1e-5 px. The test allows 64
  pixels at 1/255; a wrong dynamic offset changes 10,503 pixels.
- **AC7: one file outside gup-core changed while the story ran.** The
  orchestrator asked for a separate commit quarantining a flaky old-path test
  (`tests/gpu_statistics_integration_tests.rs`, 07606cc, plus a GUP-404 note).
  It is not part of S4a's code. `mask old-path-loc` reads 28910 before and
  after.

### Tests

The story adds 12 gup-core lib tests (71 in all on the merged tree) and 2
integration tests (`zoom_uploads` multi-chunk, `scatter_png` multi-chunk). It
passes on Intel/Mesa and on lavapipe (`VK_ICD_FILENAMES=…lvp_icd…`,
`WGPU_BACKEND=vulkan`).

- `column::tests`: the S0a-layout regression (1 to 4096 rows), small-chunk
  splitting, `chunk_rows_for`, append fill/grow/open and the NaN-first origin,
  upload once per context, and GPU tail bytes after append.
- `selection::tests`: the chunk uniform entries (row base and base per chunk),
  and an append that reuses the encodings and writes new rows only.
- `plot::tests::appended_rows_upload_only_their_bytes`: 469 appended rows, 3,752
  B, with the image equal to a plot built from all rows at once.
- `scale::conformance::chunk_boundary` (4 tests): 2.67e-5 px at the boundary;
  993.8 px for absolute f32; 993.8 px for one relative chunk spanning years;
  39.9 px for a full chunk of seconds (the S5 limit).
- `tests/scatter_png.rs::multi_chunk_scatter_matches_one_chunk`, and
  `tests/zoom_uploads.rs` over 7 chunks: 0 column bytes, 3 uniform writes per
  frame.
- Browser: `mask wasm-browser` compares 1-chunk and 7-chunk renders (0 pixels
  differ on SwiftShader; the seeded offset bug fails the page).

### Evidence

- **Visual.** I read `scatter_4_chunks.png` (the golden scatter in 4 chunks:
  title, log y and linear x ticks, viridis points inside the plot) and the
  browser PNG (the chunked render: title clipped at 320 px as before, tinted
  background, legend bar).
- **Window.** `mask gup-core-window`: the window frame is ΔE 0 against the
  `ImageTarget` and the golden. `zoom_bench` showed no regression against the
  pre-S4a tree, with one chunk or seven (RFC findings table).
- **Lint.** `mask all-check` passes.
- **Not run.** The root crate's `cargo test` and `mask smoke-examples` were left
  to CI, as the orchestrator asked. Root examples are type-checked by clippy
  `--all-targets` in `mask all-check`.

### Key files

- `crates/gup-core/src/column.rs`
- `crates/gup-core/src/render.rs`
- `crates/gup-core/src/selection.rs`
- `crates/gup-core/src/scene.rs`
- `crates/gup-core/src/plot.rs`
- `crates/gup-core/src/scale/conformance.rs`
- `crates/gup-core/tests/{scatter_png,zoom_uploads}.rs`
- `crates/gup-core/tests/common/scatter.rs`
- `crates/gup-core/examples/zoom_bench.rs`
- `crates/gup-core/wasm-size/scatter/{src/lib.rs,index.html}`

## Retrospective

**Completed**: 2026-10-10

### Key Technical Learnings

#### Exact-size chunks without breaking "never re-upload"

- **Challenge**: RFC-001 §3 sketches fixed-capacity chunks of `chunk_rows` rows.
  For a 120-row plot that is a 12 MiB buffer, and it changes the S0a byte layout
  that AC1 must preserve. Exact-size chunks, on the other hand, leave no room to
  append without re-uploading.
- **Solution**: a built chunk's capacity is its rows rounded up to a 64-row
  block. That is exactly the padding S0a's 256-byte column alignment already
  had, so the bytes are identical. When an append runs out of room, capacity
  doubles and the uploaded rows move on the GPU (`copy_buffer_to_buffer`), so no
  CPU bytes are written.
- **Pattern**: let padding the format already pays for double as growth room,
  and grow on the GPU when it runs out.

#### Count rows, not buffers

- **Challenge**: S0b's initial upload wrote the whole chunk, padding included,
  in one write, so "bytes written = appended rows' bytes" could only hold with
  "padding aside".
- **Solution**: every upload, first or tail, writes each column's missing rows.
  Column bytes written are always rows × stride, which makes the append
  assertions exact. It costs one write per column instead of one per chunk.
- **Pattern**: make counted quantities match the domain unit (rows), so tests
  can assert equality instead of bounds.

#### A golden that is byte-exact on one rasteriser is not an invariant

- **Challenge**: the first AC3 test compared multi-chunk renders with the golden
  PNG byte for byte. It passed on Intel and failed on lavapipe in CI. Even
  multi-chunk against single-chunk on the same device was not byte-exact on
  lavapipe: 7 and 9 pixels differed by 1/255.
- **Solution**: compare multi-chunk with single-chunk on the same device in the
  same run, with a bound justified by the measured cause. Per-chunk origins
  round positions differently by about 1e-5 px (AC4's measurement), so the bound
  is at most 64 pixels at 1/255. Leave golden checks to the harness's ΔE
  tolerance. Running the gup-core suite locally on lavapipe
  (`VK_ICD_FILENAMES=…/lvp_icd.x86_64.json WGPU_BACKEND=vulkan`) reproduces CI
  in seconds.
- **Pattern**: before asserting byte equality between two GPU renders, check
  whether they do the same arithmetic. If they do not, bound the difference and
  prove the bound tight with a seeded bug (10,503 pixels, against 9).

#### Per-chunk origins are necessary, not sufficient

- **Challenge**: proving AC4 needs data that is dense at a boundary yet spans
  years.
- **Solution**: coarse chunks across the years, then two dense chunks at the
  boundary. Two controls show what does the work. Absolute f32 misses by 994 px,
  and so does one relative chunk whose origin is three years back. A third
  measurement shows the limit: a full default chunk of one-second samples misses
  a one-second zoom by 40 px.
- **Pattern**: alongside the negative control a story asks for, add a control
  that measures the limit of the mechanism. That limit is input for the next
  story (S5).

### Architectural Decisions

#### Split `LayerGpu` into a shared `Encodings` half and a per-chunk half

- **Decision**: the `Encodings` uniform and LUT bind group are an `Arc` shared
  across rebuilds. After an append, only the `Chunk` uniform buffer and the
  draws are rebuilt.
- **Reasoning**: without the split, every append re-uploads palette LUTs and
  recreates their bind groups.
- **Trade-off**: one more type and an `Arc`. The zoom path is unchanged: 3
  uniform writes, and 88 B per frame for one chunk.
- **Future**: S9 culling can filter chunk draws at prepare time without touching
  encodings.

#### Growth submits during resolve

- **Decision**: a grown chunk's GPU copy is submitted from `ColumnStore::upload`
  (within `Plot::resolve`), counted by `Context::submit`.
- **Reasoning**: `write_buffer` cannot copy between buffers, and re-uploading
  from the CPU would break the byte-count claim.
- **Trade-off**: a host that resolves inside its own frame graph sees an extra
  submission on growth (log2 times per chunk). `Renderer::prepare` and
  `Prepared::draw` still never submit.
- **Future**: S12/S13 may want to defer the copy to the host's encoder.

#### A `#[doc(hidden)] pub` test seam

- **Decision**: `Selection::max_chunk_rows` is public but hidden from the docs.
- **Reasoning**: the integration tests, the wasm harness and `zoom_bench` are
  separate crates, so a `#[cfg(test)]` seam cannot reach them.
- **Trade-off**: it is a visible method that is not a supported knob.
- **Future**: if culling granularity ever becomes a user setting, this method is
  where it would go.

### Development Workflow Insights

- **Seeded bugs.** Setting every chunk's dynamic offset to 0 tested both the
  native and the browser comparisons (10,503 pixels and 51,351 bytes differ). It
  showed that the tests would see a broken offset, not merely that the code
  looks right.
- **Reading the page.** `MarkBatch::chunks()` and the page's `GUP CHUNKS` log
  line make "the browser really drew 7 chunks" checkable from the console.
- **Lost context.** The session was cut off mid-story (disk exhaustion). The
  checkpoint commit (`ad48b3b`) kept the main work safe, and the remaining items
  were resumed from a list.
- **Benchmarks need a baseline from the same session.** `zoom_bench`'s GPU pass
  measured about 3.7 ms where S0b measured 2.7–3.0 ms. Building the pre-S4a tree
  in a temporary worktree and alternating runs showed that both trees are equal,
  so the drift is not S4a's.

### Follow-up Stories

None written. The work S4a uncovered belongs to stories that already exist or
are planned:

- **S4b (GUP-415):** the bit-stride, dictionary and `Retain` notes were added to
  its Context.
- **S5 (`Time` precision limit), S9 (culling and picking) and S12 (append
  handles, eviction):** recorded in RFC-001's S4a findings for whoever writes
  those stories.
- **The GPU pass drift since S0b** (both trees about 3.7 ms against S0b's
  2.7–3.0 ms): not investigated. It predates this story; a candidate is
  GUP-407's text changes, which a before-and-after `zoom_bench` around GUP-407
  would settle.
