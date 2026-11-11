---
implementer: sonnet
implementer-reason:
  the rules (anchored grid, doubling rebuilds, whole-chunk eviction) are in the
  RFC findings and Decisions; the work follows `RowWindow` eviction and the
  append path
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/src/lod.rs,
    crates/gup-core/tests/window.rs,
    crates/gup-core/tests/lod.rs,
  ]
---

# GUP-719: LOD With Appends and Windows

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-11

## Context

GUP-714's grid is anchored per chunk, so an append that grows the layer's extent
leaves built chunks valid, but nothing yet says what happens to the chunk being
appended to, or to a chunk `RowWindow::LastRows` evicts. RFC-001 "GUP-631
findings" ("Interaction under LOD") gives the rules: appends invalidate only the
tail chunk's tiers; the tail draws in full until it holds 65,536 rows, then
rebuilds its tiers at doubling sizes (amortised `O(n log n)`, about 65 ms per 1M
rows); eviction drops whole chunks with their tiers, the stored row being
chunk-local so renumbering rewrites nothing; a tier rebuild changes which
instance draws which row, so it must bump the layer's pick epoch
(`Selection::epoch`, `crates/gup-core/src/selection.rs` ~1561).

Windowed chunks hold at most `max(1024, n / 4)` rows (`RowWindow::chunk_cap`,
`selection.rs` ~190), so a window under 800,000 rows has chunks under 200,000
and a layer under 200,000 visible rows never takes LOD anyway
(`Lod::AUTO_MIN_ROWS`).

## Read First

- RFC-001 "GUP-631 findings": "Interaction under LOD" (Window and Appends
  bullets).
- RFC-001 "S12 findings" (lines 3671-3745): eviction by whole chunks, the
  `epoch`.
- `crates/gup-core/src/selection.rs`: `RowWindow` (line ~170), `chunk_cap`, the
  `epoch` field and where it is bumped (search `epoch += 1`), the append path.
- `crates/gup-core/src/column.rs`: `Chunk` (`len`, `capacity`, the append and
  eviction methods), GUP-715's `LodGpu` on the chunk.
- `crates/gup-core/tests/window.rs` (`window_run`, lines 1-265) and
  `tests/append.rs`.

## User Story

> "As a developer of a live scatter that streams millions of rows, I want LOD to
> follow appends and a rolling window, so that a live chart stays within frame
> budget without rebuilding everything on each batch."

## Acceptance Criteria

### AC1: The tail chunk

- [ ] A tail chunk with fewer than 65,536 rows draws every row (no `Tiers`,
      built count 0, asserted) even when the layer is above 200,000 rows
      overall.
- [ ] From 65,536 rows the tail builds `Tiers` and rebuilds when its row count
      has doubled since the last build (65,536, 131,072, 262,144, 524,288 ...,
      capped by the chunk size). A test feeding 1,000-row batches up to a
      1,048,576-row chunk counts builds: exactly 5 (at 65,536; 131,072; 262,144;
      524,288; 1,048,576).
- [ ] Between rebuilds the rows past the last build draw in full (a second
      instanced range after the prefix) so a new row is visible at once; a test
      appends one far-out row after a build and finds it drawn (its pixel is
      coloured) in the next frame.
- [ ] Total build time to append 1,048,576 rows in 1,000-row batches is at most
      2.5x the time of one build of that chunk (the amortised bound; assert and
      print both).

### AC2: Full chunks are untouched by appends

- [ ] A test appends rows that extend the layer's x and y extents ten-fold while
      tiers of earlier full chunks exist: those chunks' `Tiers` are not rebuilt
      (build counter unchanged) and their draws are identical.

### AC3: Windows

- [ ] `RowWindow::LastRows(2_000_000)` fed in 100,000-row batches to 6,000,000
      rows: evicted chunks' `Tiers` and `LodGpu` are dropped with them (a
      `gpu_bytes` or counter before/after shows the LOD memory bounded by the
      window's chunks, assert at most 6 B a held row plus the tail), eviction
      writes no column bytes (the assertion `window_run` makes), and the picture
      equals a fresh unwindowed layer of the held rows within
      `DiffTolerance::default()` under `Lod::Off`, and within 1.0% of plot-rect
      pixels at ΔE00 > 10 under `Auto`.
- [ ] The pick epoch: a tier rebuild (AC1) and an eviction bump
      `Selection::epoch` (a test: a GPU pick requested before a rebuild is
      dropped, one after is answered with the data row, as for an eviction).
- [ ] Row ids after eviction: `Layer::row_id` of a representative in the oldest
      held chunk is the right data row (the chunk-local row plus `row_base` plus
      the evicted count).

### AC4: Frame cost

- [ ] A 10-frame append loop, 100,000 rows a frame, windowed 2,000,000 rows,
      release: the p95 CPU frame time (resolve + prepare + draw) is under 8 ms
      (the build runs at most once per chunk per frame, GUP-716's scheduling);
      the retrospective prints it.

## Decisions

- **65,536 and doubling** (from RFC-001): below it a full draw is cheap (about 5
  ms at 100K sorted rows in the spike), above it a rebuild costs under 5 ms and
  happens `log2(chunk/65,536)` times a chunk.
- **Rows past the last build draw in full** as a second range so a streamed
  point is never invisible; it is bounded by the doubling (at most as many rows
  as the build covers).
- **Epoch bump on rebuild, not per frame**: rebuilds are rare, and the bump
  invalidates GPU picks of the old instance mapping only.
- **No incremental tiers**: a rebuild is from scratch (GUP-714's `build`).
- **Windows below 800,000 rows never LOD** (chunks of up to `n / 4`, threshold
  200,000): documented in the `Selection::lod` and `RowWindow` docs.

## Escalation Triggers

- A rebuild invalidates the chunk's `LodGpu` while a frame's draw is recorded
  against it (buffer lifetime): stop and report where.
- Eviction frees a chunk that a pending GPU pick read-back still references.
- AC1's amortised bound fails (a rebuild per batch).

## Out of Scope

- Incremental tier maintenance; LOD for lines or areas in windows; size-ordered
  layers (GUP-721); spatial runs (GUP-720).

## Technical Tasks

- [ ] Build scheduling for the tail chunk (threshold, doubling, rest-of-rows
      range), `Tiers` and `LodGpu` drop on eviction.
- [ ] Epoch bump on rebuild.
- [ ] Tests (AC1-AC4) in `tests/lod.rs` and `tests/window.rs`; a doc note on
      `Selection::lod` and `RowWindow`.

## Dependencies

### Prerequisite Stories

- GUP-716: `Selection::lod` and Per-Frame Tier Draws 📋.
- GUP-482: RFC-001 S12c `RowWindow` eviction ✅.

### Enables Stories

- GUP-632's live-data variants (soft).

## Testing Strategy

- **Unit tests**: build counter at each doubling; extent growth leaves full
  chunks.
- **Integration tests**: windowed stream parity (`Off` exact, `Auto` by ΔE00),
  epoch, row ids.
- **Visual validation**: the windowed-stream PNG under `Auto` read beside the
  fresh plot's: same cloud, newest rows at the right edge.
- **Performance**: AC4's loop.

## Success Metrics

- [ ] Appends stay amortised (at most 2.5x one build); the window's LOD memory
      is bounded by its chunks.

## Risk Assessment

- **Medium**: a rebuild's cost (about 65 ms per 1M rows) lands on one frame.
  _Mitigation_: AC4's p95 and the once-per-frame scheduling.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the windowed-stream PNG)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm scene changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
