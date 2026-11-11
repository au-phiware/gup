---
implementer: opus
implementer-reason:
  a shader-interface change in the mark glue (an indexed-rows variant) that
  pick, brush and validity all read, plus a new per-chunk GPU buffer set
touches:
  [
    crates/gup-core/src/shader/glue.rs,
    crates/gup-core/src/render.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/src/order.rs,
    crates/gup-core/src/lod.rs,
  ]
---

# GUP-715: Representative Buffers and the Indexed-Rows Glue

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-12) **Created**: 2026-11-11

## Context

GUP-714 gives each chunk a `Tiers` (a progressive order of chunk-local rows and
per-tier prefix counts). To draw a tier, the renderer needs the representatives'
columns on the GPU, and the shader must still know each instance's data row, for
the brushed bit, the validity bit and the pick id. Today the glue computes all
three from `instance_index` (`chunk.row_base + instance_index`, and
`instance_index / 32u` for validity: `crates/gup-core/src/shader/glue.rs` lines
~726-800), which is the data row only when every row of a chunk is drawn in
order. RFC-001 "GUP-631 findings" ("Interaction under LOD", GPU pick) calls this
the "indexed rows" mark-glue variant: a `u32` chunk-local row per instance.

This story builds the buffers and the glue variant and proves them with a
forced-tier test seam; choosing a tier per frame and wiring a public setter is
GUP-716.

## Read First

- RFC-001 "GUP-631 findings": "Interaction under LOD" (GPU pick, brush), "Extra
  memory" (12 B a representative).
- `crates/gup-core/src/shader/glue.rs`: `GlueSpec` (line 47), the vertex entry
  generation (lines ~700-800: `brush_row`, `valid_row`, `vertex(...)` calls) and
  the pick variant (`Glue::pick`, line ~917).
- `crates/gup-core/src/render.rs`: `LayerGpu` (line 205) and `LayerGpu::record`
  (line 1124); `crates/gup-core/src/order.rs`: `DrawEntry` (line 295).
- `crates/gup-core/src/column.rs`: `Chunk`, `ChunkGpu` (line 692), `layout`,
  `ColumnStore::value` (GUP-664), per-column origins and `F32Relative`.
- `crates/gup-core/src/selection.rs`: `row_id` (line ~2910) and how the pick id
  maps `row_base + instance_index` (line ~1556).
- `crates/gup-core/tests/pick.rs` and `tests/hover.rs` (the GPU pick tests'
  shape).

## User Story

> "As the LOD feature, I want the GPU to draw only a chunk's representatives yet
> still colour, brush and pick them as the data rows they stand for."

## Acceptance Criteria

### AC1: Representative buffers

- [x] For a chunk with `Tiers` and a layer's column formats, a `LodGpu` holds
      the representatives `order[..counts[stored]]` gathered into the same
      column layout, formats and per-chunk origins as the chunk's own buffer (so
      the existing pipelines read them), plus a `u32` buffer of chunk-local
      rows. Gathered from the chunk's CPU bytes or, for a shared-column chunk,
      from the shared columns (`ColumnStore::value`, GUP-664). A test asserts
      the GPU bytes of representative `i` equal the chunk's bytes of row
      `order[i]` (read back), for `F32Relative` x and y and a constant-encoded
      channel. _Met with one buffer_: the gathered columns and the rows are
      ranges of one packed buffer (`LodGpu::range`), not two buffers; a constant
      `F32` column and `U32` codes are both tested, from owned and shared
      stores.
- [x] Memory: `LodGpu` of a chunk is at most 12 B a representative for an x/y
      layer (8 B columns + 4 B row), asserted by a size check on a 100,000-row
      chunk.

### AC2: The indexed-rows glue

- [x] `GlueSpec` gains `indexed: bool` (false: the glue source is byte-for-byte
      what it is today, asserted by the existing glue-source tests passing
      unchanged). When true, one extra `u32` instance column `lod_row` is bound
      after the channels' columns, the vertex entry reads `let row = lod_row;`
      and uses it where it used `instance_index` for `brush_row`, the validity
      group and bit, and the `row` argument it passes to the mark's
      `vertex`/`vertex_variant`; encodings still read the gathered columns at
      the instance. The pick variant's id is `chunk.row_base + row`. (`lod_row`
      is a member of the `Columns` input, so the line is
      `let row = col.lod_row;`.)
- [x] For a polyline geometry `indexed` is rejected (a `debug_assert` and an
      `Err` from the spec builder): LOD is quads only (Decisions).

### AC3: Drawing a forced tier, picking and brushing the data rows

- [x] `DrawEntry` gains `lod: Option<u32>` (the representative count drawn from
      `LodGpu`; `None` is today's entry and `draw_entries` produces `None`).
      `LayerGpu::record` binds `LodGpu` buffers and the indexed pipeline for
      entries with `Some`, and the chunk's own otherwise.
- [x] A `pub(crate)` seam `Selection::force_lod_tier(Option<u32>)` (test-only
      use, not public API) makes prepare draw each chunk's tier-`t` prefix. A
      test in `crates/gup-core/tests/` (no GPU-less skip: use the existing GPU
      test pattern) on a 5,000-row scatter forced to tier 4: 1. every drawn
      circle's GPU pick id (`poll_picks`, `Layer::row_id`) is a row in
      `order[..counts[4]]` of its chunk, and picking at a representative's pixel
      returns that data row (compare against the row the CPU knows from
      `Tiers`); 2. with a brush set to a row set that includes representative
      `r` and not representative `q`, `r` draws at full alpha and `q` dimmed
      (read the PNG pixels at their centres, difference from the unbrushed
      render at least ΔE00 10), proving the brushed bit is read by data row; 3.
      a layer with a null in the y column: the row is never drawn (never a
      representative per GUP-714) and a representative's validity bit is the
      data row's (a null elsewhere does not blank it). _Met in
      `crates/gup-core/src/lod/draw_tests.rs`, not `tests/`_: the seam is
      `pub(crate)` (Decisions), which an integration test cannot reach; the
      tests use the GPU pick tests' pattern (`gpu_pick/tests.rs`).

### AC4: Nothing else moves

- [x] With the seam unset every existing test and golden passes unchanged;
      `mask wasm-size` reports the scatter gz bytes (the indexed variant is not
      linked unless a layer uses it: record the delta, expected 0). _Delta
      +2,460 B gz, not 0_ (379,228 → 381,688): the tier build and the indexed
      program are not linked, but the draw path's branches are; see the
      Definition-of-Done evidence.

## Decisions

- **Gather on the CPU, upload once per chunk and tier set**, lazily, not in a
  compute pass: the representatives are at most half a chunk, the gather is a
  strided copy (tens of ms per 10M rows), and it needs no new GPU path. `LodGpu`
  is stored on `Chunk` next to `ChunkGpu`.
- **One extra instance column, not an index buffer**: instances come from
  instance-step vertex buffers (see GUP-492 Context); an index buffer cannot
  order instances.
- **Indexed variant is a pipeline variant** keyed in the layer's glue cache like
  `brushed`/`validity` (a new `indexed` in the pipeline key), so layers without
  LOD link no extra WGSL and the scatter WASM does not grow unless used.
- **Quads only**: the finding limits LOD to `Circle` and constant-extent `Rect`
  (RFC-001, "Proposed API"). A polyline reads neighbouring rows; no LOD for it.
- **The seam is `pub(crate)`**; the public API is GUP-716's.
- **Released chunks (2026-11-12, orchestrator; handed to GUP-716)**: an owned
  store's full chunks lose their CPU copy after the first prepare under
  `Retain::Auto` above 10M rows (`GpuOnly`, `Rows`: any size), and then neither
  `Tiers::build` nor the gather has a source. GUP-716 builds the tiers and
  `LodGpu` of every full chunk of a LOD-eligible layer in the prepare whose
  release would drop them (B; not A, limiting LOD to readable stores, nor C,
  delaying release); see its Decisions. Here, `LodGpu` gathers from the chunk's
  CPU bytes or shared columns and returns an error naming the chunk when it has
  neither. The `max_vertex_buffers` trigger does not fire for LOD-eligible
  marks: `Circle` needs 7 columns + `lod_row` = 8 slots, a constant-extent
  `Rect` at most 7.

## Escalation Triggers

- The glue's validity or brush reads are shared with a path (SVG, text layer,
  GPU pick compute) that cannot take `lod_row` without a second change.
- Binding one more vertex buffer exceeds a device's `max_vertex_buffers`
  (default 8) for a layer with many channels: report which mark and the count.
- The gather needs the whole chunk's CPU bytes for a released (`GpuOnly`)
  non-shared chunk (it cannot be gathered; the plan then limits LOD to layers
  with CPU data or shared columns: stop and report).
- Changing `DrawEntry` breaks `DrawOrder` spans or the polyline bridge code.

## Out of Scope

- Choosing tiers per frame, `Lod`, the threshold, `Auto` (GUP-716); CPU pick,
  hover and SVG (GUP-718); appends and eviction (GUP-719); culling runs
  (GUP-720); size order (GUP-721).

## Technical Tasks

- [x] `LodGpu` build from `Tiers` and a chunk; storage on `Chunk`; release with
      the chunk.
- [x] `GlueSpec::indexed`, generated WGSL, pick variant, pipeline-key plumbing.
- [x] `DrawEntry::lod`, `LayerGpu::record`, the force seam.
- [x] Tests of AC1-AC3; run the full suite for AC4.

## Dependencies

### Prerequisite Stories

- GUP-714: LOD Tiers in the Column Store 📋 — `Tiers`.
- GUP-664: Shared Columns as the CPU Copy ✅.

### Enables Stories

- GUP-716 (and GUP-718, 719, 720, 721 after it).

## Testing Strategy

- **Unit tests**: gather bytes; glue source with and without `indexed`.
- **Integration tests**: the forced-tier pick, brush and validity test of AC3.
- **Visual validation**: save the forced-tier PNG of the 5,000-row scatter next
  to the full draw and read both: the tier-4 image is the same cloud thinned,
  with no representative off its data position.

## Success Metrics

- [x] Picking and brushing under a forced tier return and dim exactly the data
      rows; extra GPU memory at most 12 B a representative.

## Risk Assessment

- **Medium**: pipeline-key and glue variants multiply (brushed x validity x
  indexed x pick). _Mitigation_: indexed layers are built lazily and only quads.
- **Medium**: the colour pass and the pick pass must agree on the instance
  order. _Mitigation_: both read the same `LodGpu`; AC3.1 checks the id.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the forced-tier PNG read)
- [x] `mask wasm-browser`: not needed, no layout, input or wasm scene changes
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **Glue** (`crates/gup-core/src/shader/glue.rs`): `GlueSpec::indexed` and
  `Glue::indexed`; `LOD_ROW` (`lod_row`) is the last `@location` of `Columns`;
  the vertex entry starts `let row = col.lod_row;` and reads `row` where the
  plain glue reads `instance_index` (validity group and bit, `brush_row`, the
  mark's `vertex`/`vertex_variant`/`vertex_controls` row), so the pick variant
  writes `chunk.row_base + row`. The signature gains the suffix "indexed"; with
  `indexed: false` the source is unchanged (the `scatter_glue`, `nulls_glue` and
  `line_glue*` fixtures pass as they were). A `debug_assert` rejects a polyline;
  `Selection::indexed_glue` returns the `Err`.
- **Column store** (`column.rs`): a chunk's `ChunkLod` (its `Tiers`, the x/y
  columns they were built from, and a `LodGpu` per context) beside `ChunkGpu`,
  dropped by an append or a rewrite and with the chunk on eviction.
  `ColumnStore::lod_tiers(k, xy)` builds the tiers from `cells`/`Cells::get`
  (the values as the GPU sees them); `upload_lod(cx, k)` gathers
  `order[..counts[stored]]` by a strided copy per column (`Cells::cell`: the CPU
  bytes, or a shared value encoded as the upload encodes it) and the rows into
  one packed `VERTEX` buffer, 4-byte aligned, uploaded as `Upload::Column`. A
  released chunk without shared columns is an error naming the chunk.
- **Draws** (`render.rs`, `order.rs`, `selection.rs`, `gpu_pick.rs`):
  `DrawEntry::lod`; `ChunkDraw::indexed`; `LayerGpu::indexed` (the indexed
  program, linked by `Context::indexed_program` into the base program's bind
  group layouts and pipeline layout, so one set of bind groups serves both);
  `LayerPipelines` (the base and indexed pipeline of a target, or of a pick);
  `LayerGpu::record` sets the pipeline itself and switches only when a chunk's
  kind changes. Under a forced tier, prepare points each chunk that stores the
  tier at its representatives (instances = the prefix) and patches
  `chunk_spans`' ends; the `Chunk` uniform entry and validity buffer are the
  chunk's own. Size-ordered layers and polylines are an error.
- **Seams for GUP-716**: `Selection::force_lod_tier(Option<u32>)` (`pub(crate)`)
  stores a `ForcedTier` whose `tier_entries` (non-generic: the checks and the
  per-chunk build) and `Context::indexed_program` are function pointers named
  only by the setter, so a binary that never forces a tier links none of the
  build. `__private::force_lod_tier` and `__private::lod_tier_rows` (hidden)
  serve `lod_spike`. GUP-716 replaces `ForcedTier` by its per-frame choice and
  reuses `lod_tiers`, `upload_lod`, `DrawEntry::lod`, the indexed program and
  `LayerPipelines` unchanged.
- **Measurement** (`examples/lod_spike.rs`): candidates `indexed` (the library's
  own tiers forced to `--lod-tier`, drawn through the indexed glue) and
  `gathered` (the same representatives of the visible chunks as a plain layer,
  as the spike's (B) drew).
- **Tests**: `column.rs` 3 (the gathered bytes equal the chunk's bytes of
  `order[i]` for relative x and y, a constant `F32` column and `U32` codes, from
  owned and shared stores, read back; 12.00 B a representative on a 100,000-row
  walk, 30,271 representatives, 3.63 B a row; a released chunk's error);
  `selection.rs` 2 (the indexed glue fixture `nulls_glue_indexed.wgsl`, its pick
  variant and naga layouts; a polyline's `Err`); `lod/draw_tests.rs` 3 GPU tests
  on a 5,000-row scatter in three chunks forced to tier 4 (AC3.1-3, plus turning
  the tier off again). Mutating the glue's brush or validity row back to
  `instance_index` fails AC3.2 and AC3.3.
- **Test counts**: `mask merge-check` every step PASS (gup-core all features: 69
  binaries, 998 passed, 0 failed, 8 ignored); `mask all-check` exit 0;
  `cargo test -p gup -- --test-threads=1` 11 binaries, 154 passed, 0 failed, 2
  ignored; `cargo test -p gup-visual-regression` 62 passed;
  `mask smoke-examples` pass.

### Definition-of-Done evidence

- **Rendered output**:
  `GUP_LOD_PNG_DIR=… cargo test -p gup-core --lib draw_tests` writes
  `lod-tier4-5000.png` and `lod-full-5000.png`. Read: the tier-4 image is the
  same uniform cloud thinned to about 300 discs, each on a data position (the
  pick test checks every representative's pixel); axes and ticks are unchanged.
- **Goldens unchanged**: no golden was blessed; `git diff --stat 0604d9d3` lists
  no file under any golden directory, and the gup-core golden tests (in
  `merge-check`), `gup-visual-regression` (62) and the glue fixtures pass.
- **WASM** (`mask wasm-size`, scatter gz bytes): main (0604d9d3) 379,228; branch
  381,688 (+2,460, 0.65%; budget check passes, 383,974). Not 0: the scatter
  links the tier branches of `Selection::prepare` (+1.7 KB raw), the second
  pipeline in `Renderer::prepare`/`LayerPipelines` (+1.1 KB raw), the indexed
  branches of `glue::emit` (+0.3 KB raw), `record`'s switch and `ChunkLod`'s
  drop glue. The build itself (tiers, gather, indexed program) is not linked:
  before moving it behind function pointers the delta was +5,471. Attribution:
  `twiggy` diff of the two `wasm-bindgen` outputs.
- **`mask perf-budget`** (load 6.0 then 2.6): every check passes but
  `zoom.cpu.median_ms` (0.951-0.959 against a 0.828 limit) and
  `zoom.cpu.p95_ms`, twice. A same-session A/B of `zoom_bench` built from main
  and from the branch, alternating, 4 runs each (load 1.9-2.6): main 0.917 /
  0.901 / 0.591 / 0.835 (median 0.868), branch 0.915 / 0.902 / 0.551 / 0.845
  (median 0.874): main misses the limit on this machine today too, and the
  branch adds nothing measurable. GPU and transfer checks pass; budgets not
  touched.

| metric                     | recorded | check | limit  | measured | verdict |
| -------------------------- | -------- | ----- | ------ | -------- | ------- |
| zoom.gpu_pass.median_ms    | 2.95     | +20%  | 3.540  | 2.919    | pass    |
| zoom.gpu_pass.p95_ms       | 5.60     | +25%  | 7.000  | 5.624    | pass    |
| zoom.cpu.median_ms         | 0.69     | +20%  | 0.828  | 0.959    | FAIL    |
| zoom.cpu.p95_ms            | 0.86     | +25%  | 1.075  | 1.146    | FAIL    |
| zoom.columns.bytes         | 0        | exact | 0      | 0        | pass    |
| zoom.uniforms.bytes/frame  | 136      | exact | 136    | 136      | pass    |
| zoom_culled.draws.median   | 14       | exact | 14     | 14       | pass    |
| zoom_culled.gpu_pass (ms)  | 9.90     | +20%  | 11.880 | 9.872    | pass    |
| zoom_culled.cpu.median_ms  | 1.07     | +20%  | 1.284  | 1.117    | pass    |
| pipeline.link_create (ms)  | 1.11     | +20%  | 1.332  | 1.169    | pass    |
| wasm.scatter.gz_bytes      | 376445   | +2%   | 383974 | 381688   | pass    |
| wasm.over_wgpu.gz_bytes    | 344085   | ≤     | 400000 | 349328   | pass    |
| wasm.per_row_type.gz_bytes | 1730     | ≤     | 2600   | 1304     | pass    |

### Measurements against the spike

Release, Intel HD 630 (Mesa, Vulkan), headless 1920 × 1080, `lod_spike` 10M
sorted walk, radius 1.5, 30 frames after 3 warm-up; three runs in one session,
alternating candidate order, load 2.6-2.7 (5.0 over 5 minutes). GPU ms is the
scene pass's timestamp median.

| view (tier) | candidate                | drawn   | run 1 | run 2 | run 3 | median | clock MHz |
| ----------- | ------------------------ | ------- | ----- | ----- | ----- | ------ | --------- |
| full (8)    | indexed (this story)     | 30,009  | 2.19  | 2.05  | 1.89  | 2.05   | 350-450   |
| full (8)    | gathered (same rows)     | 30,009  | 2.22  | 2.03  | 2.16  | 2.16   | 350       |
| full        | spike (B), its own tiers | 33,717  | 2.33  | 2.23  | 2.39  | 2.33   | 350-367   |
| zoom (12)   | indexed (this story)     | 106,393 | 2.61  | 3.25  | 2.68  | 2.68   | 533-700   |
| zoom (12)   | gathered (same rows)     | 106,393 | 4.46  | 3.72  | 3.77  | 3.77   | 417-467   |
| zoom        | spike (B), its own tiers | 77,033  | 3.50  | 3.71  | 3.73  | 3.71   | 350       |

- **Draw cost**: drawing the same instances, the indexed draw (one more `u32`
  column) is no slower than the gathered plain draw (full 2.05 against 2.16 ms;
  zoom 2.68 against 3.77, where the indexed runs had the higher GPU clock, so
  read zoom as "not slower", not as faster). The spike's recorded (B): 2.32 /
  3.62 ms; this session 2.33 / 3.71.
- **GPU memory**: 3,540,890 stored representatives (GUP-714's lowest-row tiers)
  × 12 B = 42.5 MB, **4.25 B a data row**, against the spike's 4.9 B at 10M (its
  2.8-5.9 B range; 4.08M representatives). `LodGpu` holds exactly 12 B a
  representative (packed columns, no 256-byte padding; asserted).
- The library's tiers are per chunk (a data-anchored grid per chunk), the
  spike's global, so a fixed tier draws different counts than (B)'s per-view
  tier: tier 8 is the nearest to (B)'s full view, tier 12 to its zoom. Choosing
  the tier per frame is GUP-716's.

## Retrospective

**Completed**: 2026-11-12

### Key Technical Learnings

#### One bind group set for two programs

- **Challenge**: a layer whose chunks draw partly in full and partly as
  representatives needs two pipelines in one pass; separate programs would mean
  separate bind group layouts, so either every bind group twice or a rebind at
  every switch.
- **Solution**: `Context::indexed_program` links the indexed glue into the base
  program's bind group layouts and pipeline layout (the variant adds a vertex
  column, never a binding; a `debug_assert` checks the uniform layouts, LUTs and
  brush binding agree). `record` switches pipelines between chunks and leaves
  groups 0-2 bound.
- **Pattern**: a glue variant that only changes vertex inputs can share its
  base's layouts; key it in the program cache by signature (its "indexed"
  suffix).

#### "Not linked unless used" needs the call graph, not a runtime flag

- **Challenge**: with the forced tier as an `Option<u32>` field read in
  `prepare`, the reference scatter linked the whole build (tiers, gather,
  indexed program): +5,471 B gz although no layer forces a tier.
- **Solution**: the setter stores function pointers (`ForcedTier`:
  `tier_entries`, `Context::indexed_program`), so only a binary that calls the
  setter references the build. +2,460 B gz remains: the draw path's branches
  (the second pipeline, the span patch, `record`'s switch), which every layer
  runs through.
- **Pattern**: to keep an optional feature out of a WASM build, make the code
  reachable only through a value the feature's entry point creates. Moving code
  into non-generic functions or `#[inline(never)]` barely moved the gz total
  here (±100 B): measure each trim, gz sizes shift with layout.

#### Mutation-check the GPU tests

- **Challenge**: the brush and validity tests passed on their first run, which
  proves nothing on its own.
- **Solution**: reverting the glue's `brush_row` and validity reads to
  `instance_index` makes both fail; the pick test fails if the vertex row is
  reverted. The validity test places nulls at the chunk-local indices that
  representatives occupy as instances, which is what a wrong read would hit.

### Architectural Decisions

#### Released chunks: build before release (orchestrator, 2026-11-12)

- **Decision**: B, handed to GUP-716: in the prepare whose release would drop a
  LOD-eligible layer's CPU copy, build the tiers and `LodGpu` of every full
  chunk first; readable stores stay lazy.
- **Reasoning**: an owned layer over 10M rows under `Retain::Auto` (the layers
  LOD is for) loses its CPU copy after the first prepare, and neither the tier
  build nor the gather has another source.
- **Trade-off**: about 0.5 s per 10M rows more on the first frame.

#### One packed buffer per chunk's representatives

- **Decision**: `LodGpu` packs every gathered column and the rows into one
  `VERTEX` buffer at 4-byte alignment, not the chunk's 256-byte column layout.
- **Reasoning**: nothing binds them as storage, so exactly 12 B a representative
  (the AC's bound; the chunk layout's padding would exceed it on small chunks).
- **Future**: a compute pass over representatives (GUP-720's culling runs) would
  need 256-byte column offsets; add the padding then.

#### Forced-tier seam

- **Decision**: `Selection::force_lod_tier` is `pub(crate)`, plus a hidden
  `__private::force_lod_tier` for `lod_spike`, like GUP-714's
  `__private::lod_tier_counts`.
- **Future**: GUP-716 replaces `ForcedTier` with its per-frame tier choice and
  can then drop the `__private` entry (or keep it for the spike).

### Development Workflow Insights

- The worktree sandbox refuses compound commands that mix `git` with other
  steps; plain separate commands, and edit scripts kept under `/tmp/gup715`, ran
  without trouble.
- A same-session A/B with two saved `zoom_bench` binaries settled the `zoom.cpu`
  budget failure in four minutes: main misses it on this machine today too.
- `twiggy top -f csv` on both `wasm-bindgen` outputs, with hashes stripped and
  summed by name, attributes a WASM delta function by function; `wasm_attrib.sh`
  alone prints only the top of one build.

### Follow-up Stories

None. The remaining work is planned: per-frame tiers and the released-chunk
build (GUP-716, Decisions updated), CPU pick, hover and SVG over representatives
(GUP-718), appends (GUP-719), culling runs (GUP-720) and size order (GUP-721).
For the orchestrator:

- `zoom.cpu.median_ms` (0.69 recorded) fails on main on this machine today (0.87
  median in the A/B above); re-record it when the machine is quiet, or look for
  a regression on main.
- GUP-716's default `Lod::Auto` will link the tier build into the scatter (the
  function-pointer seam keeps it out only while nothing forces a tier); expect
  several KB gz there and record it.
