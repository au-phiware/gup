---
implementer: sonnet
implementer-reason: a diagnosis in the plot's layout and scale path, then a fix
touches: [crates/gup-core/tests/lod.rs, crates/gup-core/src/plot.rs]
---

# GUP-737: Windowed Plot Differs From a Fresh Plot by One Pixel Row

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New **Created**:
2026-11-16

## Context

Found in GUP-719. Appending rows far from a layer's old marks, over fixed scale
domains, changes one row of pixels of the old marks. The test is
`appends_that_grow_the_extent_leave_full_chunks_alone` in
`crates/gup-core/tests/lod.rs` (two full 250,000-row chunks over x 0..500, y
0..100, scales x 0..5000 and y 0..1000, then 10,000 rows at x 2,500..5,000 and y
500..1,000).

Evidence, in the plot's bottom-left corner (the old cloud):

- Under `Lod::Auto`: 74 pixels differ, all in pixel row y = 239 (the top edge of
  the cloud, data y = 100), for example (116, 178, 213) before and (163,
  204, 227) after at x = 53; the instance counts of the old chunks are equal and
  a second frame without an append is byte-identical to the first.
- Under `Lod::Off` (no tiers at all): the same kind of difference, for example
  pixel (87, 239) blue (161, 202, 228) before and white after.

So it is not the tiers. Suspects: the plot's per-frame layout or the edge solve
(GUP-567) reading the layer's extent, moving a mark by a sub-pixel; the new
chunk changing a chunk uniform of the old ones.

**Cause (GUP-741)**: the appended rows reach the scale domains' ends (x 5,000, y
1,000), so the edge solve (GUP-567) insets the scale ranges (the axis rules end
at 376.5 and start at 17.49 instead of 378 and 16) and every old mark moves by a
sub-pixel amount. GUP-719's test now keeps its rows clear of the ends.

## User Story

> "As a developer of a live chart, I want an append that adds no marks near the
> old ones to leave them as they were, so that a streaming picture does not
> shimmer."

## Acceptance Criteria

- [ ] The cause is found and written down.
- [ ] The AC2 test of GUP-719 compares the old corner byte for byte (its
      allowance of two pixel rows is removed), under `Auto` and `Off`.

## Decisions

- If the cause is a deliberate sub-pixel adjustment, the fix is in the test,
  with the reason.

## Escalation Triggers

- The cause is a change in the plot rect (axes would move too): report.

## Out of Scope

- Windows' draw order (GUP-492).

## Technical Tasks

- [ ] Bisect with `Lod::Off`: layer extent, chunk uniforms, edge solve.
- [ ] Fix, or document and tighten the test.

## Dependencies

### Prerequisite Stories

- GUP-719 (the test).

## Testing Strategy

- The GUP-719 test, strict.

## Success Metrics

- [ ] No pixel of the old cloud changes.

## Risk Assessment

- **Low**: a few pixels at a mark's edge.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` and `mask all-check` pass
- [ ] Story status updated in the file and INDEX.md
