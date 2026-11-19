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
