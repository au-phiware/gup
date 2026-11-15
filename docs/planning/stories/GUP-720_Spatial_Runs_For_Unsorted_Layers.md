---
implementer: opus
implementer-reason:
  changes the layout of every tier block and the culling granularity, with a
  measured target on the worst case (unsorted data); no precedent in the
  codebase
touches:
  [
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/cull.rs,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/render.rs,
    crates/gup-core/examples/lod_spike.rs,
  ]
---

# GUP-720: Spatial Runs for Unsorted Layers

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-11

## Context

LOD's prefix spans a whole chunk, so it relies on chunk culling (GUP-457) to
skip what is off screen. When every chunk spans the whole extent (unsorted
data), no chunk is culled and a zoom draws every chunk's off-screen
representatives. The spike measured it on the shuffled 10M layer: B takes 7.3 ms
in the full view but **65.4 ms in the zoom view** (4.76M representatives, drawn
from a pyramid whose tier 13 holds 4.76M of them), against 229 / 138 ms for the
baseline (RFC-001 "GUP-631 findings", "Chunk culling is unchanged..."). The
finding's fix: split each tier block into Morton-ordered runs of a few thousand
representatives with their own boxes, culled per frame.

GUP-714 stores each tier block in row order (the spike's order); this story
reorders each block to Morton order and adds the runs.

## Read First

- RFC-001 "GUP-631 findings": "Interaction under LOD" (Chunk culling), the
  shuffled numbers in "Fidelity", `--shuffle` in `lod_spike.rs`.
- `crates/gup-core/src/lod.rs` (GUP-714's `Tiers`) and GUP-715/716's draw
  entries.
- `crates/gup-core/src/cull.rs` (the chunk culling rule, `pick_ranges`) and
  `crates/gup-core/src/order.rs` (`draw_entries`, `DrawEntry`).
- GUP-457's culling findings (RFC-001 "S9f findings", lines 3575-3606).

## User Story

> "As a developer plotting an unsorted 10M-row scatter, I want a zoom to cost
> what its visible region costs, so that zooming into unsorted data stays
> interactive."

## Acceptance Criteria

### AC1: Runs and boxes

- [ ] `Tiers` stores each tier's block (`order[counts[t-1]..counts[t]]`) in
      Morton order of the finest key (a change from row order: a stable counting
      sort by tier of the Morton-sorted sequence gives it for free), and a list
      of runs: consecutive slices of at most 4,096 representatives, each with
      its box (min and max x and y of its representatives, f64) and its first
      and one-past-last position in `order`. Prefix property unchanged: tier `t`
      is still `order[..counts[t]]` (test, with GUP-714's nesting test passing).
- [ ] A run's box contains all its representatives (test) and has at most `64`
      cells of the tier's grid in area on average (a Morton run of 4,096
      representatives is compact: assert the median run's box covers under 3% of
      the chunk's extent for the shuffled layer).

### AC2: Per-frame culling of runs

- [ ] For a tier prefix `[0, count)` the draw entries are the runs of every tier
      block that meet the plot rect grown by the mark's radius (as the chunk
      rule does), merged when adjacent into one instanced range: a draw per
      merged range, `first` set to the range's first instance
      (`DrawEntry::first`), the cap of 64 ranges per chunk per frame (further
      ranges merged across gaps, drawing a few extra instances).
      `Prepared::draw_calls()` for the shuffled 10M layer's zoom view is at most
      `chunks x 64`.
- [ ] A sorted layer's draws are unchanged: its chunks are culled whole first
      and the surviving chunks' runs all meet the rect, so entries merge to one
      range per chunk (draw calls equal GUP-716's, asserted on the 1M sorted
      scatter).

### AC3: The measured target

- [ ] `lod_spike --rows 10000000 --shuffle` and a library run
      (`zoom_bench --lod auto --shuffle`, adding `--shuffle` if absent, 10M
      `Columns`) in release on the development machine, load under 2: shuffled
      zoom view GPU pass median **under 16.7 ms** (spike: 65.4), drawn instances
      under 500,000, and the full view not above 10 ms (spike: 7.3). Printed in
      the retrospective.
- [ ] Fidelity unchanged: the shuffled layer's ΔE00 share against `Lod::Off`
      (GUP-717's test pattern at 1M rows, 400 x 300, shuffled) is at most 1.0%
      of plot-rect pixels in the full view and 2.0% in a 10x zoom (spike: 0.34%
      / 0.18% at 10M).
- [ ] Image order effect: the order of drawn marks changes (Morton within a tier
      instead of row order); the retrospective states whether translucent marks'
      colours changed and by how much at ΔE00 on the 1M golden of GUP-717
      (re-bless only if the golden still passes its own fidelity threshold,
      listing each).

## Decisions

- **Run size 4,096**, the finding's "a few thousand": small enough that a zoom
  view culls most runs, large enough that 4.76M representatives are 1,200 runs a
  chunk.
- **At most 64 ranges per chunk per frame**, merged across gaps beyond that, so
  a frame's draw calls stay bounded: extra instances are cheap, extra draws are
  not (GUP-492's finding on draw counts).
- **Runs are per tier block, not per prefix**: a block's runs are independent of
  which tier is selected, so no rebuild per view.
- **Boxes in f64 in data units**, tested as the chunk box is.
- **The Morton order within a block replaces row order**; it is required for
  compact runs and changes draw order only among representatives of a tier.

## Escalation Triggers

- The shuffled zoom view stays above 16.7 ms with 64 ranges: report the
  instances drawn per run; do not raise the cap without reporting.
- Morton blocks change the picture of a layer with translucent marks beyond the
  fidelity thresholds.
- Merged ranges across tier blocks break the prefix draw (the prefix and the
  runs disagree on a chunk's draw count).

## Out of Scope

- A GPU sort or GPU culling pass (the spike's next steps); size-ordered layers'
  block order (GUP-721); appends (GUP-719) beyond not breaking them (a rebuild
  rebuilds runs).

## Technical Tasks

- [ ] Morton order within blocks, runs and boxes in `Tiers::build`.
- [ ] Run culling in the draw-entry selection; range merging and the cap.
- [ ] `--shuffle` in `zoom_bench` if absent; the 10M measurements.
- [ ] Tests (AC1, AC2, fidelity); update GUP-714/715 test expectations that
      assumed row order only if the compiler or tests say so.

## Dependencies

### Prerequisite Stories

- GUP-716: `Selection::lod` and Per-Frame Tier Draws 📋.
- GUP-719: LOD With Appends and Windows 📋 — sequencing only (both edit `lod.rs`
  and the selection path); a tail rebuild also rebuilds its runs.

### Enables Stories

- GUP-721 reorders blocks by size bucket, so it follows this.

## Testing Strategy

- **Unit tests**: block order and runs; box containment; culling picks the runs
  that meet a rect against brute force.
- **Integration tests**: the shuffled 1M fidelity pair; draw calls.
- **Visual validation**: shuffled 1M zoom PNG beside `Off`, read by eye.
- **Performance**: AC3's runs, three medians.

## Success Metrics

- [ ] Shuffled 10M zoom GPU pass from 65.4 ms to under 16.7 ms.

## Risk Assessment

- **High**: a 10M shuffled layer has one chunk's representatives spanning the
  whole extent in every cell (10 per cell in the spike), so zoom may keep many
  instances inside the viewport regardless of runs. _Mitigation_: the instances
  bound of AC3 (under 500,000) is checked first; the escalation trigger reports
  it.
- **Medium**: the draw-call cap trades draws for instances. _Mitigation_:
  reported.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the shuffled zoom PNG)
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm scene changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Blocked (2026-11-16)

**Question**: AC3's shuffled targets cannot be met by runs alone under
`Lod::Auto`, and the Decision "Morton order within a block replaces row order"
slows dense views by 46%. Which of the options below should the story take?

**What is built** (commits `84c7417a`, `2204ea47` and the WIP commit after
them): Morton-ordered tier blocks (AC1, tests pass), runs of at most 4,096 with
exact f64 boxes, per-frame run culling through the encodings' images with the
chunk rule's margin, adjacent runs merged, at most 64 ranges a chunk (AC2; the
spans carry the ranges, so draws, GPU picks, the CPU pick index (GUP-718) and
SVG all read the drawn runs only). `zoom_bench --shuffle` added. `tests/lod.rs`
passes except the provisional bound of the new shuffled-zoom test (28.5% of the
prefix drawn at 400 x 300; culled and unculled images are byte-identical, the
brute-force check).

**Evidence** (release, HD 630, headless 1920 x 1080,
`lod_spike --rows 10000000 --shuffle`, same session, main's
`crates/gup-core/src` built into the same worktree for "before", load 1.0-1.7,
7.5-8.9 GB available):

| run                                           | before (main)         | after (branch)          |
| --------------------------------------------- | --------------------- | ----------------------- |
| `auto` full, GPU median ms (3 runs each)      | 13.92 / 13.92 / 13.92 | 20.20 / 20.30 / 20.37   |
| `auto` zoom, GPU median ms (drawn)            | 135.35 (10,000,000)   | 134.77 (10,000,000)     |
| forced tier 13 (`indexed --lod-tier 13`) zoom | 52.82 (prefix 3.92M)  | 16.46 (1,143,533 drawn) |
| forced tier 13 full                           | 86.70                 | 202.99-208.24           |
| forced tier 12 zoom                           | 21.78 (prefix 1.63M)  | 5.77 (399,467 drawn)    |
| sorted `auto` full / zoom (2 runs)            | 3.21-3.38 / 4.16-4.20 | 2.86-2.92 / 3.27-3.95   |

1. **The zoom draws every row under `Auto`, before and after.** Each shuffled
   1M-row chunk spans the whole extent; `lod::tier` picks 14 (the spike's tier
   13 plus `LOD_TIER_BIAS`), but tier 14 holds more than half the chunk's rows,
   so it is not stored and the chunk draws every row (GUP-716's "past the stored
   tiers"). Runs cull only tier prefixes. The spike's 65.4 ms was its own B at
   tier 13, not the library's `Auto`.
2. **Morton order slows dense views.** Consecutive instances now land on
   neighbouring pixels (RFC-001 "GUP-631 findings": sorted x costs 934 ms
   against 229 shuffled). `auto` full: 13.92 -> 20.30 ms; forced tier 13 full:
   86.7 -> 208 ms. Sorted layers are not slower.
3. **AC3's full view (at most 10 ms) fails on main already**: 13.92 ms, because
   `Auto` draws 629,602 representatives (bias 1) against the spike's 327,065.

**Prototype** (env-gated, measured, reverted; patch kept at
`~/.cache/gup/gup720/spread_and_cap.patch`, outside the repository):

- **Spread within runs**: keep each run's set (and box) but draw its
  representatives in bit-reversed order of their Morton position. `auto` full
  20.30 -> 13.89 / 13.90 ms (main's); forced tier 13 full 208 -> 86 ms, zoom
  16.47 -> 15.80 ms. Culling is unchanged (runs are the same sets).
- **Cap**: when `lod::tier` is one past the finest stored tier
  (`t <= stored + 1`), draw the finest stored tier instead of every row. With
  the spread, `auto` shuffled zoom: **15.86 ms**, 1,143,533 instances (562,816
  of them the partial last chunk, which stores only coarser tiers and still
  draws every row), ΔE00 > 10 against `base`: **0.306%** of the plot rect (10
  frames; PNG read by eye: the walk, no gaps).

**Options**:

- **A (recommended)**: spread within runs (amends AC1's "block in Morton order"
  to "runs are Morton stretches; draw order within a run is bit-reversed") and
  the one-tier cap (amends GUP-716's "past the stored tiers draws every row" for
  the bias tier only). Meets 16.7 ms zoom; full view back to main's 13.9 ms;
  instances 1.14M (over AC3's 500,000 because of the partial chunk); the full
  view stays over 10 ms (a bias question, GUP-716/717's). Needs fidelity gates
  re-measured at 400 x 300 for the cap.
- **B**: spread only, no cap: no regression, runs help forced tiers and any view
  whose tier is stored, but the shuffled 10M `Auto` zoom stays at 135 ms (AC3
  not met; a follow-up decides the tier/storage rule).
- **C**: store all rows in Morton runs (a full Morton gather, up to 12 B a row
  of extra GPU memory, twice GUP-715's bound) so a chunk past its stored tiers
  also draws only visible runs; fixes the partial chunk too. Largest change.
- **D**: keep Morton order as decided (the full-view regression stays) and relax
  AC3.
