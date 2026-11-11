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

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-11

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

- [ ] Record in the story where the resolve time goes (function and share) for
      `--bubbles --chunk-rows 25000 --points 100000`, from a profile or timers.
      State whether it runs every frame or only when a scale changes.

### AC2: Cost

- [ ] Same-session A/B (median of at least 3 runs, load noted):
      `zoom_window.cpu.median_ms` (`--bubbles --window 100000 --points 300000`)
      and the unwindowed multi-chunk bubbles case
      (`--bubbles --chunk-rows 25000 --points 100000`) are each at most 2x the
      plain scatter's resolve with the same chunking, and
      `zoom_window.cpu.median_ms` is at most 4.0.
- [ ] Draw order, picks and images are unchanged (the `DrawOrder` unit tests and
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

- [ ] Profile and record (AC1).
- [ ] Fix and measure A/B (AC2).
- [ ] Propose the `zoom_window.cpu.median_ms` budget row in the retrospective.

## Dependencies

### Prerequisite Stories

- GUP-492: Windowed Size-Ordered Draws (the `--bubbles` and `--window` flags).

### Enables Stories

- GUP-721: LOD for Size-Ordered Layers (a LOD frame on a size-ordered layer
  would pay this cost too).

## Testing Strategy

- `zoom_bench` A/B as AC2; existing order and window tests unchanged.

## Success Metrics

- [ ] `zoom_window.cpu.median_ms` at most 4.0 and at most 2x the plain
      scatter's.

## Risk Assessment

- **Low**: a cache keyed on the epoch goes stale. _Mitigation_: key it on the
  same epoch as the pick index; a test that appends and zooms.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` regularly
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask perf-budget` table pasted
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

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
