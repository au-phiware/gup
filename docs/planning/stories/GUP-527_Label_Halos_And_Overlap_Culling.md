---
implementer: opus
implementer-reason:
  a halo needs a text-pipeline or scene change drawn identically in every target
  and in SVG; the culling order is a design choice
touches:
  [
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/render.rs,
    crates/gup-core/src/svg.rs,
  ]
---

# GUP-527: Legible Labels over Dense Marks (Halo and Overlap Culling)

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-11-02) **Created**: 2026-10-26

## Context

GUP-542's text layer draws every label that reaches the plot rect. Its
1,000-label measurement draws more than 900 labels on one 800×600 scatter, most
of them on top of each other and of the discs. Over dense marks, dark text on
blue discs is hard to read. GUP-542 left out halos and collision avoidance and
named them as follow-ups. Two cheap measures cover most charts:

- a **halo**: an outline in the background colour behind each glyph;
- **overlap culling**: drop a label whose ink box overlaps the box of a label
  already kept, in row order (or in a priority order the user gives).

## User Story

> "As a visualization developer, I want labels on a busy chart to stay legible,
> so that I can label many points without a mess of overlapping text."

## Read First

- `crates/gup-core/src/text_layer.rs` (all: `TextLayer` setters, `prepare`,
  `AnnotationLayer::runs`).
- `crates/gup-core/src/scene.rs` lines 310–385 (`TextStyle`, `TextRun`).
- `crates/gup-core/src/render.rs` `RendererGpu::text` (lays runs out into one
  glyph batch).
- `crates/gup-core/src/svg.rs` `ItemKind::Text` (the `<text>` writer).
- `crates/gup-text/src/system.rs` `TextSystem::layout`, and
  `crates/gup-text/src/atlas.rs` `GlyphAtlas::glyph` (rasterise once per key).
- `crates/gup-core/tests/text_layer.rs` (the 1,000-label cost test and the
  `text_annotations` golden) and `crates/gup-core/tests/secondary_y.rs`.

## Decisions

Set by the implementer on 2026-11-02 (the story had none).

1. **Halo = dilated glyph bitmaps in the same R8 atlas.** A halo glyph is the
   glyph's coverage bitmap dilated by the halo width in physical pixels
   (`out(p) = clamp(max_q c(q) + r − |p − q|, 0, 1)`: a disc dilation with a
   one-pixel antialiased edge, i.e. a round-joined stroke of width `2r` outside
   the outline), rasterised once and cached in the atlas under its own key
   (face, glyph, size, radius). The atlas stays one R8 texture and the glyph
   shader and pipeline are unchanged: a halo is just more glyph quads, in the
   halo colour, laid out by a new `TextSystem::layout_halo` before the run's own
   quads. Rejected: sampling a neighbourhood in the fragment shader (needs atlas
   padding of the largest halo, a shader change and a cost per fragment that
   grows with r²); drawing the text 8–16 times offset (a polygonal halo that
   does not match an SVG stroke).
2. **Draw order: halo then fill, per run**, in the one glyph draw (instances
   blend in order). That is exactly SVG's `paint-order="stroke"` on each
   `<text>`: a later label's halo covers an earlier label's ink where they
   overlap, in both targets.
3. **Scene**: `TextStyle` gains `pub halo: Option<Halo>`, with
   `Halo { width: Px, color: Color }` in `scene.rs`. The text layer fills in the
   colour from `Theme::background` at resolve, so the dark theme gets a dark
   halo. Every other text (ticks, titles, legends, tooltips) has `None`, and
   `None` adds no quads: output without a halo is unchanged byte for byte.
4. **SVG**: `stroke` of the halo colour, `stroke-width` = 2 × width,
   `stroke-linejoin="round"`, `paint-order="stroke"` (resvg 0.45 supports
   `paint-order` on text spans; checked by the SVG raster comparison).
5. **Default: no halo anywhere**, bar value labels included. Their goldens
   (`bar_value_labels*.png`) show why: outside labels sit on the background
   where a halo is invisible, and stacked labels sit on segment fills where a
   background-coloured halo would paint a pale blob on the bar. The dual-axis
   test (`secondary_y`) turns `halo(Px(2.0))` on, because "48" and "90" sit on
   the temperature line.
6. **Culling rule**: `hide_overlaps(true)` keeps rows in row order; a label is
   kept only if its box overlaps (shares positive area with) no kept box. The
   box is the label's ink box cut to the plot rect (`PlacedText::bounds`), grown
   by the halo width when there is one, so halos never cover kept ink. Labels
   dropped for other reasons (off the plot, `within`, no ink) take no room.
   Priority is row order: sort the rows to rank labels. Culling runs on every
   full resolve (a zoom changes positions), with a uniform grid over the plot
   rect (cells of 4 × the font size) holding the kept boxes. Edge room (GUP-555)
   is still computed from every label.
7. **Cost**: measured by extending `resolve_cost_of_a_thousand_labels`
   (release): culling must add under 0.3 ms per 1,000 labels.

## Escalation Triggers

- The halo needs a second atlas texture, a different atlas format or a change to
  `glyph.wgsl`.
- The SVG raster comparison fails for the halo golden by more than the existing
  `MAX_BLOCK_DE` (resvg's stroke and the dilation disagree).
- Culling costs more than 0.3 ms per 1,000 labels in release.
- A golden other than the new halo golden and `secondary_y` changes.

## Out of Scope

- A priority closure, label placement search (trying other anchors) and culling
  against other layers, tick labels or marks.
- Halos on guide text (ticks, titles, legends, tooltips).
- Room at the plot edge for the halo (a halo at the plot edge is clipped).

## Acceptance Criteria

- [x] `TextLayer::halo(Px)` draws each label with an outline of that width in
      the theme's background colour, identically in PNG, window and SVG
      (`stroke` with `paint-order="stroke"` in SVG). Default: no halo.
- [x] `TextLayer::hide_overlaps(true)` keeps a label only if its ink box
      overlaps no box kept before it (row order). It is documented, it is tested
      on `Layout::texts`, and its cost is measured with 1,000 labels.
- [x] A golden with halos over dense discs, read by eye.

## Technical Tasks

- [x] Choose how the glyph pipeline draws a halo (a dilated atlas sample, or a
      second pass of offset glyphs) and record why; escalate if it changes the
      glyph atlas format.
- [x] Culling with a grid of kept boxes; tests; golden; commit.

## Dependencies

- GUP-542 ✅: the text layer.
- GUP-405 📋 (soft): shaping changes ink boxes.

## Testing Strategy

- **Integration**: the culling rule on `Layout::texts`, and halo pixels around a
  glyph in the background colour.
- **Visual**: the golden.
- **Performance**: culling cost with 1,000 labels, in release.

## Success Metrics

- [x] A 1,000-point labelled scatter shows readable, non-overlapping labels.

## Risk Assessment

- **Medium**: a halo touches the text shader shared by every guide.
  _Mitigation_: a halo width of 0 keeps today's pipeline and output byte for
  byte.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] Rendered output verified by eye
- [x] `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

Delivered as the Decisions describe; nothing differs from them.

- **`gup-text`** (`atlas.rs`, `system.rs`): `GlyphAtlas::halo` rasterises a
  glyph, dilates it (`dilate`, a disc of the radius with a one-pixel ramp) onto
  a bitmap `margin = ceil(radius)` texels larger per side, and caches it in the
  same R8 atlas under `(face, glyph, size, radius)`. `TextSystem::layout_halo`
  lays those quads out, offset by the margin. The shader, pipeline and atlas
  format are unchanged.
- **`gup-core`**: `scene::Halo { width, color }` and `TextStyle::halo` (`None`
  for all guide text); `RendererGpu::text` lays out a run's halo just before the
  run; `SvgTarget` writes `stroke`, `stroke-width` (2 × width),
  `stroke-linejoin="round"` and `paint-order="stroke"`. `TextLayer::halo(Px)`
  (panics on a negative or non-finite width) takes its colour from
  `Theme::background`; `TextLayer::hide_overlaps(bool)` culls with `Kept`, a
  uniform grid (cells 4 × the font size, at most 64 × 64) over the plot rect.
  `Plot::text`'s docs point at both.
- **Tests**: `crates/gup-core/tests/text_halo.rs` (6): halo pixels in the
  background colour beside the glyphs on the light and dark themes, with the
  text colour kept and the mark back 5 px out; `halo(Px(0.0))` renders
  byte-for-byte what no halo renders; the SVG attributes, and on the resvg
  raster the text drawn over its stroke (a mutation to `paint-order="normal"`
  fails it with 0 text pixels); the greedy rule replayed over the uncut labels
  of a 1,000-point scatter equals `Layout::texts`, with and without a halo; more
  labels kept on a larger plot; the golden. Two `gup-text` unit tests (the
  dilation's values, the halo's cache entry), a `hide_overlaps` doctest, and the
  1,000-label cost test measures culling. `secondary_y` turns `halo(Px(2.0))`
  on.

### Evidence

- **Goldens**: `tests/golden/gup_core/text_halo.png` (new, 640×400), read by eye
  at 4× zoom: 300 blue discs (r 4 px) with a 2 px orange line across them. The
  values sit 6 px above their discs, and labels that would overlap an earlier
  one are hidden (the test asserts 40–300 kept). Each label's white halo cuts
  the line ("36.8", "43.9") and the edges of neighbouring discs ("37.5", "30.4")
  cleanly, with round corners. No two labels overlap.
  `tests/golden/gup_core/secondary_y.png` (re-blessed): "48" and "90" now sit on
  a white outline that breaks the temperature line behind their digits. The
  other labels are unchanged, and so is every other golden (`mask merge-check`
  passed with no other golden diff).
- **SVG parity** (block ΔE, limit 5.5): `text_halo` 3.1, `secondary_y` 5.4 (as
  before this story, from the line's antialiasing).
- **Culling cost** (release, median of 21 full resolves, 1,000 labels on
  800×600): +0.061, +0.047, +0.051 and +0.052 ms over the same layer without
  culling (0.27 ms), under the 0.3 ms limit. About half the labels are kept.
- **WASM** (`mask wasm-size` in merge-check): scatter **361,673 B gz**, +1,402 B
  over main's 360,271 (GUP-439's measurement); over wgpu 329,313 B.
- **`mask merge-check --dogfood`**: PASS (story-ids, doc, gup-core, wasm-size,
  wasm-browser, dogfood). **`cargo test --doc -p gup`**: 56 + 4 passed.
- **`scripts/perf_budget.sh --skip-wasm`**: run 1 (load average 7.4) failed
  `zoom.cpu.p95_ms` alone (1.096 against 1.075). Run 2 (load 6.7): every timing
  row passed (`zoom.cpu.median` 0.572, `zoom.cpu.p95` 0.762,
  `zoom_culled.cpu.median` 0.770, `pipeline.link_create.median` 1.171). The WASM
  rows report "not measured" because of `--skip-wasm`. The zoom scene has no
  halo, so its path changes only by one `Option` check per run.

## Retrospective

**Completed**: 2026-11-02

### Key Technical Learnings

#### A halo is a cached bitmap, not a shader effect

- **Challenge**: draw an outline that matches an SVG stroke without touching the
  glyph shader, the atlas format or the cost of text that has no halo.
- **Solution**: dilate the coverage bitmap on the CPU once per (glyph, size,
  radius) and store it as another atlas entry. The halo is then ordinary glyph
  quads in another colour, so the GPU, window and image targets need no new
  pipeline. Treating a pixel's coverage as the outline's distance from its
  centre (`c + r − d`) gives a one-pixel antialiased edge with one `max` per
  offset.
- **Pattern**: an effect that depends only on the glyph and its size belongs in
  the rasterised bitmap. Shader work pays per fragment every frame;
  rasterisation work is paid once.

#### `paint-order` per element equals draw order per run

- **Challenge**: the GPU and SVG must agree where labels overlap.
- **Solution**: in one instanced draw, instances blend in order, so laying out
  halo-then-text per run reproduces `paint-order="stroke"` on each `<text>`.
  resvg 0.45 honours `paint-order` on text spans. The unmasked raster check
  proves it: without the attribute, a 3 px stroke covers the 1.6 px stems and no
  text pixels are left.
- **Pattern**: the SVG parity harness masks text regions, so text effects need
  their own unmasked assertion on the resvg raster.

#### The greedy rule is easiest to test by replaying it

- **Solution**: resolve the same layer with and without `hide_overlaps`, run the
  rule naively (O(n²)) over the uncut labels, and compare with `Layout::texts`.
  This checks both the grid's correctness and the "row order" promise, and it
  needs no hand-placed fixtures.

### Architectural Decisions

#### `TextStyle::halo: Option<Halo>` in the scene

- **Decision**: the halo is part of the scene's text style, with its colour
  resolved at plot resolve from `Theme::background`.
- **Reasoning**: every target (GPU, SVG) reads the same scene, and the theme
  stays a plot-level concern.
- **Trade-off**: thirteen `TextStyle` literals gained `halo: None`.
- **Future**: guide text (tick labels over gridlines, legends over marks) can
  turn halos on without another scene change.

#### Culling by the clipped ink box, grown by the halo

- **Decision**: the culling box is `PlacedText::bounds` grown by the halo width.
  A halo therefore never covers kept ink, and with no halo the box is exactly
  the AC's ink box.
- **Trade-off**: edge room (GUP-555) still counts every label, culled or not,
  and a halo at the plot edge is clipped.

### Development Workflow Insights

- My worktree started 5 commits behind main (without the GUP-439 merge).
  Fast-forwarding before the first commit avoided a merge later.
- The worktree sandbox refuses compound shell commands that mix heredocs, `cd`
  or variables with `git`. Plain commands and the Edit tool work.
- A session rate limit stopped the work mid-implementation. The checkpoint ref
  and the WIP commit straight after the restart meant nothing was lost.
- Mutation-testing the SVG assertion took two minutes and showed that the raster
  check was not vacuous.
- `perf_budget.sh` timing rows are noisy under a load of about 7: one p95
  failure in run 1, none in run 2.

### Follow-up Stories

1. **GUP-611: Label placement tries other anchors before hiding a label**: with
   `hide_overlaps`, a label that collides above its point could still fit to its
   right, left or below. A ranked list of anchors, tried with the same grid,
   would keep more labels on dense scatters.
