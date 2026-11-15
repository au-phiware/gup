---
implementer: opus
implementer-reason:
  threads in the prepare path of a released column store, with a CPU-copy
  lifetime that release currently ends synchronously
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/plot.rs,
  ]
---

# GUP-730: Tier Builds Off the Frame Path

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-12

## Context

GUP-716 builds a chunk's level-of-detail tiers (GUP-714, about 50 ns a row on
one thread) and gathers its representatives (GUP-715) inside
`Selection::prepare`. Two cases stall a frame:

- **An owned layer above 10M rows** (`Retain::Auto`) drops its full chunks' CPU
  copies after its first prepare, so that prepare builds every full chunk first:
  +1,021 ms on an 11M-row first resolve (GUP-716 measurements; 255 ms without
  LOD). At 100M that is about 9 s, which leaves nothing of GUP-632's 10 s
  first-frame budget.
- **An interactive host** (a waker set) builds one missing chunk a frame, so
  each of a 10M layer's first ten frames takes about 90-130 ms more (one 1M-row
  chunk's build and gather).

The build is a pure function of a chunk's rows (GUP-714), so chunks build
independently and in parallel; on native targets they can build on worker
threads while frames keep drawing the chunk in full (RFC-001: "a chunk whose
tiers are not built draws in full").

## User Story

> "As a developer of a 100M-row scatter, I want the first frame to show my data
> as fast as it would without level of detail, and zooming to stay smooth while
> the tiers build."

## Acceptance Criteria

- [ ] On native targets, missing chunks' tiers and gathers build on worker
      threads (a bounded pool, `std::thread::available_parallelism`); a prepare
      never waits for a build, and a chunk draws in full until its result is
      installed at a later prepare (which asks for a redraw while builds run).
- [ ] An owned layer above `Retain::AUTO_MAX_ROWS`: the first resolve of 11M
      rows costs at most 10% more than with `Lod::Off` (GUP-716: +400%), and
      every full chunk's tiers exist before its CPU copy is dropped (the copy is
      released when its build finishes, not in the first prepare).
- [ ] Headless renders keep GUP-716's behaviour: a plot without a waker builds
      every missing chunk before its one frame (in parallel).
- [ ] WASM (no threads) keeps GUP-716's one-chunk-a-frame build;
      `mask     wasm-size` unchanged within 1%.
- [ ] Every frame of GUP-719's append loop (a 2,000,000-row window, 100,000 rows
      a frame, release) is at most 8 ms at p95, building frames included:
      GUP-719 measured 8-54 ms for the frames that rebuild the newest chunk's
      tiers (about 0.1 s per million rows) and asserts the bound only on frames
      that build nothing; `a_rolling_window_frame_is_cheap` then asserts all
      frames.
- [ ] `tests/lod.rs` gains a test that a hosted plot's frames never block on a
      build (a build seam that sleeps) and that the image converges to the
      headless one.

## Decisions

- Native only; `wasm32` keeps the synchronous path.
- The CPU copy of a chunk being built is held (an `Arc` of its bytes or its
  shared columns) until the build finishes; peak memory is reported.

## Escalation Triggers

- Holding CPU copies past the first prepare raises peak RSS of an owned 100M
  layer by more than 25%: report the figures.
- `ColumnStore` cannot hand a chunk's cells to a thread without copying them.

## Out of Scope

- A GPU build (GUP-714's note); appends (GUP-719).

## Technical Tasks

- [ ] A build pool and per-chunk build state in the column store.
- [ ] Release that waits per chunk for its build.
- [ ] Tests and measurements (`scale_bench --lod-draw auto`, `lod_spike`).

## Dependencies

### Prerequisite Stories

- GUP-716: `Selection::lod` and Per-Frame Tier Draws.

### Enables Stories

- GUP-632: Hundred Million Row Demonstration (its 10 s first frame).
- GUP-719: LOD With Appends and Windows (its every-frame 8 ms bound).

## Testing Strategy

- **Integration tests**: `tests/lod.rs` (hosted frames never wait; headless
  completes in one frame).
- **Performance**:
  `scale_bench --rows 11000000 --resolve-only --lod-draw off|auto`, three runs
  each, alternating.

## Success Metrics

- [ ] First resolve of an owned 11M-row layer within 10% of `Lod::Off`.

## Risk Assessment

- **Medium**: threads and the release policy interact (a chunk released while
  its build reads it). _Mitigation_: the build holds its source.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
