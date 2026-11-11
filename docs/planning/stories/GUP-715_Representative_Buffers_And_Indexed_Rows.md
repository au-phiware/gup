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

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-11

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

- [ ] For a chunk with `Tiers` and a layer's column formats, a `LodGpu` holds
      the representatives `order[..counts[stored]]` gathered into the same
      column layout, formats and per-chunk origins as the chunk's own buffer (so
      the existing pipelines read them), plus a `u32` buffer of chunk-local
      rows. Gathered from the chunk's CPU bytes or, for a shared-column chunk,
      from the shared columns (`ColumnStore::value`, GUP-664). A test asserts
      the GPU bytes of representative `i` equal the chunk's bytes of row
      `order[i]` (read back), for `F32Relative` x and y and a constant-encoded
      channel.
- [ ] Memory: `LodGpu` of a chunk is at most 12 B a representative for an x/y
      layer (8 B columns + 4 B row), asserted by a size check on a 100,000-row
      chunk.

### AC2: The indexed-rows glue

- [ ] `GlueSpec` gains `indexed: bool` (false: the glue source is byte-for-byte
      what it is today, asserted by the existing glue-source tests passing
      unchanged). When true, one extra `u32` instance column `lod_row` is bound
      after the channels' columns, the vertex entry reads `let row = lod_row;`
      and uses it where it used `instance_index` for `brush_row`, the validity
      group and bit, and the `row` argument it passes to the mark's
      `vertex`/`vertex_variant`; encodings still read the gathered columns at
      the instance. The pick variant's id is `chunk.row_base + row`.
- [ ] For a polyline geometry `indexed` is rejected (a `debug_assert` and an
      `Err` from the spec builder): LOD is quads only (Decisions).

### AC3: Drawing a forced tier, picking and brushing the data rows

- [ ] `DrawEntry` gains `lod: Option<u32>` (the representative count drawn from
      `LodGpu`; `None` is today's entry and `draw_entries` produces `None`).
      `LayerGpu::record` binds `LodGpu` buffers and the indexed pipeline for
      entries with `Some`, and the chunk's own otherwise.
- [ ] A `pub(crate)` seam `Selection::force_lod_tier(Option<u32>)` (test-only
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
      data row's (a null elsewhere does not blank it).

### AC4: Nothing else moves

- [ ] With the seam unset every existing test and golden passes unchanged;
      `mask wasm-size` reports the scatter gz bytes (the indexed variant is not
      linked unless a layer uses it: record the delta, expected 0).

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

- [ ] `LodGpu` build from `Tiers` and a chunk; storage on `Chunk`; release with
      the chunk.
- [ ] `GlueSpec::indexed`, generated WGSL, pick variant, pipeline-key plumbing.
- [ ] `DrawEntry::lod`, `LayerGpu::record`, the force seam.
- [ ] Tests of AC1-AC3; run the full suite for AC4.

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

- [ ] Picking and brushing under a forced tier return and dim exactly the data
      rows; extra GPU memory at most 12 B a representative.

## Risk Assessment

- **Medium**: pipeline-key and glue variants multiply (brushed x validity x
  indexed x pick). _Mitigation_: indexed layers are built lazily and only quads.
- **Medium**: the colour pass and the pick pass must agree on the instance
  order. _Mitigation_: both read the same `LodGpu`; AC3.1 checks the id.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the forced-tier PNG read)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm scene changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
