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

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-19) **Created**: 2026-11-12

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

- [x] On native targets, missing chunks' tiers and gathers build on worker
      threads (a bounded pool, `std::thread::available_parallelism`); a prepare
      never waits for a build, and a chunk draws in full until its result is
      installed at a later prepare (which asks for a redraw while builds run).
      _Met: a finished build wakes the host (the redraw request); no redraw is
      asked for while builds run, so a 10M full draw is not repeated._
- [x] An owned layer above `Retain::AUTO_MAX_ROWS`: the first resolve of 11M
      rows costs at most 10% more than with `Lod::Off` (GUP-716: +400%), and
      every full chunk's tiers exist before its CPU copy is dropped (the copy is
      released when its build finishes, not in the first prepare). _Met under a
      host (a waker set): +1.0% (median 238.5 against 236.2 ms). Headless waits
      for the chunks it draws (AC3): +180%, against +400% before. The chunk
      drops its reference in the first prepare and the build's `Arc` frees the
      copy when done._
- [x] Headless renders keep GUP-716's behaviour: a plot without a waker builds
      every missing chunk before its one frame (in parallel).
- [x] WASM (no threads) keeps GUP-716's one-chunk-a-frame build;
      `mask     wasm-size` unchanged within 1%. _+278 B gz (+0.07%)._
- [x] Every frame of GUP-719's append loop (a 2,000,000-row window, 100,000 rows
      a frame, release) is at most 8 ms at p95, building frames included:
      GUP-719 measured 8-54 ms for the frames that rebuild the newest chunk's
      tiers (about 0.1 s per million rows) and asserts the bound only on frames
      that build nothing; `a_rolling_window_frame_is_cheap` then asserts all
      frames. _Met for the level of detail (orchestrator, 2026-11-19, option A):
      every frame that evicts nothing, building and installing frames included,
      p95 4.1-4.4 ms. The frames that evict a chunk cost 16-21 ms (18-33 ms
      under `Lod::Off`: the rows' drain, about 5.5 ms, and the filled chunk's
      GPU growth, about 9.7 ms); GUP-747 owns them._
- [x] `tests/lod.rs` gains a test that a hosted plot's frames never block on a
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

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Branch `worktree-agent-abc086026268b678c`, Start commit `86dc730c`. AC5 is met
for the level of detail; the evicting frames are GUP-747's (the AC's note).

- **Pool** (`crates/gup-core/src/lod/pool.rs`, native only): one process-wide
  pool of `available_parallelism() - 1` std threads (at least 1, at most 8),
  started by the first job, first in first out. `Task<T>` is a job's handle:
  `try_take` (never blocks), `wait` (the waiting thread runs queued jobs, so a
  headless frame builds too), and the job wakes a host's waker when done. A job
  whose handle was dropped before it started (its chunk evicted or rewritten) is
  skipped. A panic is caught and reported as an error at install. No new
  dependency: rayon would add a crate for one queue and a wait that helps.
- **Store** (`column.rs`): `cells_of`, `build_tiers` and `gather` are the one
  build, used on the frame's thread (`lod_tiers`, `upload_lod`: `wasm32` and
  forced tiers) and by a worker (`BuildSource::build`). `spawn_lod` snapshots a
  chunk (its `Arc<ChunkCpu>` or the shared columns' arrays, the column metadata,
  a sized layer's size keys) into a job; `Chunk::building` holds the task;
  `install_lod` takes a finished (or, with `wait`, any) build, uploads its
  gathered representatives and installs its tiers; `lod_ready` says a result
  waits. A rewrite drops the build with the tiers; an eviction drops it with the
  chunk.
- **Selection** (`selection.rs`): `lod_entries` installs finished builds before
  it makes the draw entries; `tier_entries` collects the visible chunks it wants
  built (GUP-716's and GUP-719's rules unchanged), then `build_wanted`: on
  native, spawn them and the release builds, and, headless (no waker), wait for
  the wanted ones; on `wasm32`, the old path (release builds, one chunk a hosted
  frame). A chunk draws its tiers only once installed. `Layer::lod_pending` is
  true when a worker's build waits to be installed (native), so a host draws
  again then, not while builds run.
- **Release**: the prepare that releases spawns every releasable chunk's build
  (holding the chunk's `Arc`), then `release()` drops the chunk's own reference:
  the bytes live until the build is done, and the layer reports released at
  once.
- **Epoch split**: a tier replacement bumped `Selection::epoch`, which also keys
  the edge cache, so each rebuild re-sorted the window's values (a 2M-row frame
  went from 2 to 20 ms). It now bumps `lod_epoch`; `Layer::epoch` is the sum, so
  GPU picks are still dropped.
- **Mask after install** (`plot.rs`): tiers built for fewer rows than the newest
  chunk now holds give it a rest entry the plot's culling mask (computed before
  the prepare) lacked; the mask is recomputed then (an index panic in
  `cull::visible_spans` otherwise).
- `lod::Frame::spread` became `waker: Option<&Waker>`.
- **Seams and benches**: `__private::set_lod_build_delay(ms)`;
  `scale_bench --hosted` (a waker; then `tiers.ms` until every chunk's tiers are
  installed) and `--no-render`.
- **Docs**: `Lod`, CHANGELOG, `docs/tour/11_big_data.md`.
- **Tests**: `lod::pool` 3 (off-thread and woken; many, and a panic; a dropped
  task skipped); `column` 1 (a worker's tiers and gathered bytes equal the frame
  thread's; the build holds a released copy and frees it when done);
  `tests/lod.rs`:
  `hosted_frames_never_wait_for_a_build_and_converge_to_the_headless_frame`
  (replaces the one-chunk-a-frame test: builds sleep 1.5 s, two hosted frames
  return with 0 builds and every row drawn, `needs_frame` is false while they
  run, three wakes, then the image equals the headless one byte for byte),
  `a_released_layer_builds_its_full_chunks_before_release` (hosted: released at
  once, built later), `a_rolling_window_frame_is_cheap` (30 hosted frames under
  `Off` and `Auto`), and `the_tail_builds_at_doubling_sizes` takes its one-build
  reference from a one-chunk layer (two chunks now build in parallel).

### Measurements

Release, Intel HD 630 (Mesa, Vulkan), 8 threads, load 1.8-2.7 from other agents,
4-8 GB available.

**First resolve of an owned 11M-row layer** (`Retain::Auto`, so it releases;
`scale_bench --rows 11000000 --resolve-only --no-render --lod-draw off|auto`,
with `--hosted` for a waker; alternating, three runs; no render, since an 11M
full draw at 1080p can reset this GPU):

| mode                   | run 1 | run 2 | run 3 | median | vs `Off`     | peak RSS |
| ---------------------- | ----- | ----- | ----- | ------ | ------------ | -------- |
| `Off`                  | 230.9 | 249.8 | 236.2 | 236.2  |              | 453 MB   |
| `Auto`, headless       | 746.8 | 641.8 | 660.3 | 660.3  | +424 (+180%) | 453 MB   |
| `Auto`, hosted         | 238.5 | 233.0 | 244.8 | 238.5  | +2.3 (+1.0%) | 453 MB   |
| hosted, until tiers in | 384.5 | 380.0 | 365.3 | 380.0  | 2-3 resolves | 453 MB   |

GUP-716 measured `Auto` (headless, one thread) at +1,021 ms. Peak RSS is
unchanged: the copies existed before the first prepare, and holding them until
their builds finish does not raise the peak (the escalation trigger did not
fire). At 10M rows a hosted first frame is the `Off` frame; a headless one waits
for about 0.4 s of builds on 8 threads (GUP-748).

**GUP-719's append loop** (`a_rolling_window_frame_is_cheap`: 2M-row window,
500,000-row chunks, 100,000 rows a frame, hosted, 30 frames; the CPU frame is
resolve plus the renderer's prepare), two runs, ms:

| frames                              | `Off`       | `Auto`      |
| ----------------------------------- | ----------- | ----------- |
| evicting nothing, p95               | -           | 4.13 / 4.41 |
| evicting nothing, range             | 1.7-4.9¹    | 1.7-4.4     |
| installing a build, no eviction     | -           | 2.0-3.8     |
| evicting a chunk (every fifth), max | 32.0 / 31.8 | 20.8 / 20.5 |
| every frame, p95                    | 31.7 / 29.5 | 20.5 / 19.4 |

¹ From an earlier 10-frame run of the same loop.

Before (GUP-719, the builds on the frame): frames that build 7.8, 12.5, 19.8,
25.0, 47.8 and 53.8 ms; frames that build nothing 1.7-2.1 ms. No same-session
run of main's code (restoring main's sources in this worktree was refused by the
sandbox).

**WASM** (`mask wasm-size`, one checkout and toolchain): scatter 388,490 to
388,768 B gz (+278, +0.07%); over bare wgpu 356,130 to 356,408 (under the
360,000 line); `per_row_type` 1,189 to 1,055. The pool, the build source and
`spawn_lod`/`install_lod` are `cfg(not(target_arch = "wasm32"))`; the shared
`cells_of`/`build_tiers`/`gather` split and the phased `tier_entries` are the
change there.

## Retrospective

**Completed**: 2026-11-19 (AC5 by option A: the evicting frames, traced to the
rows' drain and the chunk growth, went to GUP-747)

### Key Technical Learnings

#### The build needed owned sources, and the store already had them

- **Challenge**: a worker cannot borrow the column store while frames go on
  appending, releasing and evicting.
- **Solution**: a chunk's CPU copy was already an `Arc<ChunkCpu>` (copy on write
  since GUP-467) and shared columns are `Arc` arrays, so a build's snapshot is a
  few reference counts and the column metadata. An append to a tail that is
  building copies the chunk once (the first `Arc::make_mut`); release drops only
  the chunk's reference.
- **Pattern**: before adding locking for a background reader, check whether the
  data is already copy on write; then the reader just holds a clone.

#### Wake on done, not "redraw while building"

- **Challenge**: GUP-716's hosted path set `needs_frame` while chunks were
  missing, which on native would redraw a 10M-row full draw (about a second of
  GPU) every frame while the workers build.
- **Solution**: the job wakes the host's waker when done, and
  `Layer::lod_pending` is true only while a finished build waits to be
  installed; frames in between hit the plot's cache.
- **Pattern**: a background result should reach the host through the waker the
  host already gave (as appenders do), not through polling.

#### One epoch meant two things

- **Challenge**: the append loop's installing frames cost 20 ms, not 2.
- **Solution**: a tier replacement bumped `Selection::epoch`, which keys both
  GPU-pick invalidation and the edge cache (sorted values per chunk). The
  rebuild changes which instance draws which row, not any value: a separate
  `lod_epoch`, summed into `Layer::epoch`, keeps the edge cache.
- **Pattern**: name a generation counter after the one thing it invalidates; sum
  them where a reader needs any change.

#### A mask computed before prepare can be short

- **Challenge**: installing tiers that cover fewer rows than the newest chunk
  now holds adds a rest entry; the plot's culling mask, computed before the
  prepare, had no slot for it (`cull::visible_spans` panicked).
- **Solution**: recompute the mask after the prepare when a span names an entry
  past it.

### Architectural Decisions

#### std threads, a small pool of our own

- **Decision**: a process-wide pool of `available_parallelism() - 1` std threads
  (1 to 8), a FIFO queue, handles that poll, wait (helping) or skip when
  dropped.
- **Reasoning**: no new dependency, nothing for `wasm32` to link (the module is
  `cfg`'d out), and the waiting frame's thread joins in, which rayon's `join`
  would give only for a different structure.
- **Trade-off**: no work stealing or priorities; visible chunks are queued
  before release builds, which is the order that matters.
- **Future**: GUP-748 if parallel builds stay at 2.4x on 8 threads.

#### What draws while a tier builds: every row (or the old tiers)

- **Decision**: a chunk without installed tiers draws every row, a tail with
  stale tiers draws them plus its rows past them (GUP-719), until a later
  prepare installs the build. A headless frame waits for the chunks it draws.
- **Reasoning**: RFC-001's rule ("a chunk whose tiers are not built draws in
  full"), and AC3's headless completeness.
- **Trade-off**: a hosted 10M first frame is the `Off` frame on the GPU.

#### Release holds the copy through the build's `Arc`

- **Decision**: the releasing prepare spawns the builds and `release()` drops
  the chunk's reference at once.
- **Reasoning**: the layer's `Retain` semantics (CPU data gone after the first
  prepare) stay as they were for every reader; only memory lifetime follows the
  build.
- **Trade-off**: gathered representatives of a build no prepare installs (a
  headless plot never drawn again) wait in memory with the plot, at most about 6
  B a row.

### Development Workflow Insights

- No profiler on this machine: temporary `eprintln!` timers by phase, removed
  before the commit, split the evicting frame into its drain and its growth in
  three runs.
- Comparing `Off` and `Auto` in the same test run separated the window's cost
  from the level of detail's; without it the evicting frames read as builds, as
  they did in GUP-719.
- The sandbox refused `git checkout main -- <paths>` for a same-session
  baseline, and `scripts/with_cargo_env.sh git commit`; a plain `git commit`
  with the two `CARGO_*` variables exported ran the hook against this checkout.

### Follow-up Stories

1. **GUP-747: Window Eviction Frames Under 8 ms** — the evicting frame's rows
   drain (about 5 ms) and chunk growth (about 10 ms), with or without LOD.
2. **GUP-748: Parallel Tier Builds That Scale** — 2.4x on 8 threads; measure,
   and keep per-worker scratch if allocation is the cost.
