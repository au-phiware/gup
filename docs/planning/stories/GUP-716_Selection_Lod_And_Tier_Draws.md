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

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
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

- [x] `#[non_exhaustive] pub enum Lod { Auto, Off }` with `Default = Auto`,
      `pub const AUTO_MIN_ROWS: u64 = 200_000` on `Lod`,
      `Selection::lod(self, Lod) -> Self` (the setter's style as `retain`),
      `ScatterChart::lod(self, Lod) -> Self`, `Lod` in the prelude. Exercised
      from a doctest (external-crate view) and `crates/gup-core/tests/lod.rs`.
      _Met as `Selection::lod(&mut self, Lod) -> &mut Self`_, which is
      `retain`'s style; doctests on `Lod` and `ScatterChart::lod`.
- [x] A layer drawn in size order (`Selection::size_order`) ignores `Lod` and
      draws every row until GUP-721 (a test asserts `Auto` and `Off` images and
      draw calls are equal for one at 1M rows). _There is no public
      `size_order`: a layer whose size channel is a column (`DrawOrder`)._
- [x] Only `Circle` and constant-extent `Rect` quads take LOD; a line, area,
      segment, text or a `Rect` with per-row extents ignores the setting (a test
      per mark kind asserts `draw_calls()` and the image are unchanged under
      `Lod::Auto` at 1M rows where applicable). _Text: `TextLayer` has no `Lod`
      setter, so not applicable._

### AC2: Threshold and tier rule

- [x] A layer whose visible (culled) rows are at most 200,000 draws exactly as
      today: for 200,000 rows the image equals `Lod::Off`'s byte for byte and no
      `Tiers` are built (a counter on the layer, asserted 0). At 200,001 rows
      with a view that sees them all, LOD applies.
- [x] Tier for a visible chunk `c`, with `A` the data area of one pixel at the
      current domains and plot rect:

  ```text
  t = floor(16 + (c.ex + c.ey) / 2 - 0.5 * log2(A)) + LOD_TIER_BIAS
  ```

  `LOD_TIER_BIAS: u32 = 1` (a private const with the fidelity reason in its
  doc), `t` clamped to 0..=16. `Tiers::prefix(t)` returning `None` (past the
  stored tiers) draws every row of the chunk, so LOD never draws more than today
  (a test: zoomed far in, instances equal rows).

  _Met with one change: `t` above 16 draws every row (`None`), as the spike's
  `View::tier` (clamped to 31, "past `TMAX` every row is drawn"), instead of
  being clamped to 16. Clamped, a 1M-row chunk spanning a million seconds seen
  through a 1 ms window drew one row per 16 s cell and lost the burst
  `scales_png::time_zoomed_to_a_millisecond_draws_within_a_quarter_pixel`
  checks._

- [x] A unit test of the formula against `View::tier` of the spike on its own
      example (full view of a 10M-row layer 1920 x 1080 gives the spike's tier).
      _`lod::tests`: on a chunk whose grid is the spike's, `tier` is the spike's
      plus the bias for 28 views; on the spike's 10M full view (its tier 10),
      one chunk over that extent draws 11, the spike's "one tier finer"._

### AC3: Draws and laziness

- [x] Tiers are built lazily on the first prepare that needs them, per chunk,
      one chunk per frame at most when more than one is missing (so a first
      frame does not stall 10M x 70 ns; chunks not yet built draw in full for
      that frame and the plot requests a redraw). A test counts builds per
      prepare. _One a frame under a host with a waker (`Chart::needs_frame`
      stays true and the resolve cache is bypassed until built); a headless plot
      builds every missing chunk in one prepare (the Risk's flag is "the plot
      has a waker", not a context flag). An owned layer whose release drops its
      CPU copy builds every full chunk in that prepare (Decisions)._
- [x] A 1M-row sorted random-walk scatter at 400 x 300: `Prepared::draw_calls()`
      is at most the chunk count (one instanced draw per visible chunk); the
      drawn instance count (`Prepared` stat or the test seam) is at most
      `plot_rect_pixels` (at most one per pixel of area, times 4 for the bias).
      _10,476 representatives in one chunk, 84,150 plot pixels; draw calls equal
      `Off`'s (one instanced draw per chunk plus the guides). Instances are
      `MarkBatch::instances()`; `__private::visible_instances` serves benches._

### AC4: Appearance and opt-outs

- [x] Search `tests/`, `examples/`, `crates/gup-core/wasm-size/` and the browser
      scenes for scatters above 200,000 rows or exercising `Retain`: each either
      still passes (list them) or is opted out with `.lod(Lod::Off)` (list them
      with the reason). `mask all-check` and `mask merge-check` pass. _See
      "Opt-outs" in the Implementation Summary._
- [x] The PNG of the 1M-row scatter under `Auto` is read by eye beside `Off`:
      same band, edges at most about 1 px narrower, no gaps; path in the
      retrospective. (The fidelity share and golden are GUP-717.)
- [x] `mask wasm-browser` passes; the scatter scene is unchanged (a small
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

- [x] `Lod`, setters, prelude, docs (with the "exact output: `Lod::Off`" note).
- [x] Tier formula and the prepare-time selection producing `DrawEntry::lod`.
- [x] Lazy, per-frame build scheduling.
- [x] Tests, opt-outs, doctest; changelog line under `[0.1.0]` Added.

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

- [x] A 1M-row scatter draws at most one instanced draw per visible chunk and a
      bounded instance count; at or under the threshold nothing changes.

## Risk Assessment

- **Medium**: a default-on setting changes any large existing scatter's picture.
  _Mitigation_: AC4's search and opt-outs; `Off` is exact.
- **Medium**: the per-frame lazy build interacts with first-frame screenshots
  (headless tests that render once). _Mitigation_: a headless render builds all
  missing tiers in its first prepare (a flag on the context), only an
  interactive frame spreads it; state which in the retrospective.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (AC4's PNG pair)
- [x] `mask wasm-browser` (the default is new browser-visible behaviour)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported; a rise above +2% is
      an escalation
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **API** (`selection.rs`, `lib.rs`, `builder/scatter.rs`):
  `#[non_exhaustive] pub enum Lod { Auto, Off }` (default `Auto`) with
  `Lod::AUTO_MIN_ROWS = 200_000`, beside `Retain`, documented with what it
  covers and what still reads every row (CPU pick, hover and tooltips over CPU
  data, SVG); `Selection::lod(&mut self, Lod) -> &mut Self`;
  `ScatterChart::lod(self, Lod) -> Self`; `Lod` re-exported and in the prelude.
  Hidden seams: `__private::lod_tier_builds(&Selection)` (tiers built so far)
  and `__private::visible_instances(&MarkBatch)` (instances of the visible
  spans).
- **Tier rule** (`lod.rs`): `tier(ex, ey, kx, ky)`, the AC's formula with
  `LOD_TIER_BIAS = 1` (private, fidelity reason in its doc), from the chunk's
  pixels per data unit along x and y (its extent's image over its extent). A
  chunk with no span on one axis takes the 1-D rule on the other; no span on
  either draws one row. Above tier 16, or for a scale that is not finite,
  `None`: every row.
- **Per-frame choice** (`selection.rs`): `Selection::lod_entries` (generic and
  small: eligibility, the x/y store columns and each chunk's scale) and
  `tier_entries` (not generic): first the release build (every chunk
  `ColumnStore::releasable` when the prepare's release drops CPU copies), then,
  when the visible chunks hold more than 200,000 rows, each visible chunk draws
  its tier's prefix through GUP-715's `lod_tiers`, `upload_lod`,
  `DrawEntry::lod` and indexed program, unchanged. Eligible: `Lod::Auto`, a quad
  with `HitShape::Disc` or a constant-extent `AlignedBox`, no size order, more
  than 200,000 rows. An appended layer's newest chunk, while not full, draws
  every row (GUP-719). GUP-715's `ForcedTier` function pointers are gone (the
  default links the build anyway); `force_lod_tier` stays as a test seam that
  overrides the choice.
- **Laziness** (`plot.rs`, `lod::Frame`): the plot now computes each layer's
  visible-entry mask before `Layer::prepare` and passes it with `spread` (the
  plot has a host waker). Spread: one missing chunk builds per prepare, the
  layer reports `lod_pending`, `Chart::needs_frame` is true and `Plot::resolve`
  bypasses its cache until every visible chunk is built. Headless: every missing
  chunk builds in the one prepare.
- **Store** (`column.rs`): `has_tiers`, `chunk_full`, `releasable`.
- **Measurement** (`examples/lod_spike.rs`): candidate `auto` (`Lod::Auto`, its
  first frame timed as `setup_ms`, plus `chunks_built`); the prototypes now draw
  through `Lod::Off`.
- **Opt-outs** (AC4), each `.lod(Lod::Off)` because its numbers were recorded on
  full draws: `zoom_bench` (PERF_BUDGETS rows; GUP-717 adds `--lod`),
  `edge_inset_bench` (times resolves of 4M rows), `keyed_resolve_bench` (1M
  rows, times key resolves), `scale_bench` (default off; `--lod-draw auto`
  measures the build) and `lod_spike`'s prototypes. Passing under `Auto`:
  `tests/window.rs` (300,000 bubbles: sized, so ignored), `tests/scales_png.rs`
  (1M rows; needed the tier-above-16 rule), and the `Retain` tests
  `heatmap_png`, `events`, `msaa`, `pick`, `area_pick`, `row_text`, `svg_marks`,
  `columns`, `line_monotone` and `scatter_png` (all small). `wasm-size` scenes:
  200 and 60 rows, unchanged. `gup-culling-lod`'s examples are the old crate,
  untouched.
- **Tests**: `tests/lod.rs` 13 (threshold byte equality and 0 builds at 200,000,
  LOD at 200,001; the 1M walk's draw calls, instances, PNG pair and ΔE share;
  zoomed far in; one build a frame with a waker, all at once without; the
  release build of a `GpuOnly` layer; `Off`; size order, line, area, segment and
  per-row `Rect` unchanged at 1M; a constant `Rect` takes LOD; the scatter
  builder); `lod.rs` 3 unit tests (the formula against the spike's `View::tier`,
  the spike's own 10M full view, degenerate chunks); doctests on `Lod` and
  `ScatterChart::lod`.

### Measurements

Release, Intel HD 630 (Mesa 26.0.0, Vulkan), headless 1920 × 1080, `lod_spike`
sorted walk, radius 1.5, 30 frames after 3 warm-up; three runs, alternating
order, with `pyramid --pyramid-bias 1` (the spike's B, one tier finer) as the
same-session reference. Load 3.4-6.8 from other agents, `MemAvailable` 2.8-5.9
GB (no 80M run: under 20 GB free). GPU ms is the scene pass median.

| rows | view | candidate  | drawn   | run 1 | run 2 | run 3 | median | spike table (B / B+1) |
| ---- | ---- | ---------- | ------- | ----- | ----- | ----- | ------ | --------------------- |
| 10M  | full | auto       | 77,950  | 3.55  | 4.00  | 2.79  | 3.55   | 2.32 / 3.34           |
| 10M  | full | pyramid +1 | 94,123  | 4.80  | 4.55  | 4.53  | 4.55   |                       |
| 10M  | zoom | auto       | 106,393 | 4.62  | 4.79  | 3.65  | 4.62   | 3.62 / 3.89           |
| 10M  | zoom | pyramid +1 | 196,181 | 4.77  | 4.89  | 4.85  | 4.85   |                       |
| 30M  | full | auto       | 89,022  | 3.70  | 4.15  | 2.82  | 3.70   | 3.22 / -              |
| 30M  | full | pyramid +1 | 139,192 | 4.93  | 4.59  | 5.32  | 4.93   |                       |
| 30M  | zoom | auto       | 148,901 | 4.20  | 4.64  | 3.38  | 4.20   | 3.23 / -              |
| 30M  | zoom | pyramid +1 | 203,978 | 5.05  | 4.33  | 4.78  | 4.78   |                       |

Today's path at the same sizes (spike table): 934 / 27.1 ms (10M) and 2,914 /
51.6 ms (30M). `auto`'s first frame (headless, so every visible chunk's tiers
built and uploaded; shared columns): 953 / 1,269 / 1,294 ms at 10M, 2,363 /
2,684 / 3,263 ms at 30M (29 chunks).

**Eager build for an owned layer**
(`scale_bench --rows 11000000 --resolve-only --lod-draw off|auto`: rows,
`Retain::Auto`, so the first prepare releases; alternating, load 4.1):
`first_resolve` off 248 / 255 / 275 ms (median 255), auto 1,225 / 1,276 / 1,357
ms (median 1,276), so **+1,021 ms** (0.93 s per 10M rows: 11 chunks' tiers and
gathers). The render and readback after it take 1,190 ms off against 125 ms
auto, so the time to the first image is about equal (1,445 against 1,401 ms).
Uploads: +46.9 MB of representatives (4.26 B a row).

**Fidelity** (ΔE00 > 10 against `Lod::Off`, % of the image / of the plot rect;
deterministic):

| scene                          | auto full     | spike B+1 full | auto zoom     | spike B+1 zoom |
| ------------------------------ | ------------- | -------------- | ------------- | -------------- |
| 10M, 1920 × 1080               | 0.727 / 0.788 | 0.580 / 0.628¹ | 1.088 / 1.171 | 1.224 / 1.236¹ |
| 1M, 1920 × 1080                | 0.807 / 0.874 | 0.567 / 0.614  | 0.000 / 0.000 | 0.000 / 0.000  |
| 1M, 400 × 300 (spike's walk)   | 1.760 / 2.510 | 1.284 / 1.831  | 2.211 / 3.115 | 1.348 / 1.900  |
| 1M, 400 × 300 (`tests/lod.rs`) | 1.098 / 1.566 | -              | -             | -              |

¹ The spike's table (GUP-631). The gap to B+1 is the grid: each chunk's tier-16
grid is a power of two of data units (GUP-714), up to twice the chunk's extent
on each axis, so at the same tier its cells are up to 4× the spike's in area (at
400 × 300: 0.48 px² against 0.30, 10,763 representatives against 16,912).

### Definition-of-Done evidence

- **Rendered output**:
  `GUP_LOD_PNG_DIR=/tmp/gup716 cargo test -p gup-core --test lod a_million`
  writes `lod-auto-1m.png` and `lod-off-1m.png` (400 × 300). Read side by side:
  the same walk band, axes and ticks, no gaps; the `Auto` band's edges are
  slightly lighter and at most about a pixel narrower.
- **Goldens unchanged**: none blessed; `git diff --stat 9a39bdb8` lists no file
  under `tests/golden`, and the gup-core golden tests (in `merge-check`), the
  browser scene and the tour pages pass. Every golden draws far fewer than
  200,000 rows.
- **WASM** (`mask wasm-size`, scatter gz): 384,562 recorded on main 9a39bdb8,
  390,607 on the branch (**+6,045, +1.57%**; limit 392,253); `per_row_type`
  1,511 (recorded 1,730). Attribution (`twiggy` on both `wasm-bindgen` outputs,
  raw, +37.3 KB in all): `Layer::prepare` +7.0 KB (main folded its three
  row-type instantiations into one 18,449-byte copy; the branch keeps three of
  8,491), `lod_entries` +3.3 KB (three copies), `ColumnStore::lod_tiers` (the
  radix build) +3.2 KB, `tier_entries` +2.2 KB, `upload_lod` +1.4 KB,
  `indexed_program` +1.0 KB, `Glue::clone` +0.9 KB, `.rodata` +1.3 KB, and
  inlining shifts in `ColumnStore::upload` (+2.8 KB), `Context::program` (+1.9
  KB), `LayerUniforms::build` (+1.1 KB) and `LayerGpu::with_chunks` (+1.0 KB).
  Trims tried: the per-chunk choice moved out of generic code, and
  `#[inline(never)]` on `lod_entries` and `emit_glue`: +346 and +11 B gz; kept
  the non-generic split for clarity, dropped the attributes.
- **Validation**: `mask merge-check` every step PASS (gup-core all features: 70
  binaries, 1,022 passed, 0 failed, 8 ignored); `mask all-check` exit 0;
  `cargo test -p gup -- --test-threads=1` 11 binaries, 154 passed, 0 failed, 2
  ignored; `cargo test -p gup-egui` 5 binaries, 19 passed, 0 failed, 1 ignored;
  `mask wasm-browser` PASS (the scatter scene unchanged); `mask smoke-examples`
  PASS.
- **Not done**: `mask perf-budget` (the benches it runs are opted out, so it
  measures what it did; GUP-717 adds the LOD rows); 80M (under 20 GB free).

## Retrospective

**Completed**: 2026-11-12 (pending the fidelity decision below)

### Key Technical Learnings

#### The finest tier is not "every row"

- **Challenge**: the AC clamped the tier to 16, but tier 16 is stored whenever
  it holds at most half the rows, and its cells are a fixed fraction of the
  chunk's extent. A 1M-row chunk spanning a million seconds, seen through a 1 ms
  window, drew one row per 16 s cell; `scales_png`'s burst vanished.
- **Solution**: a tier above 16 means the finest cells are coarser than the view
  needs: draw every row, as the spike's `View::tier` did (clamped to 31).
- **Pattern**: "past the stored tiers" includes "past the finest tier there is";
  test LOD with a zoom far below the chunk's grid, not only the full view.

#### The plot knows the view; the layer knows its chunks

- **Challenge**: the tier needs the visible chunks and the scale, but
  `Layer::prepare` ran before the plot computed the culling mask.
- **Solution**: compute the mask first and pass it in `lod::Frame` (with
  `spread`, "the plot has a waker"); the scale per chunk comes from the
  encoding's image of the chunk's extent, so the layer needs no plot rect.
- **Pattern**: a per-frame choice that depends on the view belongs in prepare,
  fed by the plot, and needs a cache bypass (`lod_pending`) to finish work over
  frames without a revision bump.

#### Generic code and function folding in WASM

- **Challenge**: +6 KB gz, of which the largest item was `Layer::prepare`: main
  linked one copy for three row types (identical instantiations folded), the
  branch three.
- **Solution**: not found; moving the choice into non-generic code and
  `#[inline(never)]` changed the gz total by under 400 B. Accepted within +2%.
- **Pattern**: compare instantiation counts (`twiggy` CSV grouped by name
  without hashes), not only sizes, when a generic function grows.

### Architectural Decisions

#### Spreading builds: "has a waker" is the interactive flag

- **Decision**: one chunk a prepare when the plot has a host waker (the window
  app, the web canvas), every missing chunk otherwise (headless, `gup-egui`,
  tests).
- **Reasoning**: the hosts that set a waker are those that redraw on
  `needs_frame`; a headless render must be complete in one frame.
- **Trade-off**: an interactive frame that builds a 1M-row chunk takes about
  90-130 ms; an owned layer above 10M builds every full chunk in its first
  prepare (+1.0 s at 11M rows).
- **Future**: building off the frame path (GUP-730).

#### Opt-outs for the benches

- **Decision**: `zoom_bench`, `edge_inset_bench`, `keyed_resolve_bench` and
  `scale_bench` (default) draw `Lod::Off`.
- **Reasoning**: their budgets and recorded numbers measure full draws; GUP-717
  adds `zoom_bench --lod` and the LOD budget rows.

### Development Workflow Insights

- `lod_spike` with an `auto` candidate gave same-session A/Bs and fidelity
  shares against the spike's own design at any size, which is how the grid's
  effect at 400 × 300 was separated from the size's.
- `mask merge-check` stops gup-core's tests at the first failing binary; after a
  fix, run it again whole.
- The worktree sandbox refuses heredocs and some compound commands; edit scripts
  under `/tmp/gup716` ran without trouble.

### Follow-up Stories

1. **GUP-730: Tier builds off the frame path** — build chunk tiers and gathers
   on worker threads (native), so an owned layer above 10M rows does not add
   about 1 s per 10M rows to its first frame and an interactive frame never
   waits for a chunk's build; needed for GUP-632's 10 s first frame at 100M.

For the orchestrator (not stories): `docs/tour/11_big_data.md` line 77 still
says "There is no level of detail" (the brief kept the tour out of scope), and
GUP-717's 1.0% / 2.0% plot-rect gates at 400 × 300 are not met even by the
spike's own design there (1.831% / 1.900%; see the fidelity table).

## Blocked (2026-11-12)

**Question**: at the story's own 1M-row, 400 × 300 scene, `Lod::Auto` is 0.48
(image) / 0.68 (plot rect) points less faithful than the spike's design with the
same bias (1.760% / 2.510% against 1.284% / 1.831%, full view), over the
0.3-point escalation trigger; at the spike's 1920 × 1080 it is within it (1M:
+0.24 / +0.26; 10M: +0.15 / +0.16 against the spike's table, and better in the
zoom view). Accept the formula as specified, or change the tier rule?

**Evidence**: the fidelity table above
(`lod_spike --candidates base,pyramid,auto --pyramid-bias 1 --fidelity`, sizes
as listed). Cause: each chunk's tier-16 grid is a power of two of data units
(GUP-714's anchored grid, which keeps built tiers valid as a layer grows), up to
twice the chunk's extent per axis, so at the same tier its cells are up to 4×
the spike's in area: at 400 × 300 both draw tier 9, with cells of 0.48 px²
against 0.30 and 10,763 representatives against 16,912.

**Options**:

- **A. Accept** (as implemented): the AC's formula and bias. Within 0.3 points
  of the spike's numbers at its own scenes; GUP-717's 400 × 300 gates (1.0% /
  2.0% of the plot rect) would need new values, since the spike's design scores
  1.831% / 1.900% there.
- **B. Round instead of floor** (`t = floor(x + 0.5) + 1`, a half tier finer):
  measured once (load about 6): 10M 1080p full 0.603% / 0.653% (spike 0.580 /
  0.628), zoom 0.494 / 0.532; 1M 1080p 0.362 / 0.392; 110,031 / 265,507
  representatives at 10M, GPU 3.11 / 4.71 ms. At 400 × 300 the full view is
  unchanged (1.760 / 2.510; that view sits below the rounding point) and the
  zoom improves to 0.720 / 1.014.
- **C. A grid fitted to each chunk's extent** (not powers of two): the spike's
  cells, but appends would invalidate built tiers (against GUP-714's Decisions).
  Not recommended.

**Recommendation**: A, with GUP-717's gates re-derived from measured values at
its scene. B is a one-line change if the orchestrator prefers spike parity at
1080p for about 40% more representatives.
