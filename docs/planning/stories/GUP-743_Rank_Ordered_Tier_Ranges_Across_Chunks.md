---
implementer: sonnet
implementer-reason:
  extends GUP-721's range ordering with a measured fidelity target; the rule is
  settled (global rank order)
touches:
  [
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/order.rs,
    crates/gup-core/tests/lod.rs,
  ]
---

# GUP-743: Rank-Ordered Tier Ranges Across Chunks

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-19

## Context

GUP-721 orders each tier block by size rank, and a chunk drawing a tier draws
all its ranges where its first segment of the layer's size order is. The layer's
exact order interleaves ranks across chunks (batches appended to a size-ordered
layer, windowed layers whose chunks each sort on their own), and a finer block's
large marks draw over a coarser block's small ones. An appended bubble layer
differs from the exact draw by 3.26% of the plot rect against 2.93% for the same
scene built at once; a 1M-row window 2.44%.

## User Story

> "As a developer streaming bubbles, I want the thinned draw to layer marks as
> the full draw does, so that a large mark never covers a small one."

## Acceptance Criteria

### AC1: Ranges in global rank order

- [ ] `Run` carries its rank; a frame's ranges over all visible chunks are
      issued in rank order (stable by chunk and position), merging adjacent
      ranges of one chunk and rank, within the existing draw count bounds
      (`MAX_RANGES` a chunk).
- [ ] A measured table of draw calls and fidelity (appended and windowed bubble
      tests of GUP-721) against GUP-721's numbers; fidelity no worse, draw calls
      at most 2x.
- [ ] Layers in data order draw exactly as before (byte-identical images).

## Decisions

- Decide on the measurement: if draw calls grow past 2x, keep the per-block
  order and record the numbers.

## Escalation Triggers

- Rank-order issue needs one instanced draw per rank and chunk beyond the budget
  in `PERF_BUDGETS.md`: stop and report.

## Out of Scope

- Mixed-size cells (GUP-742).

## Dependencies

### Prerequisite Stories

- GUP-721 ✅

## Testing Strategy

- The appended and windowed bubble image tests; a draw-order unit test.

## Success Metrics

- [ ] Appended bubbles within 1.1x of the same scene built at once.

## Risk Assessment

- **Medium**: more draw calls per chunk. _Mitigation_: the measured bound.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` and `mask all-check` pass
- [ ] Rendered output read by eye
- [ ] WASM size reported
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added
