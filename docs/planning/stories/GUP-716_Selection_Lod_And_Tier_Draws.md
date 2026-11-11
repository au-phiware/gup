---
implementer: opus
implementer-reason:
  the public `Lod` API and the per-frame tier choice that decide every LOD
  frame's draws; a core selection.rs/render.rs change
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/render.rs,
    crates/gup-core/src/lod.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/builder/,
    crates/gup-core/src/plot.rs,
  ]
---

# GUP-716: `Selection::lod` and Per-Frame Tier Draws

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 📋 Planned
**Created**: 2026-11-11

## Context

GUP-714 builds per-chunk tiers; GUP-715 uploads representatives and draws a
forced tier. This story makes LOD a feature: the public `Lod` setting, the
threshold above which it applies, the lazy build, and the per-frame choice of
each visible chunk's tier (RFC-001 "GUP-631 findings", "Proposed API" and "The
decision"). It also settles the spike's open "tier offset": the decision's tier
(at most one point per pixel of area) passed the fidelity rule by 0.006 points
of the image and failed it on the plot rect (1.076%); one tier finer scored
0.580% / 0.628% for 1.0 ms more at 10M. This story draws **one tier finer**.

## Read First

- RFC-001 "GUP-631 findings": "Proposed API", "The decision" (threshold),
  "Fidelity" table, "Proposed adjustments" (tier offset, build).
- `crates/gup-core/examples/lod_spike.rs`: `View::tier`, `Pyramid::select`
  (lines ~351-380, 491-506).
- GUP-714's `lod::Tiers` (`prefix`, `ex`, `ey`) and GUP-715's `LodGpu`,
  `DrawEntry::lod`, `force_lod_tier`.
- `crates/gup-core/src/selection.rs`: the `Selection` setters (e.g. `window`,
  `retain`), `cpu_readable`, the culling and draw entries in prepare (search
  `cull::` and `draw_entries`); `crates/gup-core/src/cull.rs`.
- `crates/gup-core/src/builder/` (`ScatterChart`, how a builder forwards a
  `Selection` setter, e.g. `retain` or `window`); `crates/gup-core/src/lib.rs`
  prelude.

## User Story

> "As a developer of a scatter of tens of millions of rows, I want Gup to draw a
> bounded number of representatives per frame automatically, so that zoom and
> pan stay interactive without me thinning the data."

## Acceptance Criteria

### AC1: The API

- [ ] `#[non_exhaustive] pub enum Lod { Auto, Off }` with `Default = Auto`,
      `pub const AUTO_MIN_ROWS: u64 = 200_000` on `Lod`,
      `Selection::lod(self, Lod) -> Self` (the setter's style as `retain`),
      `ScatterChart::lod(self, Lod) -> Self`, `Lod` in the prelude. Exercised
      from a doctest (external-crate view) and `crates/gup-core/tests/lod.rs`.
- [ ] A layer drawn in size order (`Selection::size_order`) ignores `Lod` and
      draws every row until GUP-721 (a test asserts `Auto` and `Off` images and
      draw calls are equal for one at 1M rows).
- [ ] Only `Circle` and constant-extent `Rect` quads take LOD; a line, area,
      segment, text or a `Rect` with per-row extents ignores the setting (a test
      per mark kind asserts `draw_calls()` and the image are unchanged under
      `Lod::Auto` at 1M rows where applicable).

### AC2: Threshold and tier rule

- [ ] A layer whose visible (culled) rows are at most 200,000 draws exactly as
      today: for 200,000 rows the image equals `Lod::Off`'s byte for byte and no
      `Tiers` are built (a counter on the layer, asserted 0). At 200,001 rows
      with a view that sees them all, LOD applies.
- [ ] Tier for a visible chunk `c`, with `A` the data area of one pixel at the
      current domains and plot rect:

  ```text
  t = floor(16 + (c.ex + c.ey) / 2 - 0.5 * log2(A)) + LOD_TIER_BIAS
  ```

  `LOD_TIER_BIAS: u32 = 1` (a private const with the fidelity reason in its
  doc), `t` clamped to 0..=16. `Tiers::prefix(t)` returning `None` (past the
  stored tiers) draws every row of the chunk, so LOD never draws more than today
  (a test: zoomed far in, instances equal rows).

- [ ] A unit test of the formula against `View::tier` of the spike on its own
      example (full view of a 10M-row layer 1920 x 1080 gives the spike's tier).

### AC3: Draws and laziness

- [ ] Tiers are built lazily on the first prepare that needs them, per chunk,
      one chunk per frame at most when more than one is missing (so a first
      frame does not stall 10M x 70 ns; chunks not yet built draw in full for
      that frame and the plot requests a redraw). A test counts builds per
      prepare.
- [ ] A 1M-row sorted random-walk scatter at 400 x 300: `Prepared::draw_calls()`
      is at most the chunk count (one instanced draw per visible chunk); the
      drawn instance count (`Prepared` stat or the test seam) is at most
      `plot_rect_pixels` (at most one per pixel of area, times 4 for the bias).

### AC4: Appearance and opt-outs

- [ ] Search `tests/`, `examples/`, `crates/gup-core/wasm-size/` and the browser
      scenes for scatters above 200,000 rows or exercising `Retain`: each either
      still passes (list them) or is opted out with `.lod(Lod::Off)` (list them
      with the reason). `mask all-check` and `mask merge-check` pass.
- [ ] The PNG of the 1M-row scatter under `Auto` is read by eye beside `Off`:
      same band, edges at most about 1 px narrower, no gaps; path in the
      retrospective. (The fidelity share and golden are GUP-717.)
- [ ] `mask wasm-browser` passes; the scatter scene is unchanged (a small
      layer).

## Decisions

- **Default `Auto`**: LOD above the threshold is the documented behaviour of a
  large scatter; `Off` is the exact output (SVG parity, GUP-718).
- **Tier bias 1** (draw one tier finer), at most four points per pixel: 1.0 ms
  more at 10M for 0.58% against 0.994%; the plot-rect denominator passes only
  with it.
- **Threshold counts visible rows after chunk culling** (the spike's measure),
  summed over the layer.
- **One instanced draw per visible chunk**, the count being the prefix; no
  culling inside a chunk (GUP-720).
- **A chunk whose tiers are not built draws in full** (RFC-001) - so the first
  frame at 10M is today's frame, and the build is spread over frames.
- **Decision handed to GUP-716 (from GUP-715, 2026-11-12, orchestrator): build
  before release.** An owned store (a `Selection<T>`, or a columnar layer with
  any closure channel) drops the CPU copy of its full chunks after its first
  prepare under `Retain::Auto` above 10M rows (and at any size under `GpuOnly`
  or `Rows`); afterwards neither `Tiers::build` nor the `LodGpu` gather has a
  CPU source. So in the prepare whose `release()` would drop a LOD-eligible
  layer's CPU copy (quad mark, no size order, over the threshold, `Lod::Auto`),
  build the tiers and `LodGpu` of every full chunk first; readable stores
  (shared columns, or CPU copies kept) stay lazy per chunk. Not A (limit LOD to
  readable stores): owned layers over 10M rows are the layers LOD is for. Not C
  (release waits for the build): it keeps `Retain::Columns`-level CPU memory
  until every chunk has been built, and a chunk never panned into view is never
  released. Cost: about 0.5 s per 10M rows for the tiers (GUP-714: 484 ms) plus
  tens of ms for the gather, on top of today's first frame of about 1 s at 10M;
  GPU memory at most 12 B a representative, at most 6 B a data row (the
  half-chunk cap). This qualifies "a chunk whose tiers are not built draws in
  full" for released stores: their tiers are always built before release.
- **Where**: `Lod` in `selection.rs` beside `Retain`; per-frame choice in the
  prepare path that makes `DrawEntry`s; `lod.rs` holds the formula.

## Escalation Triggers

- Spreading the build over frames needs a redraw request the plot cannot make
  from prepare (it must stay synchronous: then build all missing chunks in the
  first prepare and report the stall in ms at 10M).
- A mark's `Selection` setter is consumed by a path (SVG, text, describe) that
  now sees two row sets (drawn vs all): stop; GUP-718 owns that.
- The fidelity at 1M under the bias exceeds the numbers of the spike by more
  than 0.3 points when measured by eye or by the harness.

## Out of Scope

- Pick, hover, tooltips, SVG under LOD (GUP-718); appends and windows (GUP-719);
  spatial culling (GUP-720); size order (GUP-721); the golden with the ΔE00
  check and `zoom_bench --lod`/budgets (GUP-717).

## Technical Tasks

- [ ] `Lod`, setters, prelude, docs (with the "exact output: `Lod::Off`" note).
- [ ] Tier formula and the prepare-time selection producing `DrawEntry::lod`.
- [ ] Lazy, per-frame build scheduling.
- [ ] Tests, opt-outs, doctest; changelog line under `[0.1.0]` Added.

## Dependencies

### Prerequisite Stories

- GUP-714: LOD Tiers in the Column Store 📋.
- GUP-715: Representative Buffers and the Indexed-Rows Glue 📋.

### Enables Stories

- GUP-717, 718, 719, 720, 721; GUP-632's 100M demonstration (soft).

## Testing Strategy

- **Unit tests**: formula, threshold, laziness counter.
- **Integration tests**: `crates/gup-core/tests/lod.rs` (draw calls, threshold,
  per-mark opt-out); doctest for the public API.
- **Visual validation**: AC4's PNG pair.

## Success Metrics

- [ ] A 1M-row scatter draws at most one instanced draw per visible chunk and a
      bounded instance count; at or under the threshold nothing changes.

## Risk Assessment

- **Medium**: a default-on setting changes any large existing scatter's picture.
  _Mitigation_: AC4's search and opt-outs; `Off` is exact.
- **Medium**: the per-frame lazy build interacts with first-frame screenshots
  (headless tests that render once). _Mitigation_: a headless render builds all
  missing tiers in its first prepare (a flag on the context), only an
  interactive frame spreads it; state which in the retrospective.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (AC4's PNG pair)
- [ ] `mask wasm-browser` (the default is new browser-visible behaviour)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported; a rise above +2% is
      an escalation
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
