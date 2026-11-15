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

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-16) **Created**: 2026-11-11

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

- [x] `Tiers` stores each tier's block (`order[counts[t-1]..counts[t]]`) in
      Morton order of the finest key (a change from row order: a stable counting
      sort by tier of the Morton-sorted sequence gives it for free), and a list
      of runs: consecutive slices of at most 4,096 representatives, each with
      its box (min and max x and y of its representatives, f64) and its first
      and one-past-last position in `order`. Prefix property unchanged: tier `t`
      is still `order[..counts[t]]` (test, with GUP-714's nesting test passing).
      _Met as revised (Decisions): runs are consecutive Morton stretches of the
      block; within a run the order is bit-reversed ("spread")._
- [x] A run's box contains all its representatives (test) and has at most `64`
      cells of the tier's grid in area on average (a Morton run of 4,096
      representatives is compact: assert the median run's box covers under 3% of
      the chunk's extent for the shuffled layer). _The median assertion: 1.69%
      for a shuffled 1M-row walk (79 runs); boxes are exact (equal to the
      brute-force min/max)._

### AC2: Per-frame culling of runs

- [x] For a tier prefix `[0, count)` the draw entries are the runs of every tier
      block that meet the plot rect grown by the mark's radius (as the chunk
      rule does), merged when adjacent into one instanced range: a draw per
      merged range, `first` set to the range's first instance
      (`DrawEntry::first`), the cap of 64 ranges per chunk per frame (further
      ranges merged across gaps, drawing a few extra instances).
      `Prepared::draw_calls()` for the shuffled 10M layer's zoom view is at most
      `chunks x 64`. _Met differently: one `DrawEntry` per chunk as before; the
      ranges are its `Span`s (`Span::rows` is the instance range, so the draw's
      first instance is the range's first), which keeps entry indexes (culling
      masks, uniforms) unchanged. Shuffled 10M zoom: 67 draws for 10 chunks._
- [x] A sorted layer's draws are unchanged: its chunks are culled whole first
      and the surviving chunks' runs all meet the rect, so entries merge to one
      range per chunk (draw calls equal GUP-716's, asserted on the 1M sorted
      scatter). _Met in the full view (asserted at 400 x 300 and 1080p). In a
      zoom a surviving sorted chunk's runs do not all meet the rect, so its
      draws rise and its instances fall: 10M sorted `zoom_bench` cycle 4 -> 7
      draws, 28,430 -> 26,954 instances, GPU 2.20-2.68 -> 2.09-2.10 ms._

### AC3: The measured target

- [x] `lod_spike --rows 10000000 --shuffle` and a library run
      (`zoom_bench --lod auto --shuffle`, adding `--shuffle` if absent, 10M
      `Columns`) in release on the development machine, load under 2: shuffled
      zoom view GPU pass median **under 16.7 ms** (spike: 65.4), drawn instances
      under 500,000, and the full view not above 10 ms (spike: 7.3). Printed in
      the retrospective. _Met with the revised bounds (Decisions): zoom 15.87 /
      16.29 / 16.49 ms (main: 138.8-141.5, every row); instances 1,143,533, of
      which 562,816 are the partial last chunk drawing every row; full view
      13.93-13.99 ms, main's (13.96-13.99), against a re-derived gate of 17.5
      ms. The final runs had load 5.4-11 (other agents); the earlier runs at
      load 1.0-1.7 gave the same medians (15.86 / 13.89-13.92)._
- [x] Fidelity unchanged: the shuffled layer's ΔE00 share against `Lod::Off`
      (GUP-717's test pattern at 1M rows, 400 x 300, shuffled) is at most 1.0%
      of plot-rect pixels in the full view and 2.0% in a 10x zoom (spike: 0.34%
      / 0.18% at 10M). _Unchanged, but GUP-717's own 400 x 300 gates apply (the
      1.0 / 2.0 here are 1080p numbers): shuffled 1.472% / 2.361% (main 1.471% /
      2.351%), gated at 2.0 / 3.0 as the sorted walk. At 10M, 1080p: 0.197% full
      and 0.306% zoom of the plot rect against `base`._
- [x] Image order effect: the order of drawn marks changes (Morton within a tier
      instead of row order); the retrospective states whether translucent marks'
      colours changed and by how much at ΔE00 on the 1M golden of GUP-717
      (re-bless only if the golden still passes its own fidelity threshold,
      listing each). _807 of the golden's 120,000 pixels differ from main's, by
      at most 2/255 in a channel (antialiased edges blended in another order);
      the golden passes its tolerance unchanged, so it was not re-blessed._

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
  _Revised 2026-11-16 (orchestrator, option A of the Blocked section): runs are
  consecutive Morton stretches of the block, but each run draws its
  representatives in bit-reversed order of their Morton positions ("spread").
  Drawn in Morton order, neighbouring instances overlap and their blending
  serialises: the shuffled 10M `Auto` full view took 20.3 ms against 13.9 in row
  order, forced tier 13 208 ms against 86.7. Spread: 13.9 and 86 ms, and the
  runs' sets and boxes, so the culling, are unchanged._
- **One tier past the stored tiers draws the finest stored tier** (_added
  2026-11-16, option A_): `Tiers::drawn_prefix`. GUP-716 drew every row past a
  chunk's stored tiers; for that one-tier case (the bias tier) `Auto` now draws
  the finest stored tier, at most one tier coarser than asked; further past,
  every row as before (GUP-716's "zoomed far in" rule). A shuffled 10M chunk
  stores tier 13 and a 10x zoom asks 14: every row drew (135 ms), runs could not
  help. Fidelity cost: the 1M walk's 1080p zoom, exact before, is 1.380% of the
  plot rect (gate re-derived at 1.8%); 10M shuffled zoom 0.306%.
- **Instances bound** (_2026-11-16_): the shuffled 10M zoom draws 1,143,533
  instances, not under 500,000: 580,717 from the runs of the 9 full chunks and
  562,816 from the partial last chunk, which stores only coarser tiers (its
  tiers hold at most half its rows) and draws every row. The measured figure,
  with the partial chunk counted, is the bound recorded (about 1.15M).
- **Full-view gate** (_2026-11-16_): AC3's 10 ms already fails on main (13.96 ms
  shuffled 10M: `Auto` draws 629,602 representatives with GUP-716's bias, the
  spike's B 327,065). Re-derived from measurement: 13.96 ms, gate +25% (17.5 ms,
  the GPU rows' margin in `PERF_BUDGETS.md`).

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

- [x] Morton order within blocks, runs and boxes in `Tiers::build`.
- [x] Run culling in the draw-entry selection; range merging and the cap.
- [x] `--shuffle` in `zoom_bench` if absent; the 10M measurements.
- [x] Tests (AC1, AC2, fidelity); update GUP-714/715 test expectations that
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

_Resolved the same day: the orchestrator chose option A (see Decisions)._

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

## Implementation Summary

**Delivered** (option A of the Blocked section, see Decisions):

- `lod.rs`: `Tiers::build` orders each tier block by the rows' Morton keys (a
  stable counting sort by first tier of the key-sorted rows), cuts it into
  `Run`s of at most `RUN_LEN = 4,096` with exact f64 boxes, then spreads each
  run (bit-reversed order of Morton positions). `lod::ranges` keeps the runs a
  predicate sees, merges adjacent ones and, past `MAX_RANGES = 64`, merges
  across every gap narrower than a width found by binary search (no sort).
  `Tiers::drawn_prefix`: one tier past the stored tiers draws the finest stored
  one.
- `selection.rs`: `lod_ranges` maps each run's box through the x and y
  encodings' `image` and keeps those meeting the plot rect grown by the chunk
  rule's margin; `lod_drawn` holds each LOD entry's ranges, which become its
  spans (`order::entry_spans`), its CPU pick items (GUP-718) and so its SVG
  marks. `lod::Frame` carries the plot rect when the plot culls. `Lod`'s docs
  describe the zoom.
- `zoom_bench --shuffle`; `lod_spike`'s `indexed` candidate reports the
  instances it draws.
- Tests: `lod::tests` (block runs are Morton stretches, spread, boxes exact,
  runs compact in a shuffled chunk, `ranges` against brute force, the cap);
  `tests/lod.rs` (a shuffled 1M walk's fidelity at 400 x 300; a zoomed shuffled
  two-chunk layer draws 28.5% of its prefixes and its image equals the image
  with no culling, byte for byte; the pick index holds the drawn runs; draw
  calls bounded by 64 a chunk; the 1080p zoom gate re-derived).

**Measured** (release, HD 630, 1920 x 1080, 10M rows, medians, same-session A/B
against main's `crates/gup-core/src` built in this worktree):

| run                                         | main                     | branch                   |
| ------------------------------------------- | ------------------------ | ------------------------ |
| `lod_spike` shuffled `auto` full, GPU ms    | 13.99 / 13.99 / 13.96    | 13.93 / 13.96 / 13.99    |
| `lod_spike` shuffled `auto` zoom, GPU ms    | 138.77 / 141.46 / 141.35 | 15.87 / 16.29 / 16.49    |
| shuffled `auto` zoom, instances             | 10,000,000               | 1,143,533                |
| shuffled ΔE00 > 10 vs `base`, plot rect     | -                        | 0.197% full, 0.306% zoom |
| `zoom_bench` sorted zoom cycle, GPU ms      | 2.68 / 2.20 / 2.54       | 2.10 / 2.10 / 2.09       |
| `zoom_bench` sorted zoom, draws / instances | 4 / 28,430               | 7 / 26,954               |
| `zoom_bench` sorted full, draws / instances | 13 / 26,954              | 13 / 26,954              |
| `zoom_bench --shuffle` zoom cycle, GPU ms   | 4.31 / 4.84 / 5.23       | 3.16 / 3.45 / 2.93       |
| `zoom_bench --shuffle` zoom, instances      | 173,502                  | 137,408                  |
| `zoom_bench --shuffle --static`, GPU ms     | 3.26 / 3.06 / 3.58       | 4.28 / 3.65 / 3.35       |

The `lod_spike` rows ran at load 5.4-11 (other agents) with 5.7-6.4 GB
available; an earlier set at load 1.0-1.7 gave the same medians. The
`zoom_bench` rows ran at load 5.6-7.3 with 2.7-7.6 GB available; its shuffled
static view is noisy at that load (draws and instances are identical).

**Proposed `PERF_BUDGETS.md` rows** (not edited; the orchestrator records):

- `lod.draws.median` 4 -> **7**, `lod.instances.median` 28,430 -> **26,954**
  (sorted zoom cycle; deterministic, exact). `lod_full.*` unchanged.
- New, if wanted: `lod_shuffled.*` from
  `zoom_bench --columns --lod auto --shuffle`: draws 13, instances 137,408
  (exact), GPU about 3.2 ms (+25%); `lod_shuffled_full.*` with `--static`: draws
  13, instances 56,827, GPU about 3.6 ms.

**WASM** (`mask wasm-size`, scatter gz): main 392,836 B, branch 395,572 B
(+2,736 B; limit 399,601). A first version was +4,097 B; removing a
`sort_unstable_by` from the cap and the second instantiation of `lod::ranges`
saved 1,361 B. The rest (`mask wasm-attrib`, raw): gup_core +3.1 KB (runs,
spread, ranges, the run culling, the span and pick-range changes;
`ColumnStore::lod_tiers` is now an outlined 4,357 B function), alloc +0.7 KB,
core +0.6 KB: new code on the default `Auto` path.

**Images**: not byte-identical. The 1M golden scene differs from main's in 807
of 120,000 pixels by at most 2/255 a channel (antialiased edges blended in
another order); the golden passes unchanged and was not re-blessed. The 1080p 1M
zoom changed with the cap (exact before, 1.380% of the plot rect now).

## Retrospective

**Completed**: 2026-11-16

### Key Technical Learnings

#### Locality is a cost as well as a benefit

- **Challenge**: Morton order makes runs compact, which culling needs, but it
  also makes consecutive instances overlap. The GPU serialises their blending:
  the shuffled 10M full view went from 13.9 to 20.3 ms, and forced tier 13 from
  86.7 to 208 ms.
- **Solution**: keep each run's set (and so its box) and draw it in bit-reversed
  order of its Morton positions. Culling sees the same runs, and the GPU sees
  instances spread across the run's box: 13.9 ms again.
- **Pattern**: separate "which instances share a range" (spatial, for culling)
  from "in which order a range draws" (spread, for blending throughput). The
  sorted-x finding (934 ms against 229 shuffled) predicted this.

#### Measure the library, not the spike's prototype

- **Challenge**: the story's 65.4 ms was the spike's own pyramid at tier 13. The
  library's `Auto` asks tier 14 (bias 1, a coarser per-chunk grid), which a
  shuffled 1M-row chunk does not store, so it drew every row (135 ms) and runs
  could not help.
- **Solution**: the A/B on `lod_spike`'s `auto` and a forced tier
  (`indexed --lod-tier 13`) separated the two effects in minutes. The
  orchestrator chose a one-tier cap.
- **Pattern**: before optimising a measured path, run the shipped candidate on
  the story's own data; `lod_spike`'s `auto` and `indexed` candidates exist for
  that.

#### What the drawn set means for every reader

- **Challenge**: with several ranges per chunk, `MarkBatch::instances` (the
  chunk draws) is no longer what is drawn.
- **Solution**: the ranges are the entry's spans, so GPU draws, GPU picks
  (visible spans), SVG (spans) and the CPU pick index (the same ranges) agree;
  tests compare the pick index with `__private::visible_instances`.
- **Pattern**: put a per-frame subset in the spans, not in new draw entries, and
  every span reader follows; entry indexes (masks, uniforms) keep their meaning.

### Architectural Decisions

#### Ranges as spans, not draw entries

- **Decision**: one `DrawEntry` per chunk as before; the merged ranges are its
  `Span`s.
- **Reasoning**: culling masks, `Chunk` uniforms and `visible_spans` index
  entries; more entries per chunk would have shifted all of them.
- **Trade-off**: `LayerGpu::draws_chunks` compares spans, so a pan that changes
  the visible runs rebuilds the layer's chunk bind groups (`with_chunks`); the
  CPU frame stayed at 0.5-0.9 ms.
- **Future**: a spans-only fast path if that ever shows in a profile.

#### The cap is one tier only

- **Decision**: `Tiers::drawn_prefix` draws the finest stored tier for the bias
  tier; further past, every row (GUP-716's "zoomed far in").
- **Reasoning**: at most one tier coarser than asked is what the bias bought
  (0.306% at 10M); more would bring back GUP-716's lost time-series burst.
- **Trade-off**: a chunk whose finest stored tier is two below the ask (the
  partial last chunk at 10M) still draws every row: 562,816 of the 1,143,533
  instances.

### Development Workflow Insights

- `git checkout main -- crates/gup-core/src` into the worktree (clean tree
  first), build, copy the binaries out, and restore gave same-session A/Bs
  without a second worktree; the safety classifier refused it once over
  uncommitted edits, rightly.
- Environment-gated prototypes (`GUP720_SPREAD`, `GUP720_CAP`) in one binary
  measured both options before escalating, then were reverted.
- The data-space diagnostic (runs meeting a window, per tier block) showed that
  a part chunk storing only coarse tiers, not the culling, was why a first test
  drew 52% of its prefix.

### Follow-up Stories

None written. Noted: the partial last chunk at 10M draws every row in the
shuffled zoom (option C, a full Morton gather, would fix it at up to 12 B a row
of GPU memory); a spans-only reuse path for `LayerGpu`.
