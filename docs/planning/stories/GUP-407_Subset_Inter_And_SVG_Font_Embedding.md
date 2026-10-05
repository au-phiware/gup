# GUP-407: Subset the Bundled Inter; Optional SVG Font Embedding

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-06

## Context

`gup-text` bundles all of Inter Regular 4.1: 411.6 KB, or **198.3 KB gzipped**,
which is a large share of gup-core's wasm download
([GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md), RFC-001 "S3
findings"). Chart text uses a small character set. A pyftsubset subset covering
Basic Latin, Latin-1, dashes, quotes, arrows, math symbols, µ and €, with
`kern`, `tnum` and `lnum` kept for GUP-405, is **47.6 KB raw and 23.8 KB gz**.

GUP-401's `SvgTarget` references `font-family="Inter, sans-serif"` without
embedding the font, because embedding the full face would add ~550 KB of base64
to every SVG. A 48 KB subset makes optional embedding reasonable.

## User Story

> "As a user exporting charts to the web or to SVG, I want Gup's default font to
> be small enough to ship and to embed, so that downloads are small and SVGs
> look the same everywhere."

## Acceptance Criteria

- [ ] The bundled default is an Inter subset with the coverage above, plus any
      characters gup-core's own formatters emit (for example the SI prefixes k,
      M, G and µ, and U+2212 minus). A test fails if a formatter emits a
      character the subset lacks.
- [ ] The full face stays available, through a `gup-text` feature or
      `Font::from_bytes` with documented bytes, for non-Latin text. Missing
      glyphs are a documented, tested behaviour, not a silent box.
- [ ] The subsetting is reproducible: a mask task with the exact pyftsubset
      command, and the OFL notice kept (the subset keeps the name "Inter"; the
      OFL Reserved Font Name rules are checked and recorded).
- [ ] `SvgOptions { embed_font: bool }`: when set, `SvgTarget` emits an
      `@font-face` with the subset as a base64 data URL. A test checks that the
      SVG parses and the face is declared.
- [ ] `mask wasm-size` shows the reduction, recorded in RFC-001.
- [ ] Goldens are re-blessed only if glyph shapes change (they should not), and
      checked by eye.

## Technical Tasks

- [ ] Subset with fonttools via a mask task; commit the subset and its recipe.
- [ ] Add the full-face feature or loading path.
- [ ] Add `SvgOptions` and embedding (a small base64 encoder or a crate).
- [ ] Add a formatter coverage test.

## Dependencies

### Prerequisite Stories

- GUP-401 ✅ (`SvgTarget`, size harness)
- GUP-405 📋 (soft): keep the GPOS/`tnum` features it needs in the subset.

### Enables Stories

- Smaller wasm charts (with GUP-406) and self-contained SVG export.

## Testing Strategy

- Unit: coverage, embedding round trip through resvg.
- Visual: the existing goldens pass unchanged.

## Success Metrics

- The font share of the wasm build drops from ~198 KB gz to ~24 KB gz.

## Risk Assessment

- **Low**: OFL naming rules for modified fonts. Mitigation: check the Reserved
  Font Name clause before shipping, and rename the family if required.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Tests pass and `mask all-fix` is clean
- [ ] Story status is updated in the story file and INDEX.md
- [ ] A retrospective is added
