# GUP-416: gup-text Font Fallback for Characters Outside the Face

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 💡 New **Created**: 2026-10-10

## Context

[GUP-407](GUP-407_Subset_Inter_And_SVG_Font_Embedding.md) made a 515-character
Inter subset the bundled default. It covers Latin (Basic, Latin-1, Extended-A),
basic Greek and chart symbols. A character outside it draws as Inter's `.notdef`
box, and `Font::missing_glyphs` reports it. A user can swap the whole face
(`Font::inter_full`, `Font::from_bytes`, `TextSystem::with_font`), but
`TextSystem` holds exactly one font. A chart with one Cyrillic, CJK or emoji
label therefore has two choices: boxes, or a different face for every label.

Data labels are user data, so mixed scripts are normal: city names, product
names and people's names. The full Inter covers Cyrillic and more Greek, but no
CJK, Arabic or emoji. Shipping more fonts is not the answer, because GUP-407 cut
the font's share of the wasm download from 198 KB to 30 KB gzipped. A fallback
chain the user supplies keeps that size and still draws their text.

## User Story

> "As a Gup user labelling data in several scripts, I want characters the
> default font lacks to come from a font I supply, so that my labels are legible
> and the rest of my text keeps the default look."

## Acceptance Criteria

### AC1: A fallback chain

- [ ] `TextSystem` takes a primary font and an ordered list of fallback fonts
      (for example `TextSystem::with_fonts(device, primary, &[fallbacks])`,
      replacing `with_font` rather than adding a second constructor). Each
      character uses the first font in the chain that has a glyph for it. A
      character no font has draws as the primary's `.notdef` box, as now.
- [ ] `measure`, `ink_bounds` and `layout` agree with the drawn glyphs across a
      font switch. The `draw_into_pass` test (ink only inside `ink_bounds`)
      gains a mixed-font run and still passes.
- [ ] The atlas key includes the font, so the same character from two fonts does
      not collide.

### AC2: Lazy and cheap

- [ ] A fallback font is parsed on first use, not at construction, and a chart
      whose text the primary covers never parses it.
- [ ] No fallback font is bundled. A test shows the default `TextSystem` still
      links only the subset (`mask wasm-size` unchanged).

### AC3: SVG

- [ ] `SvgTarget` emits `font-family="<primary>, <fallback>…, sans-serif"`. With
      `embed_font`, each font the scene uses gets an `@font-face`, and a
      fallback the scene does not use is not embedded.

### AC4: Visible result

- [ ] A golden image of a scatter with a mixed-script title (for example "Київ
      — 東京 — Zürich") using a test-only fallback face, checked by eye: no
      boxes, a consistent baseline.

## Technical Tasks

- [ ] Replace `TextSystem`'s single `OnceLock<Font>` with a chain whose
      fallbacks parse lazily.
- [ ] Split a run into same-font segments in `layout.rs`. With GUP-405, shape
      each segment with its own face.
- [ ] Key the atlas by `(font index, glyph, size bits)`.
- [ ] Extend `SvgTarget`'s `font-family` and embedding.
- [ ] Add a small OFL test face with CJK coverage under `tests/` (not bundled),
      or subset one with `mask subset-inter`'s recipe.

## Dependencies

### Prerequisite Stories

- GUP-407: Subset the bundled Inter ✅
- GUP-405: gup-text OpenType shaping 📋 (soft). Shaping works per font, so
  segmenting runs by font is simpler to build after it, or alongside it.

### Enables Stories

- International labels in every chart without giving up the small default font.

## Testing Strategy

- **Unit**: chain selection (first font with the glyph wins); fallback not
  parsed when unused; atlas keys distinct per font.
- **Integration**: `draw_into_pass` with a mixed-font run; SVG `font-family` and
  embedding per used font.
- **Visual**: one new golden, read by eye.

## Success Metrics

- Mixed-script labels draw without boxes when a fallback covers them.
- The default wasm build size is unchanged.

## Risk Assessment

- **Medium**: baseline and line-height consistency across faces with different
  metrics. Mitigation: line metrics come from the primary font, and a golden
  checks the baseline.
- **Low**: test-font licensing. Mitigation: use an OFL face and keep its notice
  next to it.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Tests pass and `mask all-fix` is clean
- [ ] Story status is updated in the story file and INDEX.md
- [ ] A retrospective is added
