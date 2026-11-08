---
implementer: opus
implementer-reason:
  changes how the plot answers pick_row, the marquee brush and keyboard focus
  for layers too large for a CPU pick index; design work across plot.rs, the
  pick index and the GPU pick
touches:
  [
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/pick.rs,
    crates/gup-core/src/gpu_pick.rs,
    crates/gup-core/src/selection.rs,
  ]
---

# GUP-667: Interaction Above the CPU Pick Bound

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-08

## Context

The plot answers `pick_row`, `pick_anchor`, the marquee brush (`rows_in`) and
keyboard focus (`focus_order`) from a CPU pick index of every visible row, built
once per resolve. A layer whose CPU data `Retain` dropped contributes no items;
hover and click then go through the GPU pick (`gpu_mode`).

GUP-664 made a columnar layer readable whatever its policy, but kept that bound:
`hit_items` indexes a released columnar layer only while its visible (culled)
rows are at most `Retain::AUTO_MAX_ROWS`. Indexing every row of a 100M-row layer
would cost gigabytes and seconds, and in `gpu_mode` the plot builds the index
anyway on the first hover, for the tooltip anchor (`set_hover`, `sync_overlay`).

So above the bound, `pick_row` returns `None`, the marquee brush selects nothing
and the arrow keys skip the layer. GUP-632 (100M rows with zoom, pan and keys)
needs them.

## User Story

> "As a developer showing 100M points, I want `pick_row`, the brush and the
> arrow keys to work as they do at 1M, so that a large chart is not a lesser
> chart."

## Acceptance Criteria

- [ ] `pick_row` answers for a layer above the bound (a test with a lowered
      bound or a seam), with the row the GPU pick gives, and costs no memory
      proportional to the layer's rows.
- [ ] The tooltip anchor of a GPU-picked row comes from that row alone (its
      position read from the store), never from building the full index.
- [ ] The marquee brush selects the rows of a large layer inside the rectangle
      (GPU-side or a bounded streaming read of the columns), measured at 10M and
      30M rows.
- [ ] Keyboard focus visits a large layer's rows in a defined order, or the
      story records why it cannot and what it does instead.
- [ ] Layers below the bound pick, brush and focus exactly as today (the
      existing pick and brush tests pass unchanged).

## Technical Tasks

- [ ] A per-row anchor: `Layer::anchor_of(row)` reading one row through
      `ColumnStore::value` (GUP-664) and the encodings' CPU mirrors.
- [ ] `pick_row` falls back to a GPU pick (or a streaming CPU read) for layers
      left out of the index.
- [ ] `rows_in` for large layers.
- [ ] A focus order that does not need every row's position.

## Dependencies

### Prerequisite Stories

- GUP-664: Shared Columns as the CPU Copy ✅

### Enables Stories

- GUP-632: Hundred Million Row Demonstration (keys and brush at 100M).

## Testing Strategy

- **Integration**: picks, anchors and brushes on a layer above a lowered bound,
  compared with the same layer below it.
- **Performance**: first hover, brush and pick latency and peak RSS at 10M and
  30M rows (`scale_bench` or a new phase).

## Success Metrics

- [ ] First hover on a 30M-row columnar scatter adds under 20 MiB and 50 ms.

## Risk Assessment

- **Medium**: `pick_row` has no `Context`; a GPU fallback needs one, or a
  streaming read that is slow at 100M. _Mitigation_: decide which in the first
  hour from a measurement, and escalate if both are unacceptable.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
