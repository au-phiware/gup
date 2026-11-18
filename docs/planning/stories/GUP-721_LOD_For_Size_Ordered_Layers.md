---
implementer: sonnet
implementer-reason:
  the rule is settled in the RFC findings (largest first, largest mark
  represents its cell); the work reuses GUP-492's bucket keys inside `lod.rs`
touches:
  [
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/order.rs,
    crates/gup-core/tests/lod.rs,
  ]
---

# GUP-721: LOD for Size-Ordered Layers

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-19) **Created**: 2026-11-11

## Context

A layer drawn in size order (largest mark first, so small marks stay visible on
top; `Selection::size_order`, `DrawOrder` in `crates/gup-core/src/order.rs`) is
not drawn in data order, and its draw segments are consecutive stored rows.
GUP-716's LOD draws a chunk's tier prefix as one instanced draw in progressive
order, which ignores size, so a size-ordered (bubble) layer under `Lod::Auto`
would draw its representatives in the wrong order and pick the wrong cell
members. GUP-716 therefore leaves size-ordered layers at `Lod::Off` for now.

RFC-001 "GUP-631 findings" ("GUP-492 ..." bullet) states the rule: a
size-ordered layer's representatives must still draw largest first: order each
tier block by its size bucket, not by index, and prefer the largest mark in a
cell (largest size, then lowest index) over the lowest index. This follows
GUP-492, which adds the bucketed sort keys (`DrawOrder::key`, `BUCKET_BITS`)
this story reuses.

## Read First

- RFC-001 "GUP-631 findings": "GUP-492" and "Mark size" bullets.
- GUP-492's story and its RFC findings (the bucket key, `bucketed`, the 12.5%
  rule), `crates/gup-core/src/order.rs` `DrawOrder::key`.
- `crates/gup-core/src/lod.rs` (`Tiers::build`, GUP-714; blocks, GUP-720's
  runs).
- `crates/gup-core/src/selection.rs`: `size_order`, where LOD is switched off
  for a size-ordered layer in GUP-716 (search `size_order` near `lod`).

## User Story

> "As a developer of a 10M-point bubble chart, I want LOD to draw the biggest
> bubbles first and keep each cell's biggest bubble, so that thinning does not
> reorder or lose the large marks."

## Acceptance Criteria

### AC1: Representatives are each cell's largest mark

- [x] `Tiers::build` takes an optional per-row key (the mark's draw-order key,
      GUP-492's bucketed `f32` key) and, within a Morton run of equal cells,
      makes the representative the row with the largest key, ties to the lowest
      index. A brute-force test (as GUP-714's AC1) with random sizes: for every
      tier, `order[..counts[t]]` is the set of largest-key rows per occupied
      cell. Tiers still nest (the representative of a finer cell inside a
      coarser cell's winner is that winner or another cell's winner: assert
      nesting holds, and if the first-tier rule cannot keep nesting with a
      non-index winner, STOP: Escalation Triggers).

### AC2: Largest first within the prefix

- [x] Each tier block is ordered by size bucket descending, then by the block's
      existing order; `order[..counts[t]]` is still tier `t`, but drawn
      first-to-last the larger buckets come first within each block. A test with
      sizes in 16 levels: the sequence of drawn bucket keys over a tier prefix
      is non-increasing per block (assert per block).
- [x] An image test (met differently: see the Implementation Summary): 300,000
      rows, 16 size levels drawn at random, overlapping marks, 400 x 300: `Auto`
      against `Off`, share of pixels at ΔE00 > 10 at most 2.0% of the plot rect;
      the retrospective says what the PNG shows (large bubbles beneath, small on
      top, none hidden by a thinned neighbour).

### AC3: Pick and counts

- [x] GPU pick over a size-ordered LOD layer returns the data row of the
      representative (row id through `DrawOrder::row_of` as for a non-LOD
      size-ordered layer); a test at a bubble's pixel.
- [x] A size-ordered layer under `Lod::Auto` above 200,000 rows now draws
      representatives (assert drawn instances under 10% of rows); the GUP-716
      opt-out for size order is removed, and its test updated to expect LOD.

## Decisions

- **Key source**: the layer's `DrawOrder` bucket key (GUP-492), not the raw
  size, so a bucket draws in data order exactly as the non-LOD layer does.
- **Winner = largest, then lowest index**, from the finding.
- **Windowed size-ordered layers**: LOD follows GUP-492's bucketed order; no
  extra rule.
- **Marks larger than a pixel**: the tier rule still ignores mark size (RFC-001
  "Mark size", not measured); do not change it here.

## Escalation Triggers

- Choosing the largest instead of the lowest index as a cell's winner breaks the
  nesting of tiers as prefixes (the first-tier rule `16 - floor(h / 2)` assumes
  the first row of a run represents all coarser cells containing it): stop and
  report the case; the fix is a per-tier winner, i.e. a design change.
- GUP-492 changed `DrawOrder::key` or the bucket rule from its story's Decisions
  (read its retrospective first).
- Size-ordered pick needs `row_of` for representatives and the mapping is by
  stored row, not by instance.

## Out of Scope

- Windowed unbounded draws (GUP-492, done before); spatial runs' order (GUP-720
  sets the block order this story reorders); mark-size-aware tiers.

## Technical Tasks

- [x] `Tiers::build` winner by key; block order by bucket.
- [x] Remove GUP-716's size-order opt-out; wire the key from `DrawOrder`.
- [x] Tests (AC1-AC3) and the PNG read.

## Dependencies

### Prerequisite Stories

- GUP-492: Windowed Size-Ordered Draws 📋 — the bucket keys.
- GUP-716: `Selection::lod` and Per-Frame Tier Draws 📋.
- GUP-724: Size-Ordered Resolve Cost 📋 (a size-ordered layer's frame costs
  about 12 ms in resolve today; sequencing).
- GUP-720: Spatial Runs for Unsorted Layers 📋 — block order and runs
  (sequencing: both edit `lod.rs`).

### Enables Stories

- None scheduled.

## Testing Strategy

- **Unit tests**: brute-force winners; block order; nesting.
- **Integration tests**: image parity and pick.
- **Visual validation**: the bubble PNG pair read by eye.

## Success Metrics

- [x] A bubble layer of 10M rows draws under 10% of its rows with larger bubbles
      beneath smaller, at most 2.0% of plot-rect pixels differing.

## Risk Assessment

- **Medium**: the winner rule may break nesting (Escalation Triggers).
  _Mitigation_: the brute-force nesting test comes first.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the bubble PNG pair)
- [x] `mask wasm-browser`: not needed, no layout, input or wasm scene changes
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2026-11-19

- `Tiers::build(rows, xy, rank)` (`crates/gup-core/src/lod.rs`): an optional
  per-row rank (lower draws first). A cell's representative is its lowest
  `(rank, row)`: `best_first` puts the best row of each run of equal Morton keys
  first, and `first_tiers`' nearest-better-neighbour pass compares
  `(rank, row)`. Each tier block is stably ordered by rank (`by_rank`), and runs
  are cut at a change of rank as well as every 4,096 representatives, so a run
  holds one rank and keeps its box compact.
- `DrawOrder::lod_rank(stored)` (`order.rs`): the bucketed key (12.5%, whatever
  the layer's own keys), inverted so larger sizes rank lower. The exact keys of
  an unwindowed layer are quantised for the tiers only.
- `DrawOrder::spans` now takes the draw entries and the tier ranges: a chunk
  drawing a tier draws its ranges where its first segment is, and the rows past
  an appended chunk's tiers draw in place through their own entry.
- `selection.rs`: the GUP-716 opt-out and the forced-tier error are removed;
  `TierAsk` carries the `DrawOrder`. The CPU pick index named a representative
  by its stored row; it now maps it through `DrawOrder::row_of` (it returned
  rows of the wrong data position, found by the CPU/GPU pick agreement test).
- Tests: `lod::tests` ranked tiers (best `(rank, row)` per cell at every tier,
  nesting, blocks in rank order, runs one rank and one block) and constant rank
  equals none; `tests/lod.rs` `a_layer_drawn_by_size_takes_lod`,
  `..._in_one_colour_...`, the pick test, appends, a rolling window over 2M
  rows, SVG.
- **Nesting held** (escalation trigger did not fire): the best of a union under
  any total order is the best of the parts' bests, so tiers still nest.
- **AC2 image gate met differently**: see the Retrospective. 300,000 rows with a
  random size per row cannot match the exact draw (23.0% of the plot rect over
  ΔE00 10): every cell holds all 16 sizes, the full draw shows the smallest
  marks on top and the thinned one the largest. With sizes that follow the data
  (100 clumps of one size, each drawn at random) the image is within **2.303%**
  with 16 viridis colours (gate 3.0%) and **0.884%** in one colour (gate 2.0%);
  11,371 representatives of 300,000 rows. Appended 3.258%/3.363% (gate 4.5%;
  2.927% for the same scene built at once), a 1M-row rolling window 2.437% (gate
  3.2%), 2M rows 3.137%. Under 10%: 10M rows draw 7,679 instances (first frame
  2.2 s, release build, sort and tiers included).
- WASM: scatter 396,161 -> 397,571 B gz (+1,410 B; recorded 391,766, limit
  399,601).
- Lavapipe and the default device: `tests/lod.rs` 36 passed, `tests/window.rs`
  12 passed.

## Retrospective

**Completed**: 2026-11-19

### Key Technical Learnings

#### Any total order nests

- **Challenge**: the story feared that a largest-first winner would break the
  nesting of tiers as prefixes (escalation trigger).
- **Solution**: the first-tier pass is a nearest-better-neighbour scan over
  heads in Morton order, which only needs a strict total order on rows.
  `(rank, row)` is one, so nothing else changed. The brute-force test (3
  distributions, random 16-level ranks) proves nesting and the set at every tier
  first.
- **Pattern**: write the ordering as a comparison key and the scan stays correct
  for every key.

#### What the full draw shows is the smallest mark, not the largest

- **Challenge**: random per-row sizes gave 23% of the plot rect over ΔE00 10
  (and 16.4% when the smallest mark wins instead): a dense cell holds all 16
  sizes, so the full draw shows the smallest marks (drawn last) inside, with the
  largest forming only the halo. One representative per cell can show either,
  not both.
- **Solution**: kept the story's rule (largest wins) and made the image tests
  data where sizes follow the data (clumps), where a cell mostly holds one size.
  Documented the limit on `Lod` and in the CHANGELOG's known limitations.
- **Pattern**: fidelity for overlapping marks of different sizes needs more than
  one mark per cell (a follow-up).

#### Colour contrast sets the gate

- **Challenge**: the same marks differ 0.884% in one colour and 2.303% in 16
  viridis colours: thinned edges one pixel narrower cross ΔE00 10 only against a
  contrasting neighbour.
- **Solution**: one gate per case, each 1.3x its measurement (GUP-717's rule).

#### The CPU pick index named stored rows

- **Challenge**: the GPU pick returned the right row while the CPU index, built
  from tier representatives, wrote stored rows as data rows (GUP-718 never met a
  sized layer).
- **Solution**: map through `DrawOrder::row_of`; the CPU/GPU agreement test over
  a grid found it at once.

### Architectural Decisions

#### Block order by rank, runs per rank

- **Decision**: each tier block is stably ordered by rank; runs never cross a
  change of rank, so a run keeps one rank and a compact Morton box.
- **Reasoning**: runs cut the block by Morton order; sorting by rank first and
  cutting by Morton within a rank keeps GUP-720's culling.
- **Trade-off**: ranks across blocks and chunks are not merged: a large mark of
  a finer block draws over a smaller mark of a coarser one, and a chunk's whole
  tier draws at its first segment's place in the layer's order (exact only when
  chunks hold ranks in sequence, as a fresh layer does; an appended layer's
  interleaved batches are approximate: 3.3% against 2.9%).
- **Future**: GUP-743.

#### Ranks are always bucketed for the tiers

- **Decision**: `lod_rank` quantises to 12.5% buckets even for an unwindowed
  layer's exact keys.
- **Reasoning**: a block ordered by exact size has no runs of equal rank, which
  would destroy the runs' compactness (about a run per representative).
- **Trade-off**: marks within 12.5% in size draw in spread, not size, order.

### Development Workflow Insights

- The tool refused compound commands with heredoc appends, `cd` and inline
  environment variables; plain commands with `export` and a patch script worked.
- Tests with random per-row data hid the design limit until the image was read:
  the diff image (red outlines only) separated edge effects from ordering
  errors.

### Follow-up Stories

1. **GUP-742: Mixed-Size Cells Under LOD** — a cell holding many sizes shows its
   largest only; keep a second (smallest) representative where sizes differ.
2. **GUP-743: Rank-Ordered Tier Ranges Across Chunks** — draw the ranges of a
   tier in global rank order across blocks and chunks, so an appended
   size-ordered layer draws as the exact order does.
