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

- [x] The eviction of a whole chunk moves no held row in CPU memory: the layer's
      rows (when kept) are stored so the front can be dropped in `O(chunk)` or
      less (for example per-chunk row vectors or a ring), with every reader of
      `rows` unchanged in behaviour.
- [x] The growth cost is explained (allocation, submit, or copy) with a
      measurement, and the growth of a chunk to its full capacity costs at most
      1 ms on the frame (for example: allocate full-capacity chunk buffers for
      windowed layers once the window's chunk size is known, or grow on the GPU
      without a new allocation). _Met: neither allocation (6-26 µs) nor the
      submit's bookkeeping, but the driver's recording of one copy of a
      pathological length (1,600,004 B: 8.7-18 ms). The copy now goes in two
      pieces (about 0.1 ms) and a windowed store's later chunks never grow._
- [x] `a_rolling_window_frame_is_cheap` asserts the p95 of every frame (30
      frames, evicting frames included) under 8 ms in release, under `Off` and
      `Auto`. _Met: p95 4.3-5.4 ms (`Auto`) and 4.1-5.8 ms (`Off`) over five
      runs._
- [x] No WASM growth above 1,000 B gz for a scatter without a window. _Met: +212
      B gz (388,745 to 388,957, same session)._

## Decisions

- Keep `RowWindow`'s semantics and row ids; this is a storage change only.

### Design (recorded 2026-11-20, after the measurements below)

- **Rows: a ring (`VecDeque<T>`) in place of `Vec<T>`.** An eviction drains the
  front, which only advances the ring's head (the drained rows are dropped,
  `O(evicted)`; no held row moves). Every reader that takes one row (`get`,
  tooltips, row fields, events, `len`, `holds`) is unchanged. The three that run
  an accessor over a slice (an append's new rows, the full evaluation in
  `column_inputs`, and the pick items of a layer whose CPU columns were released
  in `hit_items`) run it over the ring's contiguous parts (`as_slices`, at most
  two) and join the columns: the same values, and only when the range wraps does
  anything differ from the slice of today (the escalation trigger does not fire:
  no row is copied). `make_contiguous` is never called (it would move the rows).
  The parts are taken through a `fn` pointer field: a direct call made
  `hit_items` differ by row type, so the linker no longer folded its copies
  (+1,455 B gz on the reference scatter; +212 B gz through the field).
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

- [x] Trace the eviction frame (rows, growth, upload) and record the split.
- [x] Row storage that drops whole chunks from the front.
- [x] Chunk growth for windowed layers.
- [x] The test's every-frame bound.

## Dependencies

### Prerequisite Stories

- GUP-730: Tier Builds Off the Frame Path.

### Enables Stories

- GUP-719's every-frame bound in full.

## Testing Strategy

- `tests/lod.rs` `a_rolling_window_frame_is_cheap` (release), under `Off` and
  `Auto`; `tests/window.rs` for row ids after eviction.

## Success Metrics

- [x] Every frame of the loop at most 8 ms at p95 in release.

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

## Implementation Summary

Branch `worktree-agent-a283c3dad1dd0b7dc`, Start commit `375e1cdc`. Every
acceptance criterion is met as written; no public API changed.

- **Rows** (`selection.rs`): `Selection::rows` is a `VecDeque<T>` (`new` takes
  the `Vec` without a copy). `evict` drains the front; `release` replaces the
  ring. `parts(rows, from)` gives a range's contiguous parts (at most two) and
  `evaluate_parts` runs a column encoding over them and joins the output
  (`ColumnData::join`); `append`, `column_inputs` and `hit_items` use them. The
  readers of one row (`get`) are unchanged. `parts` is called through a `fn`
  field so `hit_items` folds across row types again (WASM).
- **Growth** (`column.rs`): `copy_in_pieces` copies a 64 KiB multiple, then the
  rest; `Chunk::upload` uses it for every growth.
  `ColumnStore::start_chunks_full` (crate-private) makes every chunk after the
  first start at `chunk_rows`; `Selection::evaluate` sets it for a windowed
  layer.
- **Docs**: `RowWindow`, CHANGELOG (Streaming).
- **Tests**: `selection` `a_wrapped_ring_of_rows_reads_like_a_vec` (an append
  straddling the wrap, re-evaluation over a wrapped ring, tooltips and `holds`
  against the held rows); `column`
  `growth_copies_in_pieces_and_later_chunks_start_full` (an 80,004-byte growth
  copy read back from the GPU, a second chunk keeping one buffer from its first
  row to its last, unwindowed stores unchanged); `tests/lod.rs`
  `a_rolling_window_frame_is_cheap` asserts the p95 of every frame under 8 ms in
  release under `Off` and `Auto`.

### Results

`a_rolling_window_frame_is_cheap`, release, HD 630, five runs after the change
(the first before the WASM fix, which changes no behaviour):

| frames                    | before (main) | after `Auto` | after `Off` |
| ------------------------- | ------------- | ------------ | ----------- |
| evicting a chunk, max     | 21.1 / 25.9¹  | 2.9-4.8      | 3.9-8.4     |
| every frame, p95          | 19.7 / 24.8¹  | 4.3-5.4      | 4.1-5.8     |
| evicting nothing, p95 (²) | 5.1           | 4.3-5.4      | -           |

¹ `Auto` / `Off`. ² The `Auto` frames that evict nothing.

**WASM** (`mask wasm-size`, one session, main's sources restored in this
worktree for the baseline): scatter 388,745 to 388,957 B gz (+212); over bare
wgpu 356,385 to 356,597 (under the 360,000 line); `per_row_type` 1,069 to 1,197.
Before the `fn` field: 390,200 (+1,455).

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

## Retrospective

**Completed**: 2026-11-20

### Key Technical Learnings

#### The growth cost was a copy length, not an allocation

- **Challenge**: GUP-730 put the 9.7 ms on the new buffer's allocation or the
  submit, and the obvious fix (pre-size the buffer) would have hidden the cause
  for unwindowed layers.
- **Solution**: timers around `create_buffer`, encoding, an empty submit just
  before the real one, and the submit, then a standalone wgpu probe of the same
  buffer chain, then a grid of copy lengths. The cost follows the copy's length
  (400,001 words: 9-17 ms; 400,002: 0.1 ms), on every destination size.
- **Pattern**: when one step of a regular sequence is slow, reproduce the
  sequence outside the library and vary one parameter at a time; here it took
  three small probes, each under a minute.

#### A ring keeps every one-row reader and needs care only for ranges

- **Challenge**: about 17 readers of `rows: Vec<T>`.
- **Solution**: all but three read one row (`get`, `len`), which `VecDeque`
  gives unchanged; the three that run an accessor over a range run it over the
  ring's two contiguous parts and join the columns. No reader needed a single
  slice, so the escalation trigger did not fire.

#### Code folding across row types is part of the WASM budget

- **Challenge**: the ring added 1,455 B gz to the reference scatter, mostly
  three copies of `hit_items` (6.8 KB raw each) that the linker had folded into
  one: a direct call to `parts::<T>` made the copies differ.
- **Solution**: call `parts` through a `fn` field of `Selection` (the same
  offset for every `T`), so the code stays identical; +212 B gz.
- **Pattern**: in generic code that the linker folds across type parameters, any
  type-specific call breaks the fold; reach type-specific code through data (a
  `fn` field, a `dyn` already loaded) instead. `twiggy diff` with hashes
  stripped and summed per name shows it at once.

### Architectural Decisions

#### Split copies for every growth, full chunks for windows

- **Decision**: every chunk growth copies in a 64 KiB multiple plus the rest; a
  windowed store's chunks after the first start at full capacity.
- **Reasoning**: the split fixes the class (any appended layer could hit a slow
  length) for about 0.1 ms; full chunks also remove the CPU growth copy (about
  1.5 ms) on the frame that fills a chunk, which is the evicting frame.
- **Trade-off**: a windowed layer holds up to one chunk of unused capacity once
  it has filled one chunk (at most 25% of a full window, on the GPU and in CPU
  address space; the CPU pages are only touched as rows arrive).
- **Future**: the split relies on the measured driver behaviour, not on a
  documented rule; another driver could have other slow lengths, which the
  rolling-window test would show.

### Development Workflow Insights

- The sandbox refused heredocs written to `.rs` files and `env -u`; `Edit` and
  `unset` in the same command worked. `git restore --source=<commit>` of the
  gup-core sources gave a same-session baseline for WASM (GUP-730 could not get
  one with `git checkout`).
- A probe test file kept untracked and deleted before the last commit kept the
  measurements reproducible without committing a test that asserts nothing.

### Follow-up Stories

None. Unwindowed layers still copy a growing chunk on the CPU (amortised by
doubling, about 1.5 ms once per chunk size); no frame budget names them.
