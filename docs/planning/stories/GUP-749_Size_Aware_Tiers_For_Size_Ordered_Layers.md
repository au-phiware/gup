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

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-20) **Created**: 2026-11-20

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

- [x] `Tiers::build` takes, for a ranked layer, each row's bucket and each
      bucket's tier shift (`floor(log2 r)` from its pixel radius `r`, or the
      rule the tuning settles on); a bucket's rows at drawn tier `t` are its
      lowest row per cell of tier `t - shift`. A row's first tier is its
      bucket's first tier plus the shift (past tier 16: never).
- [x] Tiers nest: a brute-force test (as GUP-721's, three distributions, random
      16-level buckets and shifts 0-3) asserts every tier's set equals the union
      over buckets of each bucket's lowest row per shifted cell, and each tier
      contains the previous.
- [x] Peak build memory at most 16 B a row (GUP-714's bound), measured with the
      counting allocator of `scale_bench --lod`.

### AC2: Rank order within a chunk

- [x] A chunk drawing tier `t` draws its prefix largest first across the tier's
      blocks (not block after block), within `MAX_RANGES` ranges a chunk; the
      draw calls of the full and zoomed views are measured and stay within
      GUP-743's bound (at most 2x GUP-721's).
- [x] Layers in data order draw byte-identical images.

### AC3: Fidelity and instances

- [x] 300,000 rows, a size drawn at random per row over the walk's curve
      (GUP-742's probe scene), 400 x 300: at most **5.0%** of the plot rect over
      ΔE00 10 (prototype 3.03%), at most about 41,000 instances (13.7% of rows;
      prototype 40,965).
- [x] The data-following bubbles (GUP-721's `bubbles`): at most **3.0%**
      (GUP-721's gate; prototype 3.84%, so the large marks' edges need tuning),
      about 4,000 instances (prototype 4,059; 11,371 today).
- [x] The one-colour bubbles within 2.0%; the appended and windowed bubble tests
      within their gates (or re-derived, with the measurement, if rank order
      improves them).
- [x] At 10M bubbles, instances bounded by the picture, not the rows: report
      instances and first-frame time against GUP-721's 7,679 and 2.2 s.

### AC4: The default

- [x] When AC3's gates hold: `Lod::Auto` applies to layers drawn largest first,
      `Lod::AutoSized` is removed (0.1 is unpublished: no deprecation), the
      `Lod` docs and the CHANGELOG say so, and
      `a_layer_drawn_by_size_ignores_auto` becomes a test that it takes LOD.
- [x] If they do not hold, stop and report (Escalation Triggers); the default is
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

- [x] Per-bucket first tiers with shifts in `Tiers::build` (one radix sort by
      `(bucket, key)` or per-bucket passes; measure).
- [x] Bucket radii from the size encoding at the build call sites; rebuild on a
      size-scale change.
- [x] Rank-ordered ranges within a chunk.
- [x] Tune the large marks' cells against the data-following gate.
- [x] Flip the default, remove `AutoSized`, CHANGELOG.

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

- [x] Random sizes within 5.0%, data-following within 3.0%, at the instances
      above; size-ordered LOD on by default.

## Risk Assessment

- **High**: the data-following case regresses while the random case improves
  (the prototype did). _Mitigation_: tune the shift rule on both scenes before
  wiring the default; the escalation trigger bounds the trade.
- **Medium**: rebuilds on size-scale changes (an auto domain growing with
  appends). _Mitigation_: measure an appended bubble layer's rebuild count.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] `mask all-fix` and `mask all-check` pass
- [x] Rendered output read by eye (the random and data-following PNG pairs and
      their diff images)
- [x] WASM size reported
- [x] Story status updated in the story file and INDEX.md
- [x] Retrospective added

## Implementation Summary

**Completed**: 2026-11-20

- **Per-bucket tiers** (`crates/gup-core/src/lod.rs`):
  `Tiers::build(rows, xy, Option<&Ranks>)`; `Ranks` gives each row's rank (its
  12.5% size bucket) and each bucket's shift. `by_bucket` stably regroups the
  key-sorted pairs by bucket (the pairs and one copy: 16 B a row); `first_tiers`
  runs once per bucket and the shift is added (a bucket's tier-0 row stays at
  tier 0, so every bucket draws at every tier; past 16: never). `best_first` and
  `by_rank` are gone.
- **The rule** (`Shifts`, `SHIFT_RADIUS` 2 px, `MAX_SHIFT` 1): marks of 4 px
  radius and more keep their representatives on cells one tier coarser; smaller
  marks on the drawn tier's. `Shifts::new` bisects the rank space once per power
  of two through the size encoding's `image` (`Selection::lod_shifts`,
  `DrawOrder::lod_value`) every prepare that may build; the worker build
  (GUP-730) receives a clone with the keys.
- **Scrambled pick**: a bucket's representative of a cell is its row of the
  lowest `scramble(row)` (a bijection), not its lowest row: in rows sorted by x
  the lowest row is the leftmost of its cell, which moved a bucket's marks left
  by half a cell (random sizes at 300,000 rows: 3.05% -> 2.30% under the
  prototype rule; 3M rows with no shift: 2.42% -> 0.99%). Layers in data order
  keep the lowest row, so their images are byte-identical (the 1M golden and
  every unranked test pass unchanged).
- **Rank order within a chunk** (AC2): a ranked chunk's `order` holds a copy of
  each stored tier, bucket after bucket and Morton order within one (a third
  more representatives than the finest tier, on the CPU and the GPU). A frame
  draws one range per chunk in the full view: 6 draw calls for the 300,000
  bubbles, as many as `Off` (one range per bucket on the prefix layout drew 21).
  `Run` carries `first..=last`, the tiers that draw it (`Run::draws`); `ranges`
  and `in_view` take the tier; `DrawEntry::lod` holds the tier drawn.
- **Staleness**: `Tiers::shifts` records each bucket's shift;
  `ColumnStore::has_tiers` and `shifts_hold` compare them with the current
  `Shifts`, so a size-scale change rebuilds a chunk only when one of its buckets
  crosses 4 px (a rebuild bumps the pick epoch as any rebuild does); the stale
  tiers draw meanwhile. `Building` records the shifts it builds with.
- **Default flipped** (AC4): `Lod::AutoSized` is removed; `Lod::Auto` takes
  layers drawn largest first. `Lod` docs, CHANGELOG (Added, Known limitations)
  and README updated.
- **Tests**: `lod::tests::ranked_tiers_keep_each_buckets_shifted_cells_and_nest`
  (brute force: three distributions, 16 random buckets, shifts 0-3; sets,
  nesting, rank order, runs), `shifts_follow_the_radius`,
  `one_bucket_keeps_one_row_per_cell`; `tests/lod.rs`
  `random_sizes_match_the_exact_draw` (new, diff image),
  `a_layer_drawn_by_size_takes_lod` (was `..._ignores_auto`),
  `a_layer_drawn_by_size_matches_the_exact_draw` (diff image, draw calls),
  `a_size_scale_change_rebuilds_the_tiers_it_moves` (new).
  `scale_bench --lod --sized` measures a ranked build. `tests/lod.rs` 39 and
  `tests/window.rs` 12 pass on the default device and under lavapipe.

### Measurements

400 x 300, share of the plot rect over ΔE00 10, representatives; default device
(lavapipe in brackets):

| scene                                | this story                   | GUP-721 (largest)     | gate |
| ------------------------------------ | ---------------------------- | --------------------- | ---- |
| 300,000 random sizes                 | 0.235% (0.242%), 80,758      | 23.0%, 11,375         | 5.0% |
| 300,000 data-following (`bubbles`)   | 2.244% (2.261%), 7,316       | 2.303%, 11,371        | 3.0% |
| one colour                           | passes                       | 0.884%                | 2.0% |
| appended, tiers / past them          | 2.777% / 2.888% (2.79/2.90%) | 3.258% / 3.363%       | 4.5% |
| 1M rolling window                    | 2.812% (2.813%), 3,665       | 2.437%                | 3.2% |
| 3M data-following / random (no gate) | 5.188%, 4,544 / 3.821%       | 5.160%, 6,653 / 33.1% | -    |
| 10M data-following (release)         | 5,169, first frame 1.78 s    | 7,679, 2.2 s          | -    |
| 10M random sizes (release)           | 81,311, first frame 2.22 s   | -                     | -    |

The rule's tuning (300,000 rows, copies and scrambled pick):

| shift                    | random        | data-following | 1M window |
| ------------------------ | ------------- | -------------- | --------- |
| `floor(log2 r)`, 0 to 3  | 2.30%, 40,965 | 3.54%, 4,059   | -         |
| the same, at most 2      | 1.63%, 42,633 | 3.30%, 4,208   | -         |
| the same, at most 1      | 0.27%, 64,214 | 2.57%, 5,749   | 3.211%    |
| 1 from 3 px              | 0.27%, 72,541 | 2.41%, 6,580   | 3.073%    |
| **1 from 4 px (chosen)** | 0.24%, 80,758 | 2.24%, 7,316   | 2.812%    |

- **Build** (`scale_bench --lod`, 10M rows, one thread, release): peak 16.00 B a
  row ranked (16 random levels, half shifted) and unranked; 1,090 ms against 598
  ms per 10M rows.
- **WASM** (`mask wasm-size`): scatter 390,449 B gz (recorded 388,517, limit
  396,287), over wgpu 358,089 B (recorded 356,157; soft line 360,000): about
  +1.9 KB for the per-bucket build, `Shifts` and the copies, which a scatter
  without a size channel links but does not run. Main was not re-measured in
  this session; the recorded figures are main's.

### Acceptance criteria met differently

- **AC1**: the shift is `min(1, floor(log2(r / 2)))` (the tuning above), not
  `floor(log2 r)`; a bucket's tier-0 row stays at tier 0; "lowest row" is the
  lowest scrambled row within a bucket.
- **AC2**: rank order through a copy of each stored tier instead of ranges on
  the prefix, so the full view draws one range per chunk (GUP-721's draw calls,
  not 2x).
- **AC3**: both fidelity gates hold with margin, at more representatives than
  the prototype's: random sizes 80,758 (prototype 40,965; the escalation bound
  is 82,000), data-following 7,316 (about 4,000 asked; GUP-721 drew 11,371). No
  rule measured meets both gates at the prototype's instances: the clumps'
  outlines recede by up to a cell, whatever the radius. "At most 1" (64,214)
  failed the 1M rolling window's 3.2% gate (3.211%), which chose 4 px over 2 px.
- **Risk (appended rebuilds)**: not measured on an auto domain; the appended
  test (fixed domain) builds exactly as before, and with one threshold a bucket
  rebuilds only when its radius crosses 4 px.

## Retrospective

**Completed**: 2026-11-20

### Key Technical Learnings

#### Rank order, not coarser cells, fixed the random sizes

- **Challenge**: the prototype's size-aware cells reached 3.0% for random sizes
  only with a measurement hack (sorting the drawn prefix).
- **Solution**: with each tier drawn in rank order, every bucket kept one tier
  coarser already reaches 0.27%; the radius scaling saved instances and cost
  fidelity on the outlines of large marks.
- **Pattern**: separate the order effect from the set effect before tuning a set
  rule; GUP-742's table mixed them.

#### Nested sets drawn in a global order: copy each tier

- **Challenge**: tiers as prefixes of one order cannot also be drawn in rank
  order across blocks without one range per bucket (21 draw calls against 6).
- **Solution**: store each tier's set as its own rank-ordered copy. The copies
  form a geometric series (each tier about 4x the one before), so they cost a
  third more than the finest tier.
- **Pattern**: when nested sets each need their own order, copies are cheap if
  the sets grow geometrically.

#### "Lowest row" is a position bias in sorted data

- **Challenge**: at 3M rows every shifted rule degraded sharply.
- **Solution**: in x-sorted data the lowest row of a cell is its leftmost point;
  a bijective scramble of row numbers keeps any total order's nesting and
  removes the bias.
- **Pattern**: any total order nests; choose one uncorrelated with position.

#### Outlines recede by a cell, whatever the radius

- **Challenge**: "larger marks need fewer representatives" held for interiors
  but not for outlines: a clump's edge recedes by up to one of its cells,
  visible against a contrasting neighbour.
- **Solution**: cap the shift at one tier and start it at 4 px.
- **Pattern**: fidelity at an edge depends on the cell size in pixels, not on
  the mark's radius.

### Architectural Decisions

#### Per-tier copies for ranked chunks only

- **Decision**: unranked chunks keep the progressive prefix; ranked chunks store
  copies. `Run` carries the tiers that draw it, so `ranges` serves both.
- **Reasoning**: byte-identical images and unchanged memory for scatters; one
  draw per chunk for bubbles.
- **Trade-off**: a third more GPU and CPU memory for a ranked chunk's
  representatives; a ranked build takes about 1.8x an unranked one.
- **Future**: GUP-743 can order the chunks' single ranges globally.

#### Shifts read through the encoding, checked per bucket

- **Decision**: thresholds in rank space from the size encoding's `image` each
  prepare; a chunk's tiers rebuild only when one of its buckets' shift changes.
- **Reasoning**: an auto domain that grows with appends would otherwise rebuild
  every chunk on every change.
- **Trade-off**: about 32 encoding calls a prepare for a ranked layer.

### Development Workflow Insights

- Environment-gated knobs (`GUP749_DIV`, `GUP749_MAX`, a "largest" baseline
  rule) and a probe test gave a dozen rule measurements in under an hour; the
  baseline knob reproduced GUP-742's numbers exactly before anything was
  compared with it.
- Measuring at 3M rows as well as 300,000 found the position bias, which the
  gate scene hid.
- The worktree sandbox refused heredocs, `cd` and `env -u` in compound commands;
  scripts in a scratch directory run with `python3 -I` or `bash` worked.

### Follow-up Stories

1. **GUP-751: Outlines Of Dense Bubble Layers Under LOD** — 3M data-following
   bubbles differ 5.2% (the largest rule did too): a representative anywhere in
   its cell lets a clump's outline recede; measure an outline-preserving rule.
2. **GUP-752: Ranked Tier Build Time And Memory** — a ranked build takes 1.8x an
   unranked one (1,090 ms against 598 per 10M rows) and its copies a third more
   memory; measure where the time goes and whether one pass can build the
   copies.
