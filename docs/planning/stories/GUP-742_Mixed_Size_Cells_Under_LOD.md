---
implementer: opus
implementer-reason:
  a design change to the tier structure (a second representative per cell) with
  a measured fidelity target
touches: [crates/gup-core/src/lod.rs, crates/gup-core/tests/lod.rs]
---

# GUP-742: Mixed-Size Cells Under LOD

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-20, investigation) **Created**: 2026-11-19

**Outcome**: No change shipped; size-aware tiers are GUP-749; size-ordered LOD
stays opt-in (`Lod::AutoSized`).

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

- [x] A cell whose rows span more than one size bucket also keeps its smallest
      bucket's lowest row as a representative (drawn last in its block), within
      a measured share of extra instances. _Measured, not shipped_: +97%
      instances for random sizes, +0 for data-following sizes (see the
      Investigation Results).
- [ ] The 300,000-row random-size bubble layer of GUP-721's investigation (a
      size drawn at random for each row over the walk's curve) differs from the
      exact draw by at most a gate set from the measurement (the one-winner
      draw: 23.0%). _Not met_: gate set at 5%; the rule measures 12.73%.
- [ ] Size-ordered LOD becomes the default: `Lod::Auto` applies to layers drawn
      largest first (GUP-721 made it opt-in through `Lod::AutoSized` because of
      the 23% mixed-size picture); `AutoSized` is then removed. _Not done_:
      moved to GUP-749, which flips the default when its gates are met.
- [x] Tiers still nest (the brute-force test of GUP-721, extended). _Shown by
      argument_ (below); no test shipped with the rule not shipping.

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
      rows. _Not met_: 12.73% against a 5% gate, 7.5% of rows.

## Risk Assessment

- **High**: a second representative may double instances or break nesting.
  _Mitigation_: measure before building.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked (as an investigation:
      see each AC's note)
- [x] `mask all-fix` and `mask all-check` pass
- [x] Rendered output read by eye (the bubble PNG pair, and a diff image per
      rule)
- [x] WASM size reported: unchanged (no code change ships)
- [x] Story status updated in the story file and INDEX.md
- [x] Retrospective added

## Investigation Results (2026-11-20)

**Decision** (coordinator, 2026-11-20): option B below. Largest + smallest is
not shipped, not even under the opt-in: doubling random-size instances for 23.0%
-> 12.7% is not worth the code. The prototypes (environment-gated rules in
`Tiers::build` and a probe test, commit 4edf9dfd) were removed; no behaviour
changes. Size-aware tiers with rank order within a chunk are GUP-749.

**The question it answered**: the largest+smallest rule brings the random-size
bubbles from 23.0% to 12.7% of the plot rect, not to a level I would defend as a
default.

**The gate I would defend** for the random-size case: at most 5% of the plot
rect over ΔE00 10 at 400 x 300. The data-following bubbles measure 2.3% (gate
3.0%) and GUP-717's scatters 1.6% / 2.3% (gates 2.0% / 3.0%); a size drawn at
random per row is the adversarial case, so about twice the bubble measurement,
but not a picture that reads differently. At 12.7% it does: every band of the
curve gets a thick yellow ring where the full draw has a thin gradient rim. The
diff image (pixels over ΔE00 10 in red over the faded exact draw) shows a red
ring 6-8 px wide following both edges of the whole band, and red patches inside
the band where a finer block's larger marks cover a coarser block's smallest
representative; the band's purple core and its outermost yellow edge are not
red.

**Why one or two marks per cell cannot do it**: the full draw shows, at each
pixel, the smallest size with a mark within its own radius. Inside the dense
band that is the smallest size; outside it, at distance `d` from the band's
edge, it is the size whose radius is `d`, a gradient through all 16 sizes over 8
px around the whole curve. Largest + smallest gets the core (purple) and the
outer edge (yellow) right and the 6-8 px of mid sizes between them wrong; that
ring is about 10% of the plot rect here.

**Measured** (300,000 rows, the walk's curve, 16 size levels drawn at random per
row or in 100 clumps (GUP-721's `bubbles`), radius 1-9 px, viridis by level, 400
x 300, `Auto` against `Off`, share of the plot rect over ΔE00 10, default
device, debug build; prototypes in `Tiers::build` behind environment variables,
since removed):

| rule                                         | random | instances | clumps | instances |
| -------------------------------------------- | ------ | --------- | ------ | --------- |
| largest (shipped, `AutoSized`)               | 23.00% | 11,375    | 2.303% | 11,371    |
| smallest                                     | 16.47% | 11,375    | 2.424% | 11,371    |
| **largest + smallest (this story's rule)**   | 12.73% | 22,408    | 2.426% | 11,371    |
| largest + smallest, rank order across blocks | 10.93% | 22,408    | 1.801% | 11,371    |
| largest, rank order across blocks            | 22.21% | 11,375    | 1.793% | 11,371    |
| largest + smallest, one tier finer (bias 2)  | 5.80%  | 58,967    | 1.430% | 30,812    |
| every size bucket of each cell               | 2.62%  | 122,046   | 2.426% | 11,371    |
| every size bucket, one tier coarser (bias 0) | 3.51%  | 55,949    | 3.824% | 4,457     |
| size-aware (below)                           | 6.22%  | 40,965    | 4.636% | 4,059     |
| size-aware, rank order across blocks         | 3.03%  | 40,965    | 3.842% | 4,059     |
| size-aware, cells half as coarse             | 3.69%  | 76,465    | 3.261% | 6,900     |

- Largest + smallest: tiers still nest (the union of two nested families, each
  the GUP-721 rule under one total order, nests; a row's first tier is the
  smaller of its two), so the escalation trigger does not fire. Mixed cells: at
  the drawn tier (9) in the random case nearly every cell (22,408 against
  11,375); in the clumps none (mixed cells exist at tiers 0-8 only, whose
  smallest rows are already tier-9 representatives), so clumps draw the same
  instances, but in other blocks: 2.303% -> 2.426%.
- "Rank order across blocks" sorts the drawn prefix by rank (a measurement hack:
  it breaks the prefix structure; GUP-743's ranges would do it properly).
- "Size-aware": every size bucket keeps its own representatives, on a grid whose
  cell side grows with the bucket's pixel radius `r` (`2^floor(log2 r)` cells of
  the drawn tier, so 0.5 px to 4 px a side here), RFC-001 "Mark size" ("larger
  marks ... could take coarser tiers; not measured"). The prototype read the
  radius from the test's own size scale. Its clumps fail their 3.0% gate on the
  outlines of large marks (union edges at up to 4 px spacing).

**Options** (as reported when blocked)

- **A. Ship largest + smallest under `AutoSized` only** (default not flipped):
  random 23.0% -> 12.7%, clumps 2.30% -> 2.43% (gate 3.0% holds), random
  instances +97% (7.5% of rows), clumps +0, one more sort-free pass of the build
  for ranked layers (keys recomputed, peak 14 B a row). Close GUP-742 with AC1's
  third box unmet.
- **B. Re-scope to size-aware tiers** (new story, GUP-749; with GUP-743's
  within-chunk rank order): the only measured family that reaches the 5% gate
  under 10% of rows at scale (3.0% random; 40,965 instances is 13.7% of 300,000
  rows but bounded by the band's area, not the rows, at 10M), and it draws 64%
  fewer instances for the clumps. Costs: the build needs each bucket's pixel
  radius (the size encoding), so a size-scale change rebuilds tiers; the first
  tier rule becomes per bucket (this story's escalation trigger); the large
  marks' edges need tuning (3.8% clumps against a 3.0% gate).
- **C. Flip the default with largest + smallest** and a gate of about 16% (1.3x
  12.7%) for random sizes, documenting it: not recommended; the picture differs
  visibly.

**Recommendation**: B, with `AutoSized` kept opt-in until it lands; A only if an
interim improvement for opt-in users is wanted (it is superseded by B, which
keeps every bucket and so contains largest + smallest).

## Retrospective

**Completed**: 2026-11-20

### Key Technical Learnings

#### What a dense layer of random sizes looks like

- **Challenge**: the story assumed a second representative (the smallest) would
  restore what the largest alone hides.
- **Solution**: reading the full draw as "each pixel shows the smallest size
  with a mark within its own radius" predicted the result before measuring: the
  core and the outer edge come right, and the rim between them, a gradient
  through every size, cannot come from two sizes. The measurement (12.7%) and
  the diff image agreed.
- **Pattern**: for overlapping marks of many sizes, the number of
  representatives a size needs falls with its radius squared; a fixed number per
  cell is either too few for small marks or too many for large ones.

#### Any union of nested families nests

- **Challenge**: the escalation trigger feared two representatives would break
  tiers as prefixes of one order.
- **Solution**: each rule (best under one total order) nests on its own, and a
  union of nested families nests; a row's first tier is the smaller of its two.
  The same argument covers GUP-749's one family per size bucket.

### Architectural Decisions

#### Measure rules behind environment variables before building one

- **Decision**: eleven rules measured on one probe test in about an hour,
  through environment-gated branches in `Tiers::build`, then removed.
- **Reasoning**: the story's own rule missed the gate; the alternatives (rank
  order across blocks, a finer tier, every bucket, size-aware cells) had to be
  compared on the same scene to choose a direction.
- **Trade-off**: none ships; the numbers are in this story only.

### Development Workflow Insights

- A diff image (red over a faded exact draw) separated the rim from the core
  errors at a glance; the share alone did not say why.
- The worktree sandbox refused loops over environment values and
  `scripts/with_cargo_env.sh git commit`; one command per run and a plain
  `git commit` with the checkout's `CARGO_*` variables exported worked.

### Follow-up Stories

1. **GUP-749: Size-Aware Tiers For Size-Ordered Layers** — per-bucket
   representatives on cells that grow with the mark's pixel radius, drawn in
   rank order within a chunk; flips the default and removes `Lod::AutoSized`
   when it meets the gates. GUP-743 keeps the order across chunks.
