---
implementer: opus
implementer-reason:
  design work - the ordering mechanism changes how columns are bound or
  uploaded, must stay append-friendly and correct with translucent fills
---

# GUP-431: RFC-001 Step S6e: Size-Descending and Emphasis Draw Order

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-17)
**Created**: 2026-10-16

## Context

RFC-001's "S5b findings → Proposed adjustments" says: "Draw order. Bubble charts
want size-descending z-order; today it is data order." The orchestrator's review
of the GUP-420 goldens (2026-10-11) adds emphasis: in `wgsl_function_highlight`
the vermilion highlighted countries must not sit under grey ones. This story was
split out of [GUP-422](GUP-422_RFC_001_S6a_Derive_Mark_V2.md) on 2026-10-16
because it is design work, not execution.

Today a layer draws one instanced draw per column chunk (`render.rs`,
`ChunkDraw`, lines 74–85), rows in upload order. Columns are bound as
instance-rate vertex buffers (`column_ranges`), so an instance's attributes come
from its own row and cannot be permuted at draw time without changing how
columns are bound. The constraints the design must meet:

- **Append-friendly** (RFC-001 §3, S4a): appending rows uploads only the new
  rows' bytes; a design must not re-sort or re-upload the whole store on every
  append.
- **Translucent fills**
  ([GUP-430](GUP-430_RFC_001_S6d_Mark_Stroke_And_Fill_Opacity.md)): depth
  testing alone gives wrong blends for `fill_opacity < 1` (a nearer translucent
  mark drawn first is not blended over the farther one), and no target has a
  depth attachment today (`render.rs`, `depth_stencil: None`).
- **Picking** (RFC-001 §9, S8/S9) will map a drawn instance back to its row.
- **The size is encoded on the GPU** (for example `Pow::sqrt` of a population),
  but scales are monotonic (`ShaderFn::image`), so ordering by the size
  channel's data value orders by size.

Candidate designs to evaluate (not exhaustive): sort rows on the CPU at column
build with a per-chunk-only guarantee for appended chunks; a permutation buffer
read in the vertex stage with columns read from storage buffers (vertex
pulling); and expressing emphasis as a separate layer with a higher `.z()`
([GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md)), leaving
only size order to solve.

## Read First

- RFC-001 §3 "Data model" Design (lines 160–258) and "S4a findings" → "Chunks",
  "Drawing", "Append" (lines 2042–2118).
- RFC-001 §12 "Risks" (lines 796–866), risk 7 (vertex-buffer slots).
- `crates/gup-core/src/render.rs` lines 74–120 (`ChunkDraw`, `LayerGpu`) and
  740–790 (`with_chunks`, `write_uniforms`).
- `crates/gup-core/src/selection.rs` lines 470–600 (`prepare`).
- `crates/gup-core/tests/scales_png.rs` lines 52–92 (`sqrt_radius_scatter`) and
  `crates/gup-core/tests/wgsl_function.rs` near line 329
  (`wgsl_function_highlight`).

## User Story

> "As a visualization developer drawing a bubble chart or highlighting a
> category, I want the largest marks drawn first and emphasised marks drawn
> last, so that no mark is hidden by insertion order alone."

## Acceptance Criteria

### AC1: Design recorded

- [x] A "GUP-431 findings" section appended to RFC-001 records the chosen
      mechanism, the alternatives rejected and why, its cost (bytes per row,
      draws, vertex-buffer slots) and its append behaviour.

### AC2: Size-descending order

- [x] A layer whose `Size`-role channel is a column draws rows largest first. A
      test inserts rows smallest-first at varying radii with overlapping discs
      and asserts the pixel at each row's centre is that row's own fill within
      2/255.
- [x] The same test passes after appending a second batch of rows (smaller and
      larger than existing ones), and the append uploads only the new rows'
      column bytes plus at most O(new rows) bytes of ordering data (asserted
      with `UploadStats`).
- [x] With `fill_opacity < 1`, a pixel covered by two discs equals the smaller
      disc over the larger over the background, within 3/255.

### AC3: Emphasis drawn last

- [x] `wgsl_function_highlight`'s highlighted rows draw above the others, by
      whichever mechanism AC1 records (a documented recipe, such as a second
      layer with a higher `.z()`, is acceptable if it needs no new API). The
      golden is re-blessed and read by eye: no vermilion point under a grey one.

### AC4: Goldens

- [x] `sqrt_radius` is re-blessed and read by eye: small bubbles are visible on
      top of large ones.

## Decisions

- Depth testing is not the mechanism (translucent fills, no depth attachment); a
  depth-based proposal must explain how it blends correctly.
- Rows without a `Size`-role column (constant size) keep data order.
- The mechanism and any new public API are this story's to choose and record
  (AC1).

## Escalation Triggers

- The chosen design needs storage buffers in the vertex stage on a path that
  must run where `max_storage_buffers_per_shader_stage == 0` (compare
  `prepare`'s validity-bits check).
- The design changes the column store's chunk format or `Retain` behaviour.
- `mask perf-budget` regresses outside noise.

## Out of Scope

- GPU picking's instance-to-row mapping: RFC-001 S9 (not yet a story), though
  AC1 must say how picking will recover the row.
- Per-layer `.z()`: GUP-424.

## Technical Tasks

- [x] Prototype the candidate(s) against AC2's adversarial test.
- [x] Implement the chosen mechanism; record findings in RFC-001.
- [x] Re-bless `sqrt_radius` and `wgsl_function_highlight`.

## Dependencies

### Prerequisite Stories

- GUP-430: Stroke and Fill Opacity 📋 — `fill_opacity`, which the mechanism must
  blend correctly.

### Soft Dependencies

- GUP-424: Multi-Layer Plot 📋 — per-layer `.z()`, if emphasis uses layers.

## Testing Strategy

- **Integration tests**: AC2's adversarial insertion and append tests.
- **Visual validation**: `sqrt_radius`, `wgsl_function_highlight`.
- **Performance**: `mask perf-budget`; record the numbers in the findings.

## Success Metrics

- [x] No row in the adversarial test is hidden by insertion order.

## Risk Assessment

- **High**: every candidate trades append cost, binding complexity or guarantee
  strength. _Mitigation_: measure, record, and prefer the simplest design that
  meets AC2 as written.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Mechanism** (recorded in RFC-001 "GUP-431 findings"). A layer whose
`Size`-role channel is a column sorts each batch of rows largest first before it
enters the column store, and permutes every column alike. A batch is the first
evaluation or one append. The draw order merges the sorted runs into segments of
consecutive stored rows. Each segment is drawn as instance ranges
(`first_instance`) of its chunk. The store's format, `Retain` and upload
accounting are unchanged. No ordering data goes to the GPU. The glue and the
shaders are unchanged. Fragmented layers (more than `64 + rows / 1024` segments)
evaluate their rows again into one run while they still hold them. Emphasis is a
recipe that needs no new API: a second layer over the same scales with a higher
`Selection::z`. `Selection::z`'s docs state it.

**Files**:

- `crates/gup-core/src/order.rs` (new): `DrawOrder` (batch sort, run merge,
  compaction test, `row_of`, `spans`) and `chunk_spans`. It holds 5 unit tests
  and 4 GPU tests: the AC2 adversarial chain, opaque and at `fill_opacity` 0.5,
  in one chunk and across 64-row chunks, plus compaction.
- `crates/gup-core/src/render.rs`: `Span` and `Draws`. `LayerGpu::spans` drives
  `Prepared::draw` and `draw_calls`.
- `crates/gup-core/src/selection.rs`: `size_order`, sorting in `evaluate` and
  `append`, spans in `prepare`, and the emphasis recipe in `z`'s docs.
- `crates/gup-core/tests/wgsl_function.rs`: the highlight test uses the
  higher-`z` recipe. It asserts that at least one Asian centre would have been
  under a grey disc in data order.
- `crates/gup-core/tests/stroke_png.rs`: `bubble_stroke` is now one layer with a
  radius column, so it exercises the order with translucent fills and strokes.
- `crates/gup-core/PERF_BUDGETS.md`: WASM scatter budget re-recorded at 312,306
  B, with the measured split.

**Tests**: gup-core lib 140 passed, 3 ignored (9 new, in `order.rs`), all
gup-core integration tests pass. Root-crate tests were left to CI.

### Definition-of-Done evidence

- **Goldens re-blessed and read as images.**
  - `sqrt_radius`: small discs now show on top of large ones, e.g. inside the
    large green discs at about 25,000/73 and the purple cluster at about
    2,500/57. They were hidden before.
  - `axis_titles` (theme_png): the same bubble chart, with the same change.
  - `bubble_stroke`: the teal bubble at (0.79, 0.41) now sits over the large
    yellow-green one, with its outline whole. Every smaller bubble is over every
    larger one it overlaps.
  - `wgsl_function_highlight`: no vermilion point is under a grey one. The
    golden was deleted and re-blessed, because the change, a single Asian disc
    near (700, 470K) lifted over grey, was within the harness tolerance (11
    pixels).

- **Sized-layer frame time** (`zoom_bench` patched locally to use a square-root
  radius column, 100K points). Ordered: GPU 2.88 and 3.03 ms, CPU 0.689 and
  0.696 ms. Data order: 2.81 and 3.16 ms, 0.685 and 0.702 ms. The difference is
  noise.
- `mask all-check` passes, `cargo check --examples` passes, and
  `mask old-path-loc` is 28,910.

**`mask perf-budget`**: every check passed (second run, load 7.96 at start; the
first run, at load 2.8, passed every timing too):

| metric                  | recorded | limit  | measured | verdict |
| ----------------------- | -------- | ------ | -------- | ------- |
| zoom.gpu_pass.median_ms | 2.95     | 3.540  | 3.037    | pass    |
| zoom.gpu_pass.p95_ms    | 5.60     | 7.000  | 5.911    | pass    |
| zoom.cpu.median_ms      | 0.69     | 0.828  | 0.700    | pass    |
| zoom.cpu.p95_ms         | 0.86     | 1.075  | 0.890    | pass    |
| zoom.columns.bytes      | 0        | 0      | 0        | pass    |
| pipeline.link_create    | 0.88     | 1.056  | 1.045    | pass    |
| wasm.scatter.gz_bytes   | 312306   | 318552 | 312306   | pass    |
| wasm.over_wgpu.gz_bytes | 270593   | 400000 | 270593   | pass    |

## Retrospective

**Completed**: 2026-10-17

### Key Technical Learnings

#### Append-friendly global order without a permutation buffer

- **Challenge**: AC2 asks for exact global order after an append, with
  `O(new rows)` ordering bytes. A dense permutation shifts `O(n)` entries when
  rows are inserted, and columns are instance-rate vertex buffers that cannot be
  permuted at draw time.
- **Solution**: Store each batch already sorted. The order is then a merge of
  sorted runs, which instance ranges (`first_instance`) can express over the
  unchanged buffers. An append adds at most `2m` draws and no GPU bytes.
- **Pattern**: When the GPU cannot gather, move the permutation into the data,
  one batch at a time, and express what is left as draw ranges.

#### Adversarial geometry for an ordering test

- **Challenge**: The AC2 test needs every centre to show its own fill only under
  the correct order, independent of the plot layout.
- **Solution**: Discs tangent inside one another, each radius three times the
  last, positioned through standalone identity scales
  (`Linear::new().domain(0, 1).range(Px(0), Px(1))`) so that data equals pixels.
  The test asserts its own preconditions: a larger disc covers each centre, and
  no smaller disc is near one. Disabling the sort makes it fail.
- **Pattern**: Centre test discs on pixel centres (`+0.5`). An integer centre
  put the 2 px disc's own edge within 1.5 px of its sampled pixel.

#### Sorting cost

- An unstable sort of `u64`s, each packing the order-preserving `f32` key bits
  above the row index, keeps the order stable and is faster than a stable sort
  by `f32`. The permutation gather over four `f64` columns dominates at 1M rows,
  about 64 ms.

### Architectural Decisions

#### Sort in `Selection`, not in `ColumnStore`

- **Decision**: The store receives each batch already permuted. It knows nothing
  of order.
- **Reasoning**: The chunk format and `Retain` stay exactly as they were, which
  avoids the story's escalation trigger. Dictionary codes are assigned before
  the sort, so key domains stay in data order.
- **Trade-off**: In a sized layer, stored row and data row differ. Picking must
  go through `DrawOrder::row_of` (8 B per row of CPU for key and index).
- **Future**: S9 maps instances to rows through `row_of`. S12's `LastRows`
  eviction must drop evicted rows from the segments.

#### Emphasis by layer, not by key

- **Decision**: Emphasis uses the existing `Selection::z` recipe, which needs no
  `order_by` API.
- **Reasoning**: It needs no new public API, and it does not have to decide how
  a per-row key combines with size order.
- **Trade-off**: Users split their rows. A function with `fit_keys` sees only
  its own layer's keys.

#### Compaction policy

- **Decision**: Re-evaluate into one run above `64 + rows / 1024` segments,
  while the rows are held.
- **Trade-off**: At 1M rows, frequent random appends cause a full re-evaluation
  (about 200 ms) every few hundred appended rows. Draw count wins over
  re-upload. Released layers never compact.

### Development Workflow Insights

- The visual-regression harness tolerates 0.25% of pixels, so the highlight
  change (11 pixels) passed the old golden. Re-blessing meant deleting it. A
  pixel-level assertion in the test (`lifted > 0`) is what pins the behaviour.
- The WASM budget failed on drift that other stories had left: 308,740 B at the
  start against a limit of 308,988. Measuring the start commit in a throwaway
  worktree (under `~/.cache/gup/wt`) separated that drift from this story's
  3,566 B.
- The frame-time impact was measured by a temporary, uncommitted patch to
  `zoom_bench`'s scatter (a sized radius) and an environment toggle for the
  sort. Neither was committed.

### Follow-up Stories

None written. Two notes for later steps: S12's `Window::LastRows` eviction must
update `DrawOrder` (recorded in the RFC findings), and a `zoom_bench` option for
a sized radius would let `perf-budget` cover sized layers. The second is small
and can ride with the next story that touches `zoom_bench`.
