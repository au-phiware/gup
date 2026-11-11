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

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-11

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

- [ ] `Tiers::build` takes an optional per-row key (the mark's draw-order key,
      GUP-492's bucketed `f32` key) and, within a Morton run of equal cells,
      makes the representative the row with the largest key, ties to the lowest
      index. A brute-force test (as GUP-714's AC1) with random sizes: for every
      tier, `order[..counts[t]]` is the set of largest-key rows per occupied
      cell. Tiers still nest (the representative of a finer cell inside a
      coarser cell's winner is that winner or another cell's winner: assert
      nesting holds, and if the first-tier rule cannot keep nesting with a
      non-index winner, STOP: Escalation Triggers).

### AC2: Largest first within the prefix

- [ ] Each tier block is ordered by size bucket descending, then by the block's
      existing order; `order[..counts[t]]` is still tier `t`, but drawn
      first-to-last the larger buckets come first within each block. A test with
      sizes in 16 levels: the sequence of drawn bucket keys over a tier prefix
      is non-increasing per block (assert per block).
- [ ] An image test: 300,000 rows, 16 size levels drawn at random, overlapping
      marks, 400 x 300: `Auto` against `Off`, share of pixels at ΔE00 > 10 at
      most 2.0% of the plot rect; the retrospective says what the PNG shows
      (large bubbles beneath, small on top, none hidden by a thinned neighbour).

### AC3: Pick and counts

- [ ] GPU pick over a size-ordered LOD layer returns the data row of the
      representative (row id through `DrawOrder::row_of` as for a non-LOD
      size-ordered layer); a test at a bubble's pixel.
- [ ] A size-ordered layer under `Lod::Auto` above 200,000 rows now draws
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

- [ ] `Tiers::build` winner by key; block order by bucket.
- [ ] Remove GUP-716's size-order opt-out; wire the key from `DrawOrder`.
- [ ] Tests (AC1-AC3) and the PNG read.

## Dependencies

### Prerequisite Stories

- GUP-492: Windowed Size-Ordered Draws 📋 — the bucket keys.
- GUP-716: `Selection::lod` and Per-Frame Tier Draws 📋.
- GUP-720: Spatial Runs for Unsorted Layers 📋 — block order and runs
  (sequencing: both edit `lod.rs`).

### Enables Stories

- None scheduled.

## Testing Strategy

- **Unit tests**: brute-force winners; block order; nesting.
- **Integration tests**: image parity and pick.
- **Visual validation**: the bubble PNG pair read by eye.

## Success Metrics

- [ ] A bubble layer of 10M rows draws under 10% of its rows with larger bubbles
      beneath smaller, at most 2.0% of plot-rect pixels differing.

## Risk Assessment

- **Medium**: the winner rule may break nesting (Escalation Triggers).
  _Mitigation_: the brute-force nesting test comes first.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the bubble PNG pair)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm scene changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
