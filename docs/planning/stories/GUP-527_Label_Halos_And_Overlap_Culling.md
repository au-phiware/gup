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

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress **Created**: 2026-10-26

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

- [ ] `TextLayer::halo(Px)` draws each label with an outline of that width in
      the theme's background colour, identically in PNG, window and SVG
      (`stroke` with `paint-order="stroke"` in SVG). Default: no halo.
- [ ] `TextLayer::hide_overlaps(true)` keeps a label only if its ink box
      overlaps no box kept before it (row order). It is documented, it is tested
      on `Layout::texts`, and its cost is measured with 1,000 labels.
- [ ] A golden with halos over dense discs, read by eye.

## Technical Tasks

- [ ] Choose how the glyph pipeline draws a halo (a dilated atlas sample, or a
      second pass of offset glyphs) and record why; escalate if it changes the
      glyph atlas format.
- [ ] Culling with a grid of kept boxes; tests; golden; commit.

## Dependencies

- GUP-542 ✅: the text layer.
- GUP-405 📋 (soft): shaping changes ink boxes.

## Testing Strategy

- **Integration**: the culling rule on `Layout::texts`, and halo pixels around a
  glyph in the background colour.
- **Visual**: the golden.
- **Performance**: culling cost with 1,000 labels, in release.

## Success Metrics

- [ ] A 1,000-point labelled scatter shows readable, non-overlapping labels.

## Risk Assessment

- **Medium**: a halo touches the text shader shared by every guide.
  _Mitigation_: a halo width of 0 keeps today's pipeline and output byte for
  byte.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Rendered output verified by eye
- [ ] `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
