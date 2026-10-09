# GUP-407: Subset the Bundled Inter; Optional SVG Font Embedding

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-10)
**Created**: 2026-10-06

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

- [x] The bundled default is an Inter subset with the coverage above, plus any
      characters gup-core's own formatters emit (for example the SI prefixes k,
      M, G and µ, and U+2212 minus). A test fails if a formatter emits a
      character the subset lacks.
- [x] The full face stays available, through a `gup-text` feature or
      `Font::from_bytes` with documented bytes, for non-Latin text. Missing
      glyphs are a documented, tested behaviour, not a silent box.
- [x] The subsetting is reproducible: a mask task with the exact pyftsubset
      command, and the OFL notice kept (the subset keeps the name "Inter"; the
      OFL Reserved Font Name rules are checked and recorded).
- [x] `SvgOptions { embed_font: bool }`: when set, `SvgTarget` emits an
      `@font-face` with the subset as a base64 data URL. A test checks that the
      SVG parses and the face is declared.
- [x] `mask wasm-size` shows the reduction, recorded in RFC-001.
- [x] Goldens are re-blessed only if glyph shapes change (they should not), and
      checked by eye.

## Technical Tasks

- [x] Subset with fonttools via a mask task; commit the subset and its recipe.
- [x] Add the full-face feature or loading path.
- [x] Add `SvgOptions` and embedding (a small base64 encoder or a crate).
- [x] Add a formatter coverage test.

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] Tests pass and `mask all-fix` is clean
- [x] Story status is updated in the story file and INDEX.md
- [x] A retrospective is added

## Implementation Summary

**Completed**: 2026-10-10

### What was delivered

- **The subset** (`crates/gup-text/fonts/Inter-Regular-Subset.ttf`, now
  `INTER_REGULAR` and `Font::inter`): 515 characters from
  `crates/gup-text/fonts/inter-subset.txt` (printable ASCII, Latin-1, Latin
  Extended-A, basic Greek, typographic punctuation and spaces, super- and
  subscript digits, currency, letterlike symbols, fractions, arrows, maths
  operators including U+2212, and legend shapes). It keeps GPOS `kern` and GSUB
  `tnum` (Inter has no `lnum`), drops hinting, and keeps the `.notdef` outline.
  61.3 KB raw, **30.2 KB gz** (was 411.6 KB, 198.3 KB gz). That is 6 KB gz over
  the Context's 23.8 KB estimate because the coverage is wider than Latin-1.
- **The full face**: `INTER_REGULAR_FULL` and `Font::inter_full()`, linked only
  when used. `Font::missing_glyphs(text)` lists characters the face lacks. They
  draw as Inter's `.notdef` box, which a test checks is visible.
- **Recipe**: `mask subset-inter` runs the exact `pyftsubset` command with the
  dev shell's fonttools 4.61.1 (added to `flake.nix`, pinned by `flake.lock`).
  It reproduces the committed file byte for byte (re-checked at completion).
  Inter's OFL declares no Reserved Font Name, so the subset keeps the name
  "Inter". It keeps the copyright, trademark and licence name records. The
  reasoning is recorded in `crates/gup-text/Cargo.toml` and RFC-001.
- **SVG embedding**: `SvgOptions { font, embed_font }` replaces
  `SvgTarget::with_font`. With `embed_font`, a scene that has text gets one
  `@font-face` in `<defs>` with the font as a base64 data URL. A small RFC 4648
  encoder avoids a new dependency. This adds 82 KB per file, 37 KB gzipped.
- **Formatter coverage**: a gup-core test sweeps the linear and log tick
  formatters from 1e-12 to 1e15. It fails if they emit a character the bundled
  subset lacks.
- **RFC-001**: a "GUP-407 findings" section with the subset contents, the WASM
  size table, SVG notes and proposed adjustments for GUP-405 and S8.

### WASM size (`mask wasm-size`)

| Build                   | Before (gz) | After (gz)    |
| ----------------------- | ----------- | ------------- |
| gup-core scatter        | 404.2 KB    | **238.4 KB**  |
| gup-core over bare wgpu | +362.4 KB   | **+196.7 KB** |
| bundled Inter           | 198.3 KB    | **30.2 KB**   |

### Tests (9 new)

- `gup-text` (`src/font.rs`): `subset_maps_exactly_the_listed_characters`,
  `subset_glyphs_are_the_full_faces` (all 515 characters rasterised at 11 and 16
  px from both faces, bitmaps compared), `typical_chart_text_is_covered`,
  `missing_glyphs_draw_as_a_visible_box`, `from_bytes_keeps_the_file`.
- `gup-core`: `scale::tick_labels_are_covered_by_the_bundled_font`,
  `svg::base64_matches_rfc_4648`, `svg::a_scene_without_text_embeds_no_font`,
  and `tests/svg.rs::embedded_font_is_the_bundled_subset`. That test parses the
  SVG, decodes the face, checks it is `INTER_REGULAR`, checks the rest of the
  document is unchanged, and checks that resvg draws the same pixels from the
  decoded font.
- `cargo test -p gup-text -p gup-core -- --test-threads=1`: 96 passed, 0 failed.
  `mask all-check` is clean. The root crate does not depend on `gup-text`, so
  its goldens cannot change; at the orchestrator's request, its test suite was
  left to CI (the build pool ran out of disk).

### Visual evidence

- The gup-core golden tests (`tests/svg.rs`, `msaa.rs`, `window_parity.rs`) pass
  unchanged. No re-bless was needed.
- Headless Chromium on copies of `scene_guides.svg` and
  `scene_guides_embedded.svg`, with the family renamed to `GupEmbedProbe` so
  that only the `@font-face` can supply it. The embedded file draws the title
  and tick labels (1G … 100k, 0 … 50000) in Inter. The plain file falls back to
  the serif default. resvg ignores `@font-face`, so resvg users still load
  `INTER_REGULAR` into their font database.
- `mask wasm-browser`: GUP PASS on SwiftShader with the subset.

### Key files

- `crates/gup-text/src/font.rs`, `crates/gup-text/src/lib.rs`,
  `crates/gup-text/fonts/{Inter-Regular-Subset.ttf,inter-subset.txt}`,
  `crates/gup-text/Cargo.toml`
- `crates/gup-core/src/svg.rs`, `crates/gup-core/tests/svg.rs`,
  `crates/gup-core/src/scale/mod.rs`
- `maskfile.md` (`subset-inter`, `wasm-size`), `flake.nix` (fonttools)
- `docs/planning/rfcs/RFC-001_Core_Architecture.md`
