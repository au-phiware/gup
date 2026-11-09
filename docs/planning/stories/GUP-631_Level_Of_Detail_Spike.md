---
implementer: opus
implementer-reason:
  a spike whose result decides the LOD design and the stories after it; GPU
  numerical work with only parked precedent (gup-culling-lod)
touches:
  [
    crates/gup-core/examples/lod_spike.rs,
    docs/planning/rfcs/RFC-001_Core_Architecture.md,
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/render.rs,
  ]
---

# GUP-631: Level of Detail Spike

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: ✅ Complete
(2026-11-09) **Created**: 2026-11-04

## Context

Drawing every row does not scale past what the GPU can rasterise per frame: 4M
sorted rows take 9.9 ms of GPU pass on the development machine with chunk
culling and 58.7 ms without (RFC-001 "S9f findings", lines 3575–3606), so 100M
rows in full view would take on the order of a second per frame. The review's
track N5 asks to "port LOD and density aggregation from `gup-culling-lod`"
before Gup may claim 100M or a billion points.

`crates/gup-culling-lod` (parked, out of the workspace, built against the old
core) holds a grid-aggregated LOD pyramid with tier selection (`src/lod/mod.rs`,
`src/shaders/lod_aggregate.compute.wgsl`, `LOD_SYSTEM.md`), streaming LOD with a
memory budget, GPU culling and a radix sort. Its pyramid keeps one
representative point per grid cell per tier, chosen on the GPU. `gup-core`'s
chunks (`ColumnStore`, `F32Relative` columns, per-chunk f64 stats) are the
natural unit for any LOD built now (RFC-001 §3: "Chunks are also the future unit
for LOD tiers").

There are three candidate designs, and which one is right depends on numbers
nobody has measured on this core. This story measures them and decides by the
rule below. It does not ship the feature; it writes the findings and the list of
stories that build it.

## Read First

- GUP-627's and GUP-628's retrospectives (ingest numbers; columns).
- RFC-001 §3 (lines 149–258), "S9f findings" (lines 3575–3606), "S12 findings"
  (lines 3671–3745).
- `crates/gup-culling-lod/LOD_SYSTEM.md`, `src/lod/mod.rs` (the pyramid, lines
  1–300) and `src/shaders/lod_aggregate.compute.wgsl`.
- `crates/gup-core/src/order.rs` (`draw_entries`, the culling rule) and
  `crates/gup-core/src/render.rs` (`Prepared::draw`).
- `crates/gup-core/PERF_BUDGETS.md` lines 20–60 (the machine).

## User Story

> "As the maintainer, I want the level-of-detail design chosen from measurements
> on the current core, so that the 100M-row work builds the right thing once."

## Acceptance Criteria

### AC1: Prototypes

- [x] `crates/gup-core/examples/lod_spike.rs` builds a 100M-row scatter from
      `Columns` (GUP-628; x sorted, y a random walk; at 30M if 100M does not fit
      the machine, saying so) and measures, at 1920 × 1080 on the development
      GPU, full view and a 10× zoom on x and y, each candidate of Decisions: GPU
      pass median and p95 (timestamp queries, as `zoom_bench`), CPU per frame,
      build time, extra GPU and CPU memory. _Measured at 10M, 30M and 80M
      (`--rows N`); 100M was not run: `MemAvailable` was 6.9-9.8 GB, under the
      20 GB the orchestrator required, and 80M was the largest size that fit._
- [x] The baseline (draw every visible row, today's path) at 10M and at the
      largest size that runs, for comparison.
- [x] Fidelity: for each candidate, the share of pixels whose colour differs by
      ΔE00 > 10 from the full draw at 10M rows in both views (the full draw is
      the reference; use `gup_visual_regression`'s colour functions).
- [x] Library seams needed by the prototypes are `#[doc(hidden)] pub` items
      listed in the findings; they are kept only if the chosen design needs
      them, else removed before the story closes. _None was needed: the library
      is unchanged._

### AC2: The decision

- [x] The rule of Decisions is applied to AC1's numbers and the outcome is
      recorded with the table.

### AC3: Findings and follow-ups

- [x] A "GUP-631 findings" section appended to RFC-001: the candidates, the
      table, the fidelity numbers, the decision, the public API proposed (for
      example a `Selection::lod(..)` setter or an automatic threshold), what
      happens on hover, pick, brush, SVG and tooltips under LOD, and how it
      interacts with `Window::LastRows`, appends and chunk culling.
- [x] A "Follow-up Stories" section in this story file listing the stories to
      write (title, scope, rough size, implementer tier), each sized at three
      hours or less, in order; the orchestrator has a scribe write them.

## Decisions

- **Candidates**:
  - **(A) Density per frame**: accumulate every visible row as a 1-px count into
    an `R32Float` (or `R32Uint` with atomics in a compute pass) target, then
    colour-map it with a `Sequential` scale in a full-screen pass. No
    precompute; cost grows with visible rows.
  - **(B) Decimation pyramid**: per chunk, tiers of representatives on a spatial
    grid (the `gup-culling-lod` scheme: one representative per cell, lowest
    index, `lod_aggregate.compute.wgsl` ported to `gup-core`'s shader
    composition), built at upload; each frame draws, per visible chunk, the
    finest tier whose on-screen density is at most one point per pixel.
  - **(C) Binned tiles**: per chunk, 2D count histograms at a few resolutions,
    built at upload, drawn as `Rect` cells coloured by count (a heatmap of the
    data), switching to the full draw when zoomed in far enough.
- **Rule** (budget: 16.7 ms GPU pass median at 1920 × 1080 on the development
  GPU):
  1. If the baseline meets the budget at the largest size, no LOD is needed at
     that size; record the size where it stops meeting it.
  2. Choose (B) if it meets the budget in both views, its build time is at most
     2 s per 10M rows, and its fidelity share is at most 1% at full view.
  3. Else choose (A) if it meets the budget in both views (density is then the
     documented look of a large scatter).
  4. Else choose (C) if it meets the budget; else record that no candidate meets
     it, with the closest, and propose what would (for example (B) with a
     coarser density target).
- **Above LOD's threshold only**: whatever is chosen applies only when the
  visible rows exceed the count at which the baseline meets 8 ms (measured in
  AC1); smaller charts draw exactly as today.

## Escalation Triggers

- None of the candidates can be prototyped without changing public API.
- The development machine cannot hold 30M rows in columns plus a candidate's
  structures.

## Out of Scope

- Shipping LOD (the follow-up stories); lines and areas (scatter only here);
  GUP-492's size-ordered draws.

## Technical Tasks

- [x] Port or write the three prototypes in `lod_spike.rs` with seams.
- [x] Measurements (AC1), the rule (AC2), findings and follow-ups (AC3).

## Dependencies

### Prerequisite Stories

- GUP-627: Scale Benchmark 📋 — the ingest baseline.
- GUP-628: Columnar Layers 📋 — 100M rows without a row type.

### Enables Stories

- The LOD implementation stories this spike lists; GUP-632.

## Testing Strategy

- **Performance**: AC1's runs, release, development machine, nothing else
  running.
- **Visual validation**: fidelity numbers, and the spike's PNGs of each
  candidate at both views read by eye and attached in the findings (paths).

## Success Metrics

- [x] A LOD design chosen by a stated rule from measured numbers, with stories
      to build it.

## Risk Assessment

- **High**: unknown GPU behaviour at 100M rows on an integrated GPU (memory,
  driver limits). _Mitigation_: the 30M fallback, and the rule records "no
  candidate meets the budget" rather than forcing a choice.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1` (through
      `mask merge-check`, all features: 953 passed, 0 failed, 8 ignored)
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples` (clippy over every target
      in `mask all-fix` and `mask all-check`)
- [x] Rendered output verified by eye: the candidates' PNGs (both views at 200K
      and 10M rows; base and B side by side, crops of the full view's band;
      described in RFC-001 "GUP-631 findings", Fidelity)
- [x] `mask wasm-browser`: not needed, a spike example (it passed in
      `mask merge-check`)
- [x] WASM size: `mask wasm-size` scatter gz bytes if any seam stays in the
      library, else "unchanged: no library code change" (unchanged: no library
      code change; 378,676 B gz in `mask merge-check`, a pass)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **`crates/gup-core/examples/lod_spike.rs`** (headless, release; no library
  change): the baseline and the three candidates over `scale_bench`'s sorted
  random walk from f64 `Columns`, at both views, with GPU pass timestamps, CPU
  frame, build time, the design's extra memory, the GPU clock per frame, PNGs
  (`--out`) and the ΔE00 fidelity share against the full draw (`--fidelity`).
  Options: `--rows`, `--candidates`, `--views`, `--frames`, `--warmup`,
  `--max-secs`, `--size`, `--radius`, `--shuffle`, `--pyramid-bias`, `--seed`.
- **RFC-001 "GUP-631 findings"**: the prototypes, the table at 10M, 30M and 80M,
  the baseline sweep (100K-1.6M), fidelity (with B one and two tiers finer), the
  decision, the proposed API, interaction under LOD (pick, hover, brush, SVG,
  tooltips, windows, appends, culling, GUP-492) and proposed adjustments.
- **Decision: (B), the decimation pyramid.** Base misses the budget at 80M
  (8,069 ms full, 283 ms zoom). B: 2.0-4.1 ms in both views at every size, build
  562-826 ms per 10M, fidelity 0.994% at full view (one tier finer: 0.580%). LOD
  applies above 200,000 visible rows (the baseline meets 8 ms at 200K, 7.67 ms,
  and not at 400K, 20.90 ms).

Measured on the Intel HD 630, release, medians of 3 same-process runs. Load
0.9-1.1 for the final 10M batch, 2-6 for 30M and 0.5-2.4 for 80M (other agents'
builds). 100M was not run (`MemAvailable` 6.9-9.8 GB < 20 GB).

## Follow-up Stories

In order; the orchestrator's scribe writes them. Each at most three hours.

1. **LOD tiers in the column store** (opus, ~3 h). Per chunk: Morton keys over
   an absolute power-of-two grid in data units (not the layer extent, so appends
   that grow it keep built chunks), one radix sort, the progressive order and
   prefix counts, tiers stored while at most half the chunk; built lazily per
   chunk, on the CPU. Tests: tiers nest, each representative is its cell's
   lowest index, prefix counts, grid invariance under a growing extent; build at
   most 650 ms per 10M rows on the development machine.
2. **Representative buffers and the indexed-rows glue** (opus, ~3 h). Per-chunk
   gathered columns of the stored representatives plus a `u32` chunk-local row
   column; a mark-glue variant that reads `row_base + local` for the brushed bit
   and the pick id (a shader-interface change). Tests: GPU pick and brush over
   representatives return the data rows.
3. **`Selection::lod(Lod)` and per-frame tier draws** (opus, ~3 h).
   `Lod::{Auto, Off}` (default `Auto`, prelude, `ScatterChart::lod`),
   `Lod::AUTO_MIN_ROWS = 200_000`, tier selection per visible chunk in prepare,
   draw entries over the representative buffers; decide the tier offset (the
   decision's tier or one finer, RFC-001 "GUP-631 findings"); a golden of a
   1M-row scatter under LOD with a ΔE00 share check; `zoom_bench --lod` and
   `lod.*` rows in `PERF_BUDGETS.md`.
4. **LOD in CPU pick, hover, tooltips and SVG** (sonnet, ~2-3 h). `hit_items`
   over the drawn representatives (a LOD scatter stays CPU-pickable above
   `Retain::AUTO_MAX_ROWS`), GUP-684's outline on the representative,
   `SvgTarget` writing the representatives of the view (`Lod::Off`: every row).
5. **LOD with appends and windows** (sonnet, ~2-3 h). The tail chunk draws in
   full below 65,536 rows, then rebuilds its tiers at doubling sizes;
   `Window::LastRows` eviction drops tiers with their chunks; a tier rebuild
   bumps the layer's pick epoch. Tests with a windowed stream.
6. **Spatial runs for unsorted layers** (opus, ~3 h). Each tier block in Morton
   order, split into runs of at most 4,096 representatives with boxes, culled
   per frame (draw ranges by first instance). Target: the shuffled 10M layer's
   zoom view from 65.4 ms to under 16.7 ms.
7. **LOD for size-ordered layers** (sonnet, ~2 h; after GUP-492). Tier blocks
   ordered by GUP-492's size buckets, largest first; a cell's representative is
   its largest mark, then lowest index. Tests: largest-first order under LOD.

Not proposed: a density or heatmap view (A or C) as a chart of its own; both are
a different look, and C met the budget everywhere if one is wanted.

## Retrospective

**Completed**: 2026-11-09

### Key Technical Learnings

#### Tiers as prefixes of one order

- **Challenge**: the `gup-culling-lod` pyramid stores each tier as its own
  buffer, built by a dense `atomicMin` grid per tier: at the tiers a 10× zoom
  needs (13), one grid is 256 MiB, and the tiers duplicate each other.
- **Solution**: one Morton sort per chunk gives every tier: a row first
  represents its cell at tier `16 - floor(h / 2)`, `h` the highest bit in which
  its key differs from its predecessor's. Ordering the representatives by that
  tier makes every tier a prefix, so a chunk's LOD draw is one instanced draw
  with a smaller instance count, and storage is at most the finest stored tier.
- **Pattern**: when levels nest, store one order and per-level counts rather
  than one copy per level.

#### What the baseline's cost depends on

- **Challenge**: the baseline's full-view cost grew faster than its rows (38 ns
  a row at 200K, 93 at 10M), and the shuffled layer was 4× faster than the
  sorted one (229 against 934 ms).
- **Solution**: sorted x sends consecutive instances to the same pixels, so
  blending serialises; the zoom view, whose rows spread out, costs 14-26 ns a
  row.
- **Pattern**: GPU time a row is not a constant; a threshold in rows must be
  measured on the densest view it applies to.

#### The GPU clock in comparisons

- **Challenge**: the i915 clock follows load: heavy candidates ran at 950-1,100
  MHz, light ones at the 350 MHz floor.
- **Solution**: sample `gt_act_freq_mhz` per frame and report it next to every
  number; the bias favours the slow candidates, so the decision is safe.

#### Contended atomics

- **Challenge**: (A) was expected to be cheap; it took 16-28 ns a row, because
  rows that land on the same pixel serialise their `atomicAdd`s, and the clear,
  maximum and composite passes over 2M pixels cost about 5 ms alone.
- **Pattern**: per-frame aggregation scales with visible rows and contention; a
  precomputed structure is the only way under budget at 10M+ on this GPU.

### Architectural Decisions

#### No library seam

- **Decision**: B and C draw their selection as a columnar layer built once per
  view; A records its passes around `Prepared::draw` with public `Context`
  handles.
- **Reasoning**: the GPU pass is then the library's own pipeline over exactly
  what the design would draw, and nothing needs removing afterwards.
- **Trade-off**: the per-frame CPU cost of the real design (a prefix count per
  chunk) is approximated by the prototype's selection plus a normal prepare.
- **Future**: the implementing stories own the seams (representative buffers,
  the indexed-rows glue).

#### A CPU sort instead of the GPU aggregate shader

- **Decision**: build B (and C) with a Morton sort on the CPU, not by porting
  `lod_aggregate.compute.wgsl`.
- **Reasoning**: the dense grid does not fit at fine tiers, and the sort yields
  every tier and the progressive order at once; the CPU build met the rule (≤ 2
  s per 10M).
- **Trade-off**: 562-826 ms per 10M on one thread is 7-10× the columnar first
  resolve; follow-up 1 builds lazily and with a radix sort, and a GPU radix sort
  is the next step if needed.

#### Area, not axis, for "one point per pixel"

- **Decision**: a tier's cells must be at least one pixel in area (the grid is
  square in data-extent units, so at full view cells are 1.8 × 1.0 px).
- **Reasoning**: one tier exponent for both axes; the rule's density bound holds
  per chunk.
- **Trade-off**: the wider axis loses up to 1.8 px of the band's edge, which is
  most of B's 0.994%; one tier finer halves it.

### Development Workflow Insights

- The worktree sandbox refuses shell constructs with computed values (loops,
  variables in paths); measurement loops went into small scripts under the
  checkout's target directory, run by absolute path.
- The pre-commit hook's prune runs on every commit; keeping measurement output
  under the checkout's own target directory kept it safe (the build-dir root is
  pruned by its own rules).
- All four candidates in one process per run gave same-session A/B numbers and
  let the fidelity check compare images rendered by one device.
- The machine's load swung from 0.5 to 7 during the session (other agents);
  build times moved by up to 2.6× with it, GPU times by under 5%.

### Follow-up Stories

See "Follow-up Stories" above (seven, in order; titles only, no IDs).
