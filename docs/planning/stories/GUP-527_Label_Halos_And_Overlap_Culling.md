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

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-10-26

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
