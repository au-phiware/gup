---
implementer: opus
implementer-reason:
  the row storage of a windowed Selection (every reader of `rows: Vec<T>`) and
  a GPU growth path whose cost is not yet explained
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/tests/lod.rs,
  ]
---

# GUP-747: Window Eviction Frames Under 8 ms

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-19

## Context

GUP-730 moved level-of-detail tier builds off the frame path. In GUP-719's
append loop (a 2,000,000-row `RowWindow::LastRows` window fed 100,000 rows a
frame, release, HD 630), the frames that evict nothing now take 1.7-4.4 ms (p95
4.1-4.4 ms over 30 frames, building and installing frames included). The frames
that evict a chunk (every fifth here: 500,000-row chunks) take 16-21 ms under
`Lod::Auto` and 18-33 ms under `Lod::Off`: the cost is not the level of detail.
GUP-730 traced one such frame:

- `Selection::evict` drops the evicted rows with `self.rows.drain(..n)`: a
  memmove of every held row (2M rows of 24 B, about 48 MB), 5.3-5.7 ms.
- `ColumnStore::upload`, growing the filled chunk's GPU buffer from 400,128 to
  500,000 rows (4,000,256 B): 9.7 ms, against 0.1 ms for the growth from 200,064
  to 400,128 the frame before. The copy is a GPU `copy_buffer_to_buffer`; the
  time is probably the new buffer's allocation (a new memory block) or the
  submit, not measured further.

GUP-730's AC5 asked every frame of the loop to be at most 8 ms at p95; these
frames are why it was met only for the frames that evict nothing (the
orchestrator's option A, 2026-11-19), and this story owns the every-frame bound.

## User Story

> "As a developer of a live scatter with a rolling window, I want the frame that
> drops the oldest rows to cost what any other frame costs."

## Acceptance Criteria

- [ ] The eviction of a whole chunk moves no held row in CPU memory: the layer's
      rows (when kept) are stored so the front can be dropped in `O(chunk)` or
      less (for example per-chunk row vectors or a ring), with every reader of
      `rows` unchanged in behaviour.
- [ ] The growth cost is explained (allocation, submit, or copy) with a
      measurement, and the growth of a chunk to its full capacity costs at most
      1 ms on the frame (for example: allocate full-capacity chunk buffers for
      windowed layers once the window's chunk size is known, or grow on the GPU
      without a new allocation).
- [ ] `a_rolling_window_frame_is_cheap` asserts the p95 of every frame (30
      frames, evicting frames included) under 8 ms in release, under `Off` and
      `Auto`.
- [ ] No WASM growth above 1,000 B gz for a scatter without a window.

## Decisions

- Keep `RowWindow`'s semantics and row ids; this is a storage change only.

### Design (recorded 2026-11-20, after the measurements below)

- **Rows: a ring (`VecDeque<T>`) in place of `Vec<T>`.** An eviction drains the
  front, which only advances the ring's head (the drained rows are dropped,
  `O(evicted)`; no held row moves). Every reader that takes one row (`get`,
  tooltips, row fields, events, `len`, `holds`) is unchanged. The three that run
  an accessor over a slice (an append's new rows, the full evaluation in
  `column_inputs`, the CPU values of a released layer's vector output) run it
  over the ring's contiguous parts (`as_slices`, at most two) and join the
  columns: the same values, and only when the range wraps does anything differ
  from the slice of today (the escalation trigger does not fire: no row is
  copied). `make_contiguous` is never called (it would move the rows).
  - Rejected: per-chunk row blocks (the blocks would have to follow the store's
    chunk boundaries before the first evaluation and after each re-evaluation,
    and a row read becomes a search); a lazy front offset into the `Vec` (keeps
    evicted rows alive and only makes the memmove rarer, so the spike remains).
- **GPU growth: split every growth copy at a 64 KiB multiple.** The 9.7 ms is
  neither the allocation nor the submit's bookkeeping but the driver's handling
  of one `copy_buffer_to_buffer` of a pathological length (measurements below):
  a copy of `big = len / 65,536 × 65,536` bytes then one of the rest costs about
  0.1 ms for every length measured. This fixes the class: every appended layer's
  chunk growth, windowed or not.
- **Windowed stores create every chunk after their first at full capacity.** A
  windowed chunk is `n / 4` rows, so once a store holds a full chunk, its next
  chunks start at `chunk_rows`: no CPU copy (`Chunk::grow`, about 1.5 ms for
  400,128 to 500,000 rows) and no GPU copy at all. The unused capacity is at
  most one chunk while the store holds at least one full chunk (at most 25% of a
  full window's columns, transiently); a windowed layer that never fills its
  first chunk keeps growing by doubling and allocates nothing more. Unwindowed
  stores keep doubling (their chunks are the device's maximum, often millions of
  rows).

## Escalation Triggers

- A reader of `rows` needs a contiguous slice that a ring cannot give without
  copying on the frame path.

## Out of Scope

- Tier builds (GUP-730).

## Technical Tasks

- [ ] Trace the eviction frame (rows, growth, upload) and record the split.
- [ ] Row storage that drops whole chunks from the front.
- [ ] Chunk growth for windowed layers.
- [ ] The test's every-frame bound.

## Dependencies

### Prerequisite Stories

- GUP-730: Tier Builds Off the Frame Path.

### Enables Stories

- GUP-719's every-frame bound in full.

## Testing Strategy

- `tests/lod.rs` `a_rolling_window_frame_is_cheap` (release), under `Off` and
  `Auto`; `tests/window.rs` for row ids after eviction.

## Success Metrics

- [ ] Every frame of the loop at most 8 ms at p95 in release.

## Risk Assessment

- **Medium**: `rows: Vec<T>` has many readers (tooltips, row fields, CPU data).
  _Mitigation_: one accessor for row `i`, tested against the current behaviour.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Measurements

Release, Intel HD 630 (Mesa, Vulkan), 2 build jobs, 2-4 GB available.

**Baseline** (`a_rolling_window_frame_is_cheap`, main's code, 2026-11-19):
evicting frames 18.0-25.9 ms under `Off`, 17.1-21.1 ms under `Auto`; p95 of
every frame 24.8 ms (`Off`) and 19.7 ms (`Auto`); frames that evict nothing
1.5-5.1 ms.

**The evicting frame, traced** (temporary timers, removed): the rows' drain
5.6-8.8 ms; `store.append` 2.4-2.9 ms against about 1.0 ms on other frames (the
CPU `Chunk::grow` of 400,128 to 500,000 rows and the new chunk); the GPU growth
400,128 to 500,000 rows: `create_buffer` 6-26 µs, encoding 10-30 µs, an empty
submit just before it 0.06-0.14 ms (so not the submit's bookkeeping or the
evicted chunk's frees), and the submit of the copy 8.7-18 ms. Each earlier
growth (64 to 100,032, to 200,064, to 400,128) submits in 0.1-0.3 ms.

**The growth copy, isolated** (a temporary wgpu probe, removed): the same chain
of buffers outside gup reproduces it (400,128 to 500,000: 8.7-10.5 ms; every
other step 0.05-0.11 ms). The cost follows the copy's length, not the buffer
sizes: copying 400,001 rows of a 4-byte column (1,600,004 B) costs 8.8-16.6 ms
into a 400,128-, 500,000-, 800,000- or 1,000,000-row buffer, and 400,000,
400,002, 400,003, 400,004 or 399,999 rows cost 0.1 ms (1.7-2.0 ms when the
source buffer is new). Slow lengths (in 4-byte words): 400,001, 400,009,
400,011, 330,001, 800,002, 1,050,304 (a 256-byte multiple), 57,143 (1.9-4.4 ms);
each has a prime factor over 16,384, which suggests the driver lays a copy out
as a 2D surface of at most 16,384 texels a side. A `write_buffer` of the same
lengths costs 0.03-0.10 ms. Split at a 64 KiB multiple plus the rest, every slow
length copies in 0.04-0.13 ms.
