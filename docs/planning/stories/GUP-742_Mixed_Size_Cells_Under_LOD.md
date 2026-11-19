---
implementer: opus
implementer-reason:
  a design change to the tier structure (a second representative per cell) with
  a measured fidelity target
touches: [crates/gup-core/src/lod.rs, crates/gup-core/tests/lod.rs]
---

# GUP-742: Mixed-Size Cells Under LOD

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-19

## Context

GUP-721 gives a size-ordered layer a level of detail: a cell's representative is
its largest mark. Where every cell holds marks of many sizes (300,000 rows with
a size drawn at random for each row, over a thin curve), the full draw shows the
smallest marks on top (drawn last) and the large ones only as a halo, while the
thinned draw shows the largest: 23.0% of the plot rect differs by ΔE00 > 10 at
400 x 300 (16.4% if the smallest wins instead, which loses the halo). One
representative per cell cannot show both.

## User Story

> "As a developer of a bubble chart whose sizes are unrelated to position, I
> want the thinned draw to look like the full draw, so that I need not switch
> LOD off."

## Acceptance Criteria

### AC1: A second representative where a cell's sizes differ

- [ ] A cell whose rows span more than one size bucket also keeps its smallest
      bucket's lowest row as a representative (drawn last in its block), within
      a measured share of extra instances.
- [ ] The 300,000-row random-size bubble layer of GUP-721's investigation (a
      size drawn at random for each row over the walk's curve) differs from the
      exact draw by at most a gate set from the measurement (the one-winner
      draw: 23.0%).
- [ ] Size-ordered LOD becomes the default: `Lod::Auto` applies to layers drawn
      largest first (GUP-721 made it opt-in through `Lod::AutoSized` because of
      the 23% mixed-size picture); `AutoSized` is then removed.
- [ ] Tiers still nest (the brute-force test of GUP-721, extended).

## Decisions

- Measure first: the number of mixed cells, extra instances and the share of
  pixels over ΔE00 10 for the largest+smallest rule, against the 4 quarter-pixel
  cells a pixel already holds (GUP-716's bias).

## Escalation Triggers

- Two representatives per cell cannot keep tiers as prefixes of one order
  without changing the `first_tier` rule: stop and report.

## Out of Scope

- Layers of one size; ordering across chunks (GUP-743).

## Dependencies

### Prerequisite Stories

- GUP-721 ✅

## Testing Strategy

- Brute-force nesting and sets; the random-size image test.

## Success Metrics

- [ ] The random-size bubble case within a stated gate, instances under 10% of
      rows.

## Risk Assessment

- **High**: a second representative may double instances or break nesting.
  _Mitigation_: measure before building.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` and `mask all-check` pass
- [ ] Rendered output read by eye (the bubble PNG pair)
- [ ] WASM size reported
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added
