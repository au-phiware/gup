# GUP-492: Bounded Draws for Windowed Size-Ordered Layers

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 💡 New **Created**: 2026-10-23

## Context

GUP-482 (RFC-001 "S12 findings") sorts a windowed size-ordered layer's batches
in runs that end at chunk boundaries, so whole chunks can leave in data order.
The draw order is still exact (largest first), but its segments cannot merge
across chunks that were sorted apart. When sizes do not follow data order (a
random size column, say), a windowed layer draws about one instanced draw per
row: a 100,000-row window would issue about 100,000 draws a frame. Compaction
cannot help (GUP-482 gave it a floor so it does not re-evaluate every append).
Unwindowed layers are unaffected: compaction re-sorts them into one run.

## User Story

> "As a developer of a live bubble chart, I want a windowed layer sized by a
> column to draw in a bounded number of draws, so that a large window stays
> interactive."

## Acceptance Criteria

- [ ] A windowed `Circle` layer of 100,000 rows with uniformly random sizes, fed
      in batches of 1,000, draws in at most `O(chunks)` draw calls a frame
      (measured), and its image equals a fresh unwindowed plot of the same rows.
- [ ] Eviction still writes no column bytes, and AC2–AC4 of GUP-482 pass.
- [ ] The chosen design and the rejected ones are recorded in RFC-001.

## Technical Tasks

- [ ] Choose between a per-layer index buffer of stored rows in draw order
      (rewritten at eviction, `O(rows)` bytes), a GPU sort (RFC-001 GUP-431
      alternative 4) and size buckets (alternative 6); measure each against the
      budget.
- [ ] Implement it for windowed layers only; keep unwindowed draws unchanged.
- [ ] Tests and a `mask perf-budget` row for the windowed bubble case.

## Dependencies

### Prerequisite Stories

- GUP-482: RFC-001 S12c `Window::LastRows` eviction ✅.

### Enables Stories

- None scheduled.

## Testing Strategy

- **Unit tests**: draw counts after appends and evictions.
- **Integration tests**: image equality with a fresh plot, as in
  `crates/gup-core/tests/window.rs`.
- **Performance**: frame time and draws for the 100,000-row window.

## Success Metrics

- [ ] Draws per frame bounded by chunks, not rows.

## Risk Assessment

- **Medium**: an index buffer brings back ordering data on the GPU, which
  GUP-431 avoided; WebGPU limits on storage buffers apply.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask perf-budget` table pasted
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added to the story document
