---
implementer: sonnet
implementer-reason:
  lazy parsing, the chain order, validation and the test face are fixed in
  Decisions; gup-text only, on GUP-405's per-face layout
touches: [crates/gup-text/]
---

# GUP-416: gup-text Font Fallback Chain with Lazy Parsing

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-10 (decision-complete rewrite 2026-10-31)

## Context

[GUP-407](GUP-407_Subset_Inter_And_SVG_Font_Embedding.md) made a 515-character
Inter subset the bundled default. A character outside it, such as Cyrillic, CJK
or emoji, draws as Inter's `.notdef` box. `TextSystem` holds one font per
weight. The only fallback is internal: the SemiBold subset draws what it lacks
from the Regular subset (`Font::fallback`, `face_for` in `font.rs`). Data labels
are user data, so mixed scripts are normal. Shipping more fonts is not the
answer, because GUP-407 cut the font's share of the WASM download from 198 KB to
30 KB gz.

This story lets a user add fonts that are tried in order for characters the
primary lacks. Fonts are parsed lazily, which matters. Measured on 2026-10-31
(release, native), fontdue parses the 9.6 MB Noto Sans JP in 259 ms, but
`ttf_parser::Face::parse` reads its table directory in 2.6 µs, and 16 cmap
lookups take 5.7 µs. A program that adds a CJK fallback "just in case" must not
pay 259 ms, or about three times that in a browser, unless a label needs it.

gup-core's entry point (`Context::add_fallback_font`), the SVG side and the
golden are [GUP-580](GUP-580_Fallback_Fonts_In_Gup_Core_And_Svg.md).

## Read First

- `crates/gup-text/src/font.rs` (whole, about 430 lines after GUP-405): `Font`,
  `Data`, `Face`, `parse`, `inter_semibold`, `has_glyph`, `has_own_glyph`,
  `missing_glyphs`, `face_for` and the tests.
- `crates/gup-text/src/system.rs` lines 180–310: `TextSystem` fields, the
  constructors, `font`, `font_weight` and `measure*`.
- `crates/gup-text/src/layout.rs`: the shaping function GUP-405 added (glyphs
  are looked up per char through `face_for`, and kerning stops at a face
  change).
- `crates/gup-text/tests/draw_into_pass.rs` lines 140–220: the ink-inside-bounds
  check.
- `maskfile.md` `## subset-inter` (lines 153–190) for the `pyftsubset` flags.

## User Story

> "As a Gup user labelling data in several scripts, I want characters the
> default font lacks to come from a font I supply, so that my labels are legible
> and the rest of my text keeps the default look."

## Acceptance Criteria

### AC1: The chain

- [ ] `TextSystem::add_fallback(&mut self, font: Font)` appends to the chain.
      Each character draws from the first of these that has its glyph: the
      weight's own face, that face's internal fallback (SemiBold to Regular),
      then the added fonts in the order they were added. A character none of
      them has draws as the primary's `.notdef` box, as now.
- [ ] Unit test: with fallbacks `[Font::inter_full(), jp]` (jp being the test
      face below), "Київ — 東京 — Zürich" has no missing glyphs
      (`font_weight(Regular).missing_glyphs(..)` is empty). Its glyphs come from
      Inter subset, Inter full and jp, in order of the characters. With the
      chain `[jp, Font::inter_full()]`, "東" still comes from jp and "К" from
      Inter full, because the subset test face has no Cyrillic.
- [ ] Kerning and `tnum` stop at a face change (no pair across faces), as they
      do today.

### AC2: Lazy

- [ ] `Font::from_bytes` does not run fontdue. A
      `#[doc(hidden)] pub fn     is_parsed(&self) -> bool` reports whether the
      glyphs were parsed. Test: a `TextSystem` with fallback jp lays out,
      measures and finds the ink bounds of "Zürich 1.5k −3 °C": `jp.is_parsed()`
      is `false`. It then lays out "東京": `true`.
- [ ] `Font::from_bytes("Nope", b"not a font")` is still an `Error::Font` naming
      "Nope" (the existing test passes). A font whose cmap maps a code point
      past its glyph count is an `Error::Font` at `from_bytes`: there is a unit
      test with a hand-patched copy of the test face.

### AC3: Measuring agrees with drawing across faces

- [ ] `draw_into_pass` gains a case: "Київ — 東京 — Zürich" at 16 px with the
      chain above. All its ink lies inside `ink_bounds`. "東" and "京" each draw
      a bitmap different from `.notdef`'s at that size. Every glyph sits on the
      primary's baseline: for each `PlacedGlyph` of the run (from `walk`),
      `g.y + g.metrics.height as f32 + g.metrics.ymin as f32` is the same value
      for "Z", "К" and "東".
- [ ] The atlas holds "東" from jp and "Z" from Inter under different `FaceId`s.
      Adding the same fallback twice adds no atlas entries for glyphs already
      drawn.

### AC4: Size

- [ ] `mask wasm-size`: a default `TextSystem` (no fallbacks) changes the
      scatter by at most 2,000 B gz. Report the figures.

## Decisions

- **API** (all in gup-text):
  - `TextSystem::add_fallback(&mut self, font: Font)` and
    `TextSystem::fallbacks(&self) -> &[Font]`.
  - `Font::with_fallbacks(&self, fallbacks: &[Font]) -> Font`, public: this
    font, then its own fallbacks, then `fallbacks`. gup-core's SVG target uses
    it (GUP-580). Only each fallback's own face is consulted. A fallback's own
    fallbacks are not followed.
  - `#[doc(hidden)] pub fn is_parsed(&self) -> bool`.
  - `Font::from_bytes(name, data)` keeps its signature and validates as below.
  - The 2026-10-24 note's example used a one-argument `from_bytes`. The real
    signature is `Font::from_bytes(name: &str, data: &[u8])`.
- **Storage.** `Font` becomes:

  ```rust
  pub struct Font {
      name: Arc<str>,
      data: Data,
      /// GUP-405's `Face` (fontdue glyphs and `OtLayout`), parsed on first use.
      face: Arc<OnceLock<Face>>,
      /// Tried in order for characters this face lacks; replaces `fallback`.
      fallbacks: Arc<[Font]>,
  }
  ```

  `inter_semibold()` is the SemiBold subset with `fallbacks = [Font::inter()]`.
  The bundled statics stay `OnceLock<Font>` statics. They now parse their glyphs
  on first use, not at first call.

- **Validation at `from_bytes`** makes the deferred parse infallible.
  `ttf_parser::Face::parse(data, 0)` must succeed. Every cmap mapping
  (`face.tables().cmap` subtables, `codepoints` then `glyph_index`) must be
  below `number_of_glyphs()`, otherwise
  `Error::Font { reason: "its cmap maps U+XXXX to glyph G of N" }`. fontdue
  fails on nothing else but a GSUB substitute past the glyph count. So the
  deferred parse tries `FontSettings::default()`, then
  `FontSettings { load_substitutions: false, ..Default::default() }`. After
  validation the second cannot fail. Write it as
  `expect("validated in from_bytes")`.
- **Coverage without parsing**: `has_own_glyph(c)` uses the parsed fontdue face
  when `is_parsed()`, and otherwise
  `ttf_parser::Face::parse(self.data(), 0).is_ok_and(|f| f.glyph_index(c).is_some())`.
  `face_for(c)` picks the face, then forces its parse (`get_or_init`).
- **The chain in `TextSystem`**: keep `fonts: [OnceLock<Font>; 2]` (the base
  fonts) and add `fallbacks: Vec<Font>` and `chained: [OnceLock<Font>; 2]`.
  `font_weight(w)` returns
  `chained[w].get_or_init(|| base(w).with_fallbacks(&self.fallbacks))`.
  `add_fallback` pushes, then resets both `chained` cells (`OnceLock::take`).
  Every existing caller (`measure*`, `ink_bounds`, `layout`, gup-core's
  `font_weight` users) then sees the chain with no change.
- **Line metrics** come from the primary. A fallback glyph sits on the primary's
  baseline, at its own metrics (`ymin`, `height`) at the run's pixel size, as
  SemiBold-to-Regular does now. Fallbacks serve both weights. SemiBold text
  falls back through Regular, then the added fonts.
- **Test face**: `crates/gup-text/tests/fonts/NotoSansJP-Test.ttf`, about 2.6
  KB, with `NotoSansJP-OFL.txt` (the OFL 1.1 text with the font's name ID 0
  line: "(c) 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name
  'Source'."). It is not bundled; tests `include_bytes!` it.
  `crates/gup-text/tests/fonts/README.md` records these commands (fonttools from
  the dev shell; the Noto Sans JP variable font from nixpkgs' `google-fonts`,
  found with `fc-list | grep NotoSansJP`):

  ```bash
  fonttools varLib.instancer --update-name-table -o target/NotoSansJP-Regular.ttf "<path>/NotoSansJP[wght].ttf" wght=400
  pyftsubset target/NotoSansJP-Regular.ttf --text="東京" --layout-features=kern \
    --no-hinting --notdef-outline --name-IDs=0,1,2,3,4,5,6,7,13,14 \
    --output-file=crates/gup-text/tests/fonts/NotoSansJP-Test.ttf
  ```

  Checked on 2026-10-31: fontdue parses it (3 glyphs), "東" and "京" have ink,
  and its line ascent at 16 px is 18.56. Inter's is about 15.5, which is why
  line metrics come from the primary.

- **Where code goes**: `font.rs` (storage, validation, laziness, chain lookup),
  `system.rs` (chain), tests in `font.rs`, `system.rs` and
  `tests/draw_into_pass.rs`. No new module.

## Escalation Triggers

- A gup-core test or golden changes. This story must not change output without
  fallbacks: stop and report it.
- fontdue rejects a font that passed validation. That means another failure mode
  exists: report the font and the reason.
- AC4's 2,000 B gz is exceeded.
- Some `TextSystem` path measures with the base font instead of the chained one
  (an `ink_bounds` disagreement in AC3) and needs a signature change to fix.

## Out of Scope

- `Context::add_fallback_font`, the `gup_core::Font` re-export, SVG
  `font-family` and embedding, and the gup-core golden: GUP-580.
- Bundling any fallback font. Subsetting a user's font for SVG embedding.
- Script itemisation, right-to-left text, emoji colour fonts.

## Technical Tasks

- [ ] Make the test face and README; commit them with the licence.
- [ ] `Font`: lazy `face`, `fallbacks`, validation, `has_own_glyph`,
      `with_fallbacks`, `is_parsed`; update the font tests (`Arc::ptr_eq` on the
      `face` cell).
- [ ] `TextSystem`: `fallbacks`, `chained`, `add_fallback`, `fallbacks()`.
- [ ] Unit tests (AC1, AC2), the `draw_into_pass` case (AC3), `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-407: Subset the bundled Inter ✅
- GUP-405: gup-text kerning and glyph-index layout 📋 (hard): `Face`,
  `FaceId`-keyed atlas and per-face kerning.

### Enables Stories

- GUP-580: fallback fonts in gup-core and SVG.

## Testing Strategy

- **Unit tests**: chain order, laziness (`is_parsed`), validation errors, atlas
  keys per face.
- **Integration tests**: `draw_into_pass` with a three-face run.
- **Visual validation**: the AC3 pixel checks here; the read-by-eye golden is
  GUP-580's.
- **Performance**: no fallback parse for covered text (AC2).

## Success Metrics

- [ ] Mixed-script labels draw without boxes when a fallback covers them.
- [ ] A covered chart never parses its fallbacks.

## Risk Assessment

- **Medium**: deferring Inter's parse moves its cost (about 5 ms for the subset)
  from `Font::inter()` to the first measure. _Mitigation_: `TextSystem` already
  deferred it, so nothing a user sees moves.
- **Low**: `has_own_glyph` on an unparsed fallback re-parses the table directory
  (about 3–7 µs) for each missing character until the fallback is used.
  _Mitigation_: only characters the primary lacks reach it.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-text -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output: AC3's pixel checks (no gup-core output changes)
- [ ] `mask wasm-browser`: not needed (no layout, input or wasm code change);
      say so
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
