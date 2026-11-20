---
implementer: opus
implementer-reason:
  a design change to the tier build (one family of representatives per size
  bucket on cells scaled by the mark's pixel radius) with measured fidelity
  gates, and flipping a public default
touches:
  [
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/order.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/tests/lod.rs,
    CHANGELOG.md,
  ]
---

# GUP-749: Size-Aware Tiers For Size-Ordered Layers

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-20

Planned, not New: GUP-742 measured the rule and the coordinator chose it (option
B of GUP-742's Investigation Results); the targets below come from that
prototype. What remains open is tuning, not direction.

## Context

A size-ordered (bubble) layer takes a level of detail only under the opt-in
`Lod::AutoSized` (GUP-721), because one representative per cell, the largest,
cannot draw a cell of many sizes as the full draw does: 300,000 rows with a size
drawn at random per row differ from the exact draw in 23.0% of a 400 x 300 plot
rect over ΔE00 10. GUP-742 measured eleven rules. A second (smallest)
representative reaches only 12.7%; the full draw shows at each pixel the
smallest size with a mark within its own radius, so its rim is a gradient
through every size, which needs every size present at a spacing about its
radius.

The rule that reaches a defensible level: **size-aware tiers**. Each size bucket
keeps its own representatives (its lowest row per cell, GUP-714's rule within
the bucket), on a grid whose cell side grows with the bucket's pixel radius `r`:
the bucket's representatives at drawn tier `t` are its representatives of tier
`t - floor(log2 r)` (cells of the drawn tier, 0.5 px a side under GUP-716's
bias, so 0.5 px to 4 px for radii 1 to 9 px). RFC-001 "Mark size" ("larger marks
overlap more and could take coarser tiers for the same fidelity; not measured")
anticipated it. Drawn with the chunk's prefix in rank order (largest first
across the tier's blocks, not per block), the prototype measured:

| rule (GUP-742 prototype)               | random | instances | data-following | instances |
| -------------------------------------- | ------ | --------- | -------------- | --------- |
| largest (shipped, `AutoSized`)         | 23.00% | 11,375    | 2.303%         | 11,371    |
| size-aware, per-block rank order       | 6.22%  | 40,965    | 4.636%         | 4,059     |
| size-aware, rank order across blocks   | 3.03%  | 40,965    | 3.842%         | 4,059     |
| size-aware, cells half as coarse       | 3.69%  | 76,465    | 3.261%         | 6,900     |
| every bucket of each cell (no scaling) | 2.62%  | 122,046   | 2.426%         | 11,371    |

The data-following case (GUP-721's `bubbles`: 100 clumps of one size) fails its
3.0% gate on the outlines of large marks, whose representatives are up to 4 px
apart: the union of their disks is scalloped where the exact draw's edge is
smooth. That is the tuning this story owns.

Rank order across a chunk's blocks is the within-chunk half of GUP-743; this
story absorbs it. GUP-743 keeps the order across chunks (appended and windowed
layers).

## Read First

- GUP-742's Investigation Results (the table, the gate reasoning, the nesting
  argument) and Retrospective.
- GUP-721's Implementation Summary and Retrospective (ranks, `best_first`,
  `by_rank`, runs per rank, `DrawOrder::lod_rank`).
- GUP-714's Retrospective ("lowest row per cell" is a nearest-smaller-value scan
  for any total order).
- `crates/gup-core/src/lod.rs` (`Tiers::build`, `first_tiers`, `runs`, `spread`,
  `ranges`), `crates/gup-core/src/order.rs` (`lod_rank_of`, buckets),
  `crates/gup-core/src/column.rs` (the two `build_tiers` call sites, GUP-730's
  worker builds).
- GUP-743 (what stays there).

## User Story

> "As a developer of a bubble chart whose sizes are unrelated to position, I
> want the thinned draw to look like the full draw, so that I need not switch
> LOD off, and LOD can be on by default for bubbles."

## Acceptance Criteria

### AC1: Per-bucket representatives on radius-scaled cells

- [ ] `Tiers::build` takes, for a ranked layer, each row's bucket and each
      bucket's tier shift (`floor(log2 r)` from its pixel radius `r`, or the
      rule the tuning settles on); a bucket's rows at drawn tier `t` are its
      lowest row per cell of tier `t - shift`. A row's first tier is its
      bucket's first tier plus the shift (past tier 16: never).
- [ ] Tiers nest: a brute-force test (as GUP-721's, three distributions, random
      16-level buckets and shifts 0-3) asserts every tier's set equals the union
      over buckets of each bucket's lowest row per shifted cell, and each tier
      contains the previous.
- [ ] Peak build memory at most 16 B a row (GUP-714's bound), measured with the
      counting allocator of `scale_bench --lod`.

### AC2: Rank order within a chunk

- [ ] A chunk drawing tier `t` draws its prefix largest first across the tier's
      blocks (not block after block), within `MAX_RANGES` ranges a chunk; the
      draw calls of the full and zoomed views are measured and stay within
      GUP-743's bound (at most 2x GUP-721's).
- [ ] Layers in data order draw byte-identical images.

### AC3: Fidelity and instances

- [ ] 300,000 rows, a size drawn at random per row over the walk's curve
      (GUP-742's probe scene), 400 x 300: at most **5.0%** of the plot rect over
      ΔE00 10 (prototype 3.03%), at most about 41,000 instances (13.7% of rows;
      prototype 40,965).
- [ ] The data-following bubbles (GUP-721's `bubbles`): at most **3.0%**
      (GUP-721's gate; prototype 3.84%, so the large marks' edges need tuning),
      about 4,000 instances (prototype 4,059; 11,371 today).
- [ ] The one-colour bubbles within 2.0%; the appended and windowed bubble tests
      within their gates (or re-derived, with the measurement, if rank order
      improves them).
- [ ] At 10M bubbles, instances bounded by the picture, not the rows: report
      instances and first-frame time against GUP-721's 7,679 and 2.2 s.

### AC4: The default

- [ ] When AC3's gates hold: `Lod::Auto` applies to layers drawn largest first,
      `Lod::AutoSized` is removed (0.1 is unpublished: no deprecation), the
      `Lod` docs and the CHANGELOG say so, and
      `a_layer_drawn_by_size_ignores_auto` becomes a test that it takes LOD.
- [ ] If they do not hold, stop and report (Escalation Triggers); the default is
      not flipped.

## Decisions

- **Shift from pixels, at build time**: the bucket's radius is read through the
  size encoding when the chunk's tiers are built; a change of the size scale
  (domain or range) invalidates the layer's tiers like any other rebuild (a pick
  epoch bump, GUP-719). The worker build (GUP-730) receives the shifts with the
  ranks.
- **Gates**: random sizes 5.0% (GUP-742: about twice the data-following
  measurement, not a picture that reads differently), data-following 3.0%
  (GUP-721), one colour 2.0%.
- **Rank order across chunks stays in GUP-743.**

## Escalation Triggers

- The data-following bubbles cannot reach 3.0% without raising random-size
  instances above about 2x the prototype (82,000): report the trade-off table.
- Rank order within a chunk needs more than `MAX_RANGES` ranges a chunk in the
  full view.
- The shift cannot be known at build time for some size encoding (a size channel
  whose pixel radius depends on the view).

## Out of Scope

- Ordering across chunks (GUP-743); marks other than circles and constant-size
  rects; size-aware tiers for layers of one size (a tier bias by radius for
  plain scatters is a separate question).

## Technical Tasks

- [ ] Per-bucket first tiers with shifts in `Tiers::build` (one radix sort by
      `(bucket, key)` or per-bucket passes; measure).
- [ ] Bucket radii from the size encoding at the build call sites; rebuild on a
      size-scale change.
- [ ] Rank-ordered ranges within a chunk.
- [ ] Tune the large marks' cells against the data-following gate.
- [ ] Flip the default, remove `AutoSized`, CHANGELOG.

## Dependencies

### Prerequisite Stories

- GUP-742 ✅ (the measurements)
- GUP-721 ✅, GUP-730 ✅

### Enables Stories

- GUP-743 (narrowed to across chunks)

## Testing Strategy

- Brute-force sets and nesting with shifts; the random-size and data-following
  image tests, a diff image saved under `GUP_LOD_PNG_DIR`; draw-call bounds;
  `scale_bench --lod` peak bytes.

## Success Metrics

- [ ] Random sizes within 5.0%, data-following within 3.0%, at the instances
      above; size-ordered LOD on by default.

## Risk Assessment

- **High**: the data-following case regresses while the random case improves
  (the prototype did). _Mitigation_: tune the shift rule on both scenes before
  wiring the default; the escalation trigger bounds the trade.
- **Medium**: rebuilds on size-scale changes (an auto domain growing with
  appends). _Mitigation_: measure an appended bubble layer's rebuild count.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix` and `mask all-check` pass
- [ ] Rendered output read by eye (the random and data-following PNG pairs and
      their diff images)
- [ ] WASM size reported
- [ ] Story status updated in the story file and INDEX.md
- [ ] Retrospective added
