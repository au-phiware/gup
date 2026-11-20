---
implementer: opus
implementer-reason:
  a representative rule for outlines that must keep tiers nested, measured
  against fidelity gates at several densities
touches: [crates/gup-core/src/lod.rs, crates/gup-core/tests/lod.rs]
---

# GUP-751: Outlines Of Dense Bubble Layers Under LOD

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-20

## Context

GUP-749 made size-ordered LOD the default: each size bucket keeps its own
representatives, marks of 4 px radius and more on cells one tier coarser, drawn
largest first. At 300,000 rows the data-following bubbles differ from the full
draw in 2.24% of a 400 x 300 plot rect (gate 3.0%), but at 3,000,000 rows in
5.19% (GUP-721's largest-mark rule: 5.16%; every bucket on the drawn tier's
cells: 4.22%). The diff images show red outlines around clumps of large marks: a
cell's representative can sit anywhere in its cell, so the union of the drawn
disks recedes from the full draw's edge by up to a cell. The denser the layer,
the more cells are full and the more of the outline is affected.

## User Story

> "As a developer of a dense bubble chart, I want the thinned draw's outlines to
> match the full draw's, so that LOD does not shrink my clusters."

## Acceptance Criteria

### AC1: A measured outline rule

- [ ] A table of at least three rules (for example: a representative chosen by
      distance from its cell's parent centre, an extra representative per
      boundary cell, a finer tier for cells at the edge of a bucket's occupied
      cells) on the data-following and random-size bubbles at 300,000 and
      3,000,000 rows: share of the plot rect over ΔE00 10 and representatives.
- [ ] Tiers stay nested (brute-force test as GUP-749's).
- [ ] If a rule brings 3,000,000 data-following bubbles under 4% at no more than
      1.5x the representatives, ship it; otherwise record the table.

## Decisions

- The 300,000-row gates of GUP-749 (5.0% random, 3.0% data-following, 1M window
  3.2%) must keep holding.

## Escalation Triggers

- Every outline-preserving rule breaks the nesting of tiers.

## Out of Scope

- Ordering across chunks (GUP-743).

## Dependencies

### Prerequisite Stories

- GUP-749 ✅

## Testing Strategy

- The bubble image tests at 300,000 rows; a 3,000,000-row measurement with diff
  images under `GUP_LOD_PNG_DIR`.

## Success Metrics

- [ ] 3,000,000 data-following bubbles under 4% of the plot rect.

## Risk Assessment

- **Medium**: an outline rule needs a cell's neighbours, which the
  nearest-better-neighbour scan does not see. _Mitigation_: measure first with a
  brute-force prototype.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` and `mask all-check` pass
- [ ] Rendered output read by eye
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added
