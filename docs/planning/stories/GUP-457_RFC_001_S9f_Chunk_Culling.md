---
implementer: sonnet
implementer-reason:
  the visibility rule, margins, bridge rule, where the filter lives and the
  bench flags are fixed in Decisions; it filters spans that already exist
---

# GUP-457: RFC-001 Step S9f: Chunk Culling for Draws and Picks

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-18)

## Context

The column store splits a layer into chunks of at most 2^20 rows, each with f64
min/max stats per column (S4a). Every chunk is drawn every frame, whatever the
view. RFC-001 planned "CPU chunk culling first" (§12 risk 15). The S4a and S5a
findings name it as what makes deep zoom into large data cheap. S5a's scenario
(a) has 94.7M rows in 91 chunks, and without culling every row is drawn at every
zoom.

Picking has the same cost. The CPU pick index is rebuilt on every zoom from
every row of every layer (`Plot::pick_index`, `Layer::hit_items`). The GPU pick
draws every span of the last resolve (`Plot::drawn`). Nothing outside the plot
rect can be picked or seen, so a chunk whose marks all lie outside the plot rect
(plus the pick tolerance) can be skipped by all three paths. Because the CPU and
GPU picks must keep agreeing (GUP-455's oracle: 10,000/10,000), both paths use
the same visibility result.

Culling only helps when chunks are spatially coherent: time series, or data
sorted on x. For the reference scatter (random x), every chunk spans the whole x
range and nothing is culled. That case must cost almost nothing.

## Read First

- RFC-001 §12 risk 15 (line 859), "S4a findings" → "Proposed adjustments" → S9
  (lines 2186–2190), "S5a findings" frame time and "Proposed adjustments" → S9
  (lines 2468–2502, 2619–2622), "S9d findings" → "The scheme" (lines 3455–3480).
- `crates/gup-core/src/render.rs` lines 74–135 (`ChunkDraw`, `Span`, `Draws`,
  `LayerGpu`) and 1440–1490 (`draw_calls`, `Prepared::draw`).
- `crates/gup-core/src/order.rs` `draw_entries` (line 228): what a polyline's
  bridge entry is.
- `crates/gup-core/src/selection.rs` `overhang` (726–753) and `hit_items`
  (985–1060).
- `crates/gup-core/src/plot.rs` lines 1394–1411 (`pick_index`) and 1660–1690
  (the prepare loop and `self.drawn`); `src/gpu_pick.rs` `request_pick` (232+).
- `crates/gup-core/src/column.rs` `ColumnStats` (320–410).
- `crates/gup-core/examples/zoom_bench.rs` lines 1–110 and `zoom` (208–222);
  `scripts/perf_budget.sh` lines 70–105.
- `crates/gup-core/PERF_BUDGETS.md` "Budget table" and "CI-side proxies".

## User Story

> "As a developer zoomed deep into millions of time-ordered rows, I want only
> the chunks on screen to be drawn and picked, so that zoom, hover and click
> stay fast however much data is off screen."

## Acceptance Criteria

### AC1: Visibility rule

- [ ] A `pub(crate)` function computes, per layer, which draw entries are
      visible, from the rule in Decisions. Unit tests cover:
  - a circle chunk wholly left of the plot rect is culled;
  - one whose pixel box ends within the margin is kept;
  - a `Rect` chunk with a `Derived` width whose centres lie just outside but
    whose half-width reaches inside is kept;
  - a `Segment` chunk with both ends outside, on opposite sides, is kept;
  - a polyline bridge between two culled chunks on opposite sides of the view is
    kept;
  - a chunk whose position stats are `None` (all null) is culled;
  - a layer with constant positions is kept or culled by that one point.

### AC2: Images are unchanged

- [ ] In `crates/gup-core/tests/zoom_uploads.rs`, a plot of 64 x-sorted chunks
      (`max_chunk_rows(1024)`, 65,536 circles) is zoomed to x ∈ chunk 20.5…23.5.
      It renders byte-identical with culling on and with
      `Plot::chunk_culling(false)`. `Prepared::draw_calls` counts exactly the
      chunks the rule keeps, plus guides, and fewer than the unculled count.
- [ ] The same byte-identity holds for a `Line` of 4,096 rows in 64-row chunks,
      zoomed so the only visible segment has both ends off screen in two
      different chunks, and for `Rect` bars with a `Derived` width.
- [ ] The existing draw-count proxies in `tests/zoom_uploads.rs` (4 draw calls
      for 1 chunk, 10 for 7) pass unchanged, since random data culls nothing.

### AC3: Picks agree and skip culled chunks

- [ ] At the AC2 circle zoom, 10,000 random points in the plot rect give the
      same `pick_row` with culling on and off, and `pick_row_gpu` agrees with
      `pick_row` on all of them. Test: `tests/pick.rs`.
- [ ] The CPU pick index holds items only from visible entries. A test counts
      the items (a `#[cfg(test)]` accessor) at the AC2 zoom: at most the rows of
      the kept chunks.
- [ ] A brush at the AC2 zoom selects the same rows with culling on and off.

### AC4: Zoom writes nothing new

- [ ] Over 300 zoomed frames of the AC2 plot, culling writes 0 column, validity
      and texture bytes. Uniform and instance writes per frame are the same as
      with culling off.

### AC5: Measured

- [ ] `zoom_bench` gains `--sorted`, `--no-cull` and `--metric-prefix NAME`
      (Decisions).
- [ ] Three alternating runs each of culling on and off with
      `--points 4194304 --chunk-rows 65536 --sorted --present mailbox --uncapped`.
      With culling on, the GPU pass median is at least 25% below culling off.
      Record both in RFC-001 (AC7).
- [ ] `PERF_BUDGETS.md` gains the rows in Decisions, and
      `scripts/perf_budget.sh` runs that second `zoom_bench` and checks them.
      `mask perf-budget` passes. The existing `zoom.*` rows do not move beyond
      their tolerances.

### AC6: Size

- [ ] `mask wasm-size` passes. Growth over 2,000 B gz is an escalation.
      `wasm.scatter.gz_bytes` is re-recorded with the reason if it moves more
      than +2%.

### AC7: Record

- [ ] RFC-001 gains "S9f findings": the rule, the on/off timings (GPU pass, CPU,
      draws per frame) and the pick-index build time at the AC5 size, with and
      without culling, at the deepest zoom.

## Decisions

- **Unit of culling**: a draw entry (`order::draw_entries`), the thing a `Span`
  refers to. A column-chunk entry's box comes from that chunk's stats. A
  polyline bridge entry's box is the union of the boxes of the two chunks it
  joins.
- **Box**: for each position channel (role `X` or `Y`: `x`, `x_end`, `y`,
  `y_end`), its pixel extent in the current scales:
  - a column: `DynColumnEncoding::image(stats.extent())` of that chunk's stats
    (scales are monotonic, so the image of the extent bounds every value);
  - `Const` or `Derived`: that value;
  - unset: the channel default.

  The x interval is the union over the X-role channels and the y interval the
  union over the Y-role channels. A chunk with no finite position value (stats
  `None`) is culled.

- **Margin**: `PICK_TOLERANCE + 1.0 + m`, where `m` is the largest value any
  non-position `f32` channel can take in the layer (radius, width, height,
  stroke width, opacity): `Const` or `Derived` values, or the upper end of
  `image` over the column's merged stats. This is a new
  `Layer::cull_margin(&self) -> f32` beside `overhang`; `overhang` is unchanged.
  The margin is conservative on purpose: it never culls a mark that could draw
  or pick inside the plot rect.
- **Visible**: the box, grown by the margin on every side, intersects the plot
  rect. Otherwise the entry is culled.
- **Where**: in `Plot::resolve`, after the layers are prepared and the scales
  ranged, on every resolve that is not a cache hit (so after every zoom or pan).
  Each layer's `MarkBatch` carries `visible: Arc<[Span]>`, its spans in draw
  order without the culled ones. `Draw::Marks` records those spans, and so does
  `self.drawn` for the GPU pick. `LayerGpu`, its buffers and `Prepared::draw`'s
  signature are unchanged. No GPU write is added.
- **CPU picks**: `Layer::hit_items` gains a `visible: &[bool]` parameter (one
  per draw entry, from the same computation). It skips the rows of culled
  chunks. A polyline row whose segment is a kept bridge is kept.
- **Seam**:
  `#[doc(hidden)] pub fn Plot::chunk_culling(&mut self, on: bool) -> &mut Self`,
  default on, bumping the revision. It is for the tests and `zoom_bench`.
- **zoom_bench**:
  - `--sorted` sorts the rows by x before building.
  - `--no-cull` calls `chunk_culling(false)`.
  - `--metric-prefix NAME` (default `zoom`) prefixes every `metric` line.
- **Budget rows** (measured by `perf_budget.sh`'s second run, with
  `--metric-prefix zoom_culled`):
  - `zoom_culled.draws.median`: exact
  - `zoom_culled.gpu_pass.median_ms`: +20%
  - `zoom_culled.cpu.median_ms`: +20%
  - `zoom_culled.columns.bytes`: exact
  - `zoom_culled.gpu_clock.median_mhz`: info

  `zoom_bench` prints `draws.median` as the median over measured frames of
  `Prepared::draw_calls`. It is deterministic, because the zoom path is.

## Escalation Triggers

- Any golden image changes, or AC2's byte-identity fails for a case the rule
  keeps.
- `pick_row` and `pick_row_gpu` disagree on any AC3 point.
- `DynColumnEncoding::image` is not available, or not monotonic, for some
  library scale on a position channel (for example a `Band` chain).
- With culling on, the AC5 GPU pass median is less than 25% below culling off.
- `mask wasm-size` grows by more than 2,000 B gz.

## Out of Scope

- Render bundles and `multi_draw_indirect` (§12 risk 15, T7).
- Culling rows within a chunk, LOD and density (T7, `gup-culling-lod`).
- The 94.7M-row scenario itself (needs ~5 GB of memory).

## Technical Tasks

- [ ] `Layer::cull_margin`; the visibility function and its unit tests (AC1).
- [ ] `MarkBatch::visible`, used by `Draw::Marks` and `self.drawn`; the
      `hit_items` parameter; the `chunk_culling` seam.
- [ ] AC2/AC3/AC4 tests in `tests/zoom_uploads.rs` and `tests/pick.rs`.
- [ ] `zoom_bench` flags; `perf_budget.sh` second run; budget rows (AC5).
- [ ] RFC-001 "S9f findings"; `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-455: RFC-001 S9d GPU Pick Pass ✅ — the GPU pick that must agree.
- GUP-456: RFC-001 S9e GPU Pick in Hover and Hosts ✅ — `self.drawn` use in
  hover.

### Enables Stories

- RFC-001 S12 (`appender`, `Window`): appended chunks off screen cost nothing to
  draw.

## Testing Strategy

- **Unit**: the visibility rule on hand-built stats (AC1).
- **Integration**: byte-identity on/off, draw counts, upload counts, pick
  agreement (AC2–AC4).
- **Performance**: `zoom_bench` on 4M sorted rows, on and off (AC5).

## Success Metrics

- [ ] Draws per frame at deep zoom fall roughly in proportion to the visible
      share of the data, with identical images.
- [ ] The GPU pass at the AC5 size is at least 25% faster with culling.

## Risk Assessment

- **Medium**: a missed overhang culls a visible mark. _Mitigation_: the margin
  takes every non-position `f32` channel's maximum, and AC2 checks byte-identity
  for circles, lines and bars.
- **Low**: per-resolve cost on unsorted data. _Mitigation_: one box per chunk;
  64 chunks is microseconds. AC4 shows no new writes.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask perf-budget` and `mask wasm-browser` pass; no golden re-blessed
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Resolved (2026-10-18)

Accepted by the orchestrator: `wasm.scatter.gz_bytes` 312,427 to 315,608 B gz
(+3,181; over wgpu 280,067 to 283,248). The orchestrator re-records the budget
row at merge.

## Implementation Summary

- `src/cull.rs`: `PixelBox`, `visible_entries`, `pick_ranges`, `visible_spans`
  (10 unit tests). `Layer::cull_margin` and `Layer::visible_entries`;
  `hit_items` takes `visible` and decodes and builds items only for visible
  chunks.
- `MarkBatch::visible` feeds `Draw::Marks` and `Plot::drawn` (GPU pick), so
  draws and both picks share one result. `Plot::chunk_culling` and
  `pick_item_count` are doc(hidden).
- A `Rect` with a constant non-zero width or height is boxed as centre +- extent
  (its unset far edge is ignored).
- `zoom_bench`: `--sorted`, `--no-cull`, `--metric-prefix`, `draws.median`,
  pick-index timing. `perf_budget.sh` runs a second `zoom_culled` pass; five
  `zoom_culled.*` rows added.
- Tests: 5 in `zoom_uploads.rs`, 4 in `pick.rs`; no golden changed.

## Retrospective

**Completed**: 2026-10-18

### Key Technical Learnings

#### Unset channels widen boxes

- **Challenge**: The Decisions rule boxes an unset position channel at its
  default, which stretched bars toward pixel 0 so only left chunks culled.
- **Solution**: For `AlignedBox` marks, ignore `x_end`/`y_end` when width/height
  is a non-zero constant.
- **Pattern**: Derive the box from what the shader draws, per mark mode.

#### Pick index cost

- **Challenge**: Culled pick index still took 92 ms at 4M rows.
- **Solution**: Decode only needed chunks; the NaN-filled per-channel vectors
  and `rank` table remain O(rows).
- **Pattern**: Make those sparse for a further win (follow-up).

### Architectural Decisions

#### Culling result lives in the MarkBatch

- **Decision**: `visible: Arc<[Span]>` per batch; `LayerGpu` unchanged.
- **Reasoning**: No GPU write; one result for three consumers.
- **Trade-off**: A small allocation per uncached resolve.
- **Future**: S12 appends cost nothing off screen.

### Development Workflow Insights

- An unknown `image` (None) keeps the chunk, so culling never hides a mark.
- Escalation on wasm size (+3,181 B) was accepted by the orchestrator.

### Follow-up Stories

None written; sparse pick-index vectors are noted above (GUP-468 reserved,
unused).
