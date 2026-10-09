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

## Retrospective

**Completed**: 2026-10-10

### Key Technical Learnings

#### What a subset may drop

- **Challenge**: pyftsubset's defaults keep hinting, many layout features and an
  empty `.notdef`. Each choice affects size, shaping or correctness. The
  Context's estimate also assumed `lnum`, which Inter does not have.
- **Solution**: measure each candidate (hinting on and off, default features
  against `kern,tnum`, with and without Latin Extended-A or Greek) and keep only
  what the pipeline uses. fontdue and resvg do not hint, so hinting was 23 KB
  (raw) of dead weight. `--notdef-outline` keeps the missing-glyph box visible:
  without it, an uncovered character vanishes silently.
- **Pattern**: prove a lossy asset transform is lossless where it matters. Here
  a test rasterises every subset character from both faces at two sizes and
  compares the bitmaps. That is stronger than "the goldens didn't move".

#### Testing that a browser uses an embedded font

- **Challenge**: Chromium draws "Inter, sans-serif" the same whether the
  `@font-face` works or a system Inter is installed, so a screenshot proves
  nothing.
- **Solution**: rename the family in copies of both SVGs to a name no system
  font has (`GupEmbedProbe`, falling back to serif). The embedded copy draws
  sans-serif Inter and the plain copy draws serif, so the difference is visible
  at a glance.
- **Pattern**: when a fallback looks the same as success, make the fallback look
  different.

#### resvg ignores `@font-face`

- The integration test decodes the data URL itself and gives resvg the decoded
  bytes. It checks the pixels match those from `INTER_REGULAR`, so it proves the
  payload is the right font, not that a renderer reads it. The browser probe
  covers the second half.

### Architectural Decisions

#### Keep the name "Inter"

- **Decision**: the subset is still called Inter, and it keeps the copyright,
  trademark and licence name records.
- **Reasoning**: Inter's OFL declares no Reserved Font Name, so OFL condition 3
  does not require a rename. Keeping the records means an embedded copy carries
  its own licence notice.
- **Trade-off**: none found. A font with an RFN would need a new family name in
  `mask subset-inter`.
- **Future**: any other bundled face needs the same check.

#### `SvgOptions` replaces `SvgTarget::with_font`

- **Decision**: one options struct (`font`, `embed_font`) instead of a second
  builder method next to `with_font`.
- **Reasoning**: one obvious way to configure the target. More SVG options
  (precision, ids) will join the struct.
- **Trade-off**: a breaking change for the one caller, which is acceptable
  pre-alpha.

#### A hand-written base64 encoder

- **Decision**: a 20-line RFC 4648 encoder in `svg.rs`, with test vectors. The
  `base64` crate is a dev-dependency only, for decoding in tests.
- **Reasoning**: keeps gup-core's runtime dependency set (and wasm size)
  unchanged for one encode call.

#### Wider coverage than the estimate

- **Decision**: 515 characters (Latin Extended-A, Greek, symbols) at 30.2 KB gz
  rather than Latin-1 at 23.8 KB gz.
- **Reasoning**: Central and Eastern European names and Greek-letter units are
  common in chart labels. 6 KB gz is cheap next to the 166 KB saved.
- **Future**: GUP-416 handles scripts beyond the subset without growing it.

### Development Workflow Insights

- **The disk ran out mid-story.** A full `cargo test` builds the root crate's
  test and example binaries (about 270 MB each), and with the shared build
  directory it filled the pool. The story only touched `gup-text` and
  `gup-core`, so `cargo test -p gup-text -p gup-core`, `mask all-check` (check
  only) and the wasm tasks were the right gate. The root crate does not depend
  on `gup-text`, so its goldens could not change. Check the dependency graph
  before running the whole workspace's tests, and check `df` before heavy
  builds.
- `mask subset-inter` needs the dev shell (`pyftsubset` is not on the host
  PATH); `nix develop -c mask subset-inter` regenerates the committed file byte
  for byte.
- `mask wasm-size` varies by about 0.1 KB between builds. Record sizes to one
  decimal place and don't read meaning into the last digit.

### Follow-up Stories

1. **GUP-416: gup-text font fallback** — a user-supplied fallback chain, so
   characters outside the subset draw from another font instead of as boxes,
   with nothing new bundled.
2. **GUP-405 amended** (not a new story): its Dependencies record what the
   subset keeps (`kern`, `tnum`; no `lnum` in Inter), and a task makes
   `SvgTarget` emit `font-variant-numeric: tabular-nums` for `tnum` runs so
   browsers match the measured widths.
3. Not written up: per-document subsetting of the embedded face (only the glyphs
   an SVG uses) would cut the 37 KB gz per file further. It needs a Rust
   subsetter in gup-core, so it is not worth it until someone embeds many SVGs
   in one page.
