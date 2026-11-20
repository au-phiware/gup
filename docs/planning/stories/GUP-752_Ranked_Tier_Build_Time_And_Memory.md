---
implementer: sonnet
implementer-reason:
  profiling and tightening one build function against measured numbers; the
  layout is settled
touches: [crates/gup-core/src/lod.rs, crates/gup-core/examples/scale_bench.rs]
---

# GUP-752: Ranked Tier Build Time And Memory

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-20

## Context

GUP-749's ranked build (a layer drawn largest first) regroups the sorted pairs
by size bucket, scans each bucket, and stores a copy of each stored tier in rank
order. `scale_bench --lod --sized` (10M rows, 16 random levels, one thread,
release) measured 1,090 ms against 598 ms for the unranked build, both at a peak
of 16 B a row. The copies hold a third more representatives than the finest
tier, on the CPU and the GPU. Where the extra time goes (the rank closure per
row, the binary search per row, the scrambled comparisons, the copy filter over
every stored tier) is not measured.

## User Story

> "As a developer of a large bubble chart, I want its tiers to build as fast as
> a scatter's, so that its first frame is not slower."

## Acceptance Criteria

### AC1: Measured and reduced

- [ ] A profile of the ranked build's phases at 10M rows, recorded in the story.
- [ ] The ranked build within 1.3x the unranked one's time at the same peak of
      16 B a row, or the table of what was tried.
- [ ] Representatives on the GPU for a ranked chunk measured (bytes per row) and
      reported next to the unranked chunk's.

## Decisions

- The layout (a copy of each stored tier) stays unless the measurement shows a
  cheaper one with the same draw calls.

## Escalation Triggers

- Reducing the time needs a change of the representatives drawn.

## Out of Scope

- The rule of which rows represent (GUP-751).

## Dependencies

### Prerequisite Stories

- GUP-749 ✅

## Testing Strategy

- `lod::tests` (brute force) unchanged; `scale_bench --lod --sized` before and
  after.

## Success Metrics

- [ ] Ranked build within 1.3x the unranked one.

## Risk Assessment

- **Low**: a build-only change, covered by the brute-force tests.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` and `mask all-check` pass
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added
