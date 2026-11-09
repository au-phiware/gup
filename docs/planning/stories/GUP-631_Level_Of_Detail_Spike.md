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

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-04

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

- [ ] `crates/gup-core/examples/lod_spike.rs` builds a 100M-row scatter from
      `Columns` (GUP-628; x sorted, y a random walk; at 30M if 100M does not fit
      the machine, saying so) and measures, at 1920 × 1080 on the development
      GPU, full view and a 10× zoom on x and y, each candidate of Decisions: GPU
      pass median and p95 (timestamp queries, as `zoom_bench`), CPU per frame,
      build time, extra GPU and CPU memory.
- [ ] The baseline (draw every visible row, today's path) at 10M and at the
      largest size that runs, for comparison.
- [ ] Fidelity: for each candidate, the share of pixels whose colour differs by
      ΔE00 > 10 from the full draw at 10M rows in both views (the full draw is
      the reference; use `gup_visual_regression`'s colour functions).
- [ ] Library seams needed by the prototypes are `#[doc(hidden)] pub` items
      listed in the findings; they are kept only if the chosen design needs
      them, else removed before the story closes.

### AC2: The decision

- [ ] The rule of Decisions is applied to AC1's numbers and the outcome is
      recorded with the table.

### AC3: Findings and follow-ups

- [ ] A "GUP-631 findings" section appended to RFC-001: the candidates, the
      table, the fidelity numbers, the decision, the public API proposed (for
      example a `Selection::lod(..)` setter or an automatic threshold), what
      happens on hover, pick, brush, SVG and tooltips under LOD, and how it
      interacts with `Window::LastRows`, appends and chunk culling.
- [ ] A "Follow-up Stories" section in this story file listing the stories to
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

- [ ] Port or write the three prototypes in `lod_spike.rs` with seams.
- [ ] Measurements (AC1), the rule (AC2), findings and follow-ups (AC3).

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

- [ ] A LOD design chosen by a stated rule from measured numbers, with stories
      to build it.

## Risk Assessment

- **High**: unknown GPU behaviour at 100M rows on an integrated GPU (memory,
  driver limits). _Mitigation_: the 30M fallback, and the rule records "no
  candidate meets the budget" rather than forcing a choice.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: the candidates' PNGs
- [ ] `mask wasm-browser`: not needed, a spike example
- [ ] WASM size: `mask wasm-size` scatter gz bytes if any seam stays in the
      library, else "unchanged: no library code change"
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
