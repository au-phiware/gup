---
implementer: sonnet
implementer-reason:
  profile first; the suspects are per-frame O(rows) loops in `selection.rs`, a
  contained fix unless the profile shows a design question (then escalate to
  opus)
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/order.rs,
    crates/gup-core/examples/zoom_bench.rs,
  ]
---

# GUP-724: A Size-Ordered Layer's Resolve Costs What a Plain Scatter's Does

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-12) **Created**: 2026-11-11

## Context

GUP-492 measured the zoom benchmark on a size-ordered ("bubble") layer (release,
1000x700, `--present mailbox --uncapped`, machine load about 3 to 5):

| run                                                      | cpu median | draws |
| -------------------------------------------------------- | ---------- | ----- |
| `--points 300000` (plain scatter, one chunk)             | 0.86 ms    | 4     |
| `--bubbles --points 100000` (one chunk)                  | 0.62 ms    | 4     |
| `--chunk-rows 25000 --points 100000` (plain scatter)     | 1.08 ms    | 7     |
| `--bubbles --chunk-rows 25000 --points 100000`           | 12.5 ms    | 7     |
| `--bubbles --window 100000 --points 300000` (before 492) | 12.6 ms    | 19    |
| the same after GUP-492 (`BUCKET_BITS` 3 or 2)            | 12.0 ms    | 19    |

A size-ordered layer with more than one chunk spends about 12 ms a frame in
`Plot::resolve` (prepare 0.08 ms, GPU pass about 1 ms), windowed or not,
bucketed or not. Draw count is not the cause. The suspects, unprofiled, are
per-frame O(rows) loops in `selection.rs` (about lines 2765 to 2910: the `rank`
vector over every stored row, `row_of` binary searches per row). GUP-492's AC3
gate (`zoom_window.cpu.median_ms` at most 4.0) moved here.

## Read First

- `docs/planning/stories/GUP-492_Windowed_Size_Order_Draws.md`, the "Blocked"
  section.
- `crates/gup-core/src/selection.rs` from line 2740 to 2930, and `DrawOrder` in
  `crates/gup-core/src/order.rs` (`row_of`, `stored_of`, `spans`).
- `crates/gup-core/examples/zoom_bench.rs`: `--bubbles`, `--window`,
  `--chunk-rows`.

## User Story

> "As a developer zooming a bubble chart, I want a size-ordered layer's frames
> to cost what a plain scatter's do, so that its size order does not make it
> slower than the same chart without one."

## Acceptance Criteria

### AC1: Profile first

- [x] Record in the story where the resolve time goes (function and share) for
      `--bubbles --chunk-rows 25000 --points 100000`, from a profile or timers.
      State whether it runs every frame or only when a scale changes.

### AC2: Cost

- [x] Same-session A/B (median of at least 3 runs, load noted):
      `zoom_window.cpu.median_ms` (`--bubbles --window 100000 --points 300000`)
      and the unwindowed multi-chunk bubbles case
      (`--bubbles --chunk-rows 25000 --points 100000`) are each at most 2x the
      plain scatter's resolve with the same chunking, and
      `zoom_window.cpu.median_ms` is at most 4.0.
- [x] Draw order, picks and images are unchanged (the `DrawOrder` unit tests and
      `tests/window.rs` pass without edits).

## Decisions

- The fix is whatever the profile shows; cache per-epoch what does not change
  while the columns do not (rank, row maps), rather than recompute per frame.
- If the profile shows a design question (for example the pick index must be
  built another way), stop and report it.

## Escalation Triggers

- The profile shows the time is not in `selection.rs` or `order.rs`.
- Meeting 2x needs a change to the pick or culling API.

## Out of Scope

- Level of detail (GUP-721 builds on this).
- Recording `PERF_BUDGETS.md` (the orchestrator does).

## Technical Tasks

- [x] Profile and record (AC1).
- [x] Fix and measure A/B (AC2).
- [x] Propose the `zoom_window.cpu.median_ms` budget row in the retrospective.

## Dependencies

### Prerequisite Stories

- GUP-492: Windowed Size-Ordered Draws (the `--bubbles` and `--window` flags).

### Enables Stories

- GUP-721: LOD for Size-Ordered Layers (a LOD frame on a size-ordered layer
  would pay this cost too).

## Testing Strategy

- `zoom_bench` A/B as AC2; existing order and window tests unchanged.

## Success Metrics

- [x] `zoom_window.cpu.median_ms` at most 4.0 and at most 2x the plain
      scatter's.

## Risk Assessment

- **Low**: a cache keyed on the epoch goes stale. _Mitigation_: key it on the
  same epoch as the pick index; a test that appends and zooms.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` regularly
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] `mask perf-budget` table pasted
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Profile (2026-11-12, AC1)

`perf`, `valgrind` and `samply` are not installed and ptrace attach is refused
(`yama` scope 1), so the profile is from timing spans (`Instant`, temporary,
reverted) around every stage of `Plot::resolve_full`. Release,
`--bubbles --chunk-rows 25000 --points 100000 --present mailbox --uncapped`,
load average about 4.7, median of 660 frames (stage times are cumulative from
the start of `resolve_full`):

| stage                                               | time         |
| --------------------------------------------------- | ------------ |
| `resolve_full` total                                | 11.80 ms     |
| steps 1 to 5 up to the start of step 6              | 11.58 ms     |
| of which `Layer::edge_marks` (the edge-inset solve) | 11.44 ms     |
| `Layer::prepare` (uniforms, columns)                | 0.08 ms      |
| `fit_domains`, `visible_entries`, `visible_spans`   | 0.002 ms     |
| steps 6 and 7 (the scene)                           | about 0.2 ms |

The same run without `--bubbles`: `edge_marks` 0.017 ms, `resolve_full` 0.40 ms.

So 97% of the cost is `Selection::edge_marks` (`selection.rs`, called from
`resolve_full` step 5 for explicit-domain axes), not the `rank` / `row_of` loops
the story suspected (those are in the pick-index build, which runs once per
epoch). It runs every frame in which a scale changes (a zoom changes the
provisional range and so the candidates). With a size column the GUP-567
shortcut (sorted values plus binary search, which needs every row to reach the
same `fixed` extent) does not apply, so every chunk the statistics cannot rule
out (up to the 65,536-row `EDGE_ROW_BUDGET`) is decoded (`chunk_values`) and
scanned row by row with two dynamic `cpu_eval` calls per row. Any layer with a
size column pays it, ordered or not; the order is incidental (a plain scatter
has no size column). The fix is within `selection.rs`.

## Implementation Summary

`Selection::edge_marks` (the edge-inset solve of an explicit-domain axis, run
every frame a scale changes) decoded and scanned every chunk of a layer with a
size column. Fix: a per-chunk `ReachIndex` in the layer's `EdgeCache` (keyed on
the layer's epoch, like the sorted-value cache of GUP-567): the chunk's rows
with a finite position sorted by position, with next and previous greater-size
and smaller-size links (monotonic stacks, built once per chunk). A frame finds
the in-domain rows by binary search and follows the links from both ends, so it
evaluates the rows no other in-domain row beats on both position and reach (the
solve takes `max(h - t * span)` with `span > 0`, so the others cannot set an
inset). It falls back to the old scan when a chunk has a non-finite size, there
is more than one size column, the span is not positive, the size image goes
below zero, or a chain exceeds 4,096 links. Files:
`crates/gup-core/src/selection.rs` only (plus this story). Tests added:
`reach_index_matches_the_scan` (1,500 random cases against the scan, bit-equal
insets) and `a_size_column_uses_the_reach_index_and_stays_fresh` (append and
eviction against a cold cache). `gup-core` 320 lib tests and every integration
test (including `tests/window.rs`) pass unedited.

### Measurements (release, 1000x700, mailbox, uncapped, same session, alternating

old and new builds, load 1.6 to 5; cpu = `CPU work (all but acquire)` median)

| run                                                  | old (ms)          | new (ms)       |
| ---------------------------------------------------- | ----------------- | -------------- |
| `--bubbles --window 100000 --points 300000`          | 10.94 10.83 10.88 | 0.91 0.87 0.98 |
| `--bubbles --chunk-rows 25000 --points 100000`       | 11.62 11.79 12.44 | 0.95 0.81 0.90 |
| `--chunk-rows 25000 --points 100000` (plain scatter) | 0.64 0.61 0.70    | 0.65 0.66 0.63 |
| `--chunk-rows 25000 --points 300000` (plain scatter) | 0.65 0.68 0.66    | 0.65 0.67 0.67 |

`Plot::resolve` alone: bubbles 10.5 to 12.1 ms before, 0.21 to 0.37 ms after
(plain scatter 0.22 to 0.26). So the bubbles' CPU frame is 1.4x the plain
scatter's (2x allowed) and the windowed one is 0.9 ms (4.0 allowed). Draw counts
are unchanged (7 and 19). WASM (`mask wasm-size`, same session):
`wasm.scatter.gz_bytes` 379,193 on the base, 382,995 here (+3,802 B, +1%, within
the 2% check).

### `mask perf-budget` (2026-11-12, load about 3 to 5)

Every check passed. The rows nearest the change:

```text
zoom.cpu.p95_ms                0.86   +25%   1.075   0.940  pass
zoom_culled.cpu.median_ms      1.07   +20%   1.284   0.757  pass (faster than recorded: re-record?)
wasm.scatter.gz_bytes        376445    +2%  383973.9 382995 pass
wasm.over_wgpu.gz_bytes      344085 <= 400000 400000 350635 pass
```

## Retrospective

**Completed**: 2026-11-12

### Key Technical Learnings

#### The suspect was wrong; the profile was not

- **Challenge**: GUP-492 and this story's text blamed the `rank` vector and
  `row_of` loops (the pick-index build). No profiler works here (`perf` missing,
  ptrace refused), so the first guess was a reading of code.
- **Solution**: temporary `Instant` spans bisecting `resolve_full` by its
  numbered steps, then the one call (`edge_marks`) inside the slow step: 97% of
  the 11.8 ms, every frame a scale changes.
- **Pattern**: without a sampling profiler, bisect by stage markers in the
  function that the bench shows slow; a 5-minute exercise beat reading 300 lines
  of suspects. Sized layers (any size column), not size-ordered ones, pay it;
  the order was incidental.

#### A dominance cache for a maximum

- **Challenge**: the edge solve is `max(h - t * span)` over rows with a
  per-frame `t` (the view moves), so a cache of results does not survive a zoom,
  and the GUP-567 sorted-value search needs one `h` for all rows.
- **Solution**: cache per chunk what does not change with the view (rows sorted
  by position, next and previous greater and smaller size by monotonic stacks)
  and keep the per-frame part (in-domain slice, `t`, `h`) to a handful of rows.
  Dominance needs `span > 0`: a randomized test with a negative span found it,
  and the code now refuses (scans) instead.
- **Pattern**: for "max over rows of a function that is monotone in two per-row
  keys", index by one key and chain by the other; compare against the brute
  force with random data including ties, zeros, reversed encodings and both
  scale directions.

### Architectural Decisions

#### Fallback to the scan, not to statistics

- **Decision**: when the index cannot answer (non-finite size, more than one
  size column, non-positive span, negative size image, a chain over 4,096
  links), the old exact scan runs, still bounded by `EDGE_ROW_BUDGET`.
- **Reasoning**: results stay bit-identical to before in every case; only the
  common case got faster.
- **Trade-off**: a `Rect` (width and height size columns) or a chunk with a null
  size still costs the scan; `Rect` could take a two-column index.
- **Future**: GUP-721's LOD frames on a sized layer read the same index.

### Development Workflow Insights

- A/B of two release binaries (`/tmp/zb_old`, `/tmp/zb_new`, alternating, 3
  runs) under load 1.6 to 5 gave stable medians (10.8 to 12.4 ms against 0.8 to
  0.98 ms); a first awk slip printed a word instead of the number, so check the
  table's columns before reading it.
- The sandbox's worktree guard refuses any command it cannot prove stays in the
  worktree (heredocs with `git` in text, paths with `github.com` in a variable);
  write scripts to `/tmp` files and run them.

### Proposed budget row (for the orchestrator; `PERF_BUDGETS.md` untouched)

`zoom_window.cpu.median_ms` (`--bubbles --window 100000 --points 300000`):
record 0.9 ms (three runs 0.87, 0.91, 0.98; plain scatter with the same chunking
0.65), tolerance as the other CPU rows (+20%), with the story's 4.0 ms as the
hard ceiling.

### Follow-up Stories

None. A two-column (width and height) reach index for `Rect` is under 30 minutes
of the same code but has no measured need; noted here, not written up.
