---
implementer: sonnet
implementer-reason:
  a context method over GUP-416's chain, one scene field and SVG family and
  embedding rules spelled out in Decisions, with a golden
touches:
  [
    crates/gup-text/src/system.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/context.rs,
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/plot.rs,
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/src/svg.rs,
    tests/golden/,
  ]
---

# GUP-580: Fallback Fonts in gup-core and SVG

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-31

## Context

[GUP-416](GUP-416_Gup_Text_Font_Fallback.md) gives `gup-text` a fallback chain
(`TextSystem::add_fallback`, `Font::with_fallbacks`) whose fonts parse lazily.
gup-core has no way to reach it. `Context::text()` is crate-private, and
`gup_text::Font` is not re-exported, although `SvgOptions::font` already has
that type. `SvgTarget` names one family (`font-family="Inter, sans-serif"`) and
embeds only the primary faces. A chart with "東京" in its title would draw
correctly on the GPU and as boxes or a system font in the SVG.

The owner confirmed the entry point's name on 2026-10-24:
`Context::add_fallback_font`.

## Read First

- `crates/gup-core/src/context.rs` lines 170–190 (the `text` mutex and lock
  order) and 520–535 (`text()`).
- `crates/gup-core/src/scene.rs` lines 26–90: `Scene`, `Scene::new`,
  `Scene::guides`.
- `crates/gup-core/src/plot.rs` around line 2432:
  `let mut scene = Scene::new(width, height, th.background);`.
- `crates/gup-core/src/svg.rs` lines 95–145 (`SvgOptions`) and 197–226 and
  340–380 (`write`: `@font-face` and the `ItemKind::Text` branch).
- `crates/gup-core/tests/svg.rs` lines 184–300: the embedding tests.
- `crates/gup-core/tests/scatter_png.rs` lines 1–75: a golden with the SVG
  parity check.
- GUP-416's Decisions: `with_fallbacks`, `has_own_glyph`, `is_parsed`, the test
  face `crates/gup-text/tests/fonts/NotoSansJP-Test.ttf`.

## User Story

> "As a visualization developer, I want to add a font to my context for the
> characters Inter lacks, so that labels in any script draw on screen and in the
> SVG I export."

## Acceptance Criteria

### AC1: The entry point

- [ ] `gup_core::Font` (and so `gup::Font`) re-exports `gup_text::Font`.
      `Context::add_fallback_font(&self, font: Font)` and
      `Context::fallback_fonts(&self) -> Vec<Font>` are public and documented.
- [ ] Doctest on `add_fallback_font`, running headless:
      `cx.add_fallback_font(Font::inter_full())`. A plot titled "Київ" then
      resolves with no missing glyphs:
      `Font::inter().with_fallbacks(&cx.fallback_fonts()).missing_glyphs("Київ")`
      is empty.
- [ ] A fallback added after a resolve takes effect on the next `resolve`, with
      nothing else changed. Integration test: a plot titled "東京" with a text
      layer labelled "東京" is resolved, then the jp face is added, then the
      plot is resolved again. The second resolve is a full one, and both the
      title's and the label's `PlacedText` widths change: from two `.notdef`
      boxes to two jp glyphs, whose advance is one em each. Assert that each ink
      width after the change is within 12% of twice its run's size. The ink of
      "東京" in the test face is about 1.9 em.

### AC2: Lazy through gup-core

- [ ] Integration test: a context with the jp test face as fallback renders a
      scatter whose text Inter covers. `jp.is_parsed()` stays `false`. After a
      title with "東京", it is `true`.

### AC3: SVG

- [ ] `Scene` has `pub fallback_fonts: Vec<gup_text::Font>`. `Scene::new` leaves
      it empty, `Plot::resolve` fills it from `cx.fallback_fonts()`, and
      `Scene::guides` copies it.
- [ ] `SvgTarget` writes
      `font-family="Inter, &quot;<fallback 1>&quot;, …, sans-serif"`. Each
      fallback family, as named in Decisions, is CSS-quoted with `css_string`
      and XML-escaped. Its `baseline_origin` comes from
      `font.with_fallbacks(&scene.fallback_fonts)`, so its anchor rounding
      matches the GPU's.
- [ ] With `embed_font`, each fallback that some run of the scene uses gets an
      `@font-face` with its family name and data. A fallback no run uses is not
      embedded. Test in `tests/svg.rs`: fallbacks `[Font::inter_full(), jp]` and
      a title "東京" embed jp and not Inter full. Neither is embedded for a
      title "Zürich".

### AC4: Golden (user-visible)

- [ ] `crates/gup-core/tests/fallback_fonts_png.rs` uses its own
      `Context::new_blocking()`, never `shared()`, so no other test sees the
      fallbacks. It draws the scatter of `scatter_png.rs` titled "Київ — 東京 —
      Zürich", with fallbacks `[Font::inter_full(), jp]`. Golden
      `gup_core/fallback_title`, harness checks plus
      `svg_marks::assert_matches`. The test first asserts that
      `Font::inter().missing_glyphs(title)` is
      `['К', 'и', 'ї', 'в', '東', '京']`, so the golden shows the fallback at
      work.
- [ ] Read the PNG by eye. The title shows Cyrillic, CJK and Latin with no
      boxes, on one baseline, centred, and the plot rect matches
      `scatter.png`'s.

### AC5: Size

- [ ] `mask wasm-size`: at most 1,000 B gz growth (`svg` is off in the WASM
      build). Report the figures.

## Decisions

- **Signatures** (`context.rs`):

  ```rust
  /// Add `font` to the end of this context's fallback chain: characters the
  /// bundled Inter lacks draw from the first fallback that has them. Parsed on
  /// first use. Affects every plot drawn with this context (all users of
  /// `Context::shared()`), from its next full resolve.
  pub fn add_fallback_font(&self, font: Font) { self.text().add_fallback(font) }
  /// The fallback chain, in order.
  pub fn fallback_fonts(&self) -> Vec<Font> { self.text().fallbacks().to_vec() }
  ```

  It takes `&self` and the text lock, so it works on `Context::shared()`.
  `pub use gup_text::Font;` goes in `lib.rs` beside `pub use svg::…`, not in the
  prelude.

- **Cached resolves and measured labels**: gup-text gains
  `TextSystem::generation(&self) -> u64`. It starts at 0 and `add_fallback`
  adds 1. In gup-core, `plot.rs`'s `CacheKey` gains `fonts: u64`, read as
  `cx.text().generation()` in the `key` closure of `Plot::resolve`. That is a
  short lock, never taken together with the pipeline lock. `text_layer.rs`'s
  `Measured` gains `fonts: u64`, and `prepare` re-measures when it or the size
  differs. So a fallback added at any time applies from the next `resolve`.
- **SVG family names**: the primary is written as today (`Inter`). Each fallback
  is written as its `Font::name()`. If that name equals the primary's or an
  earlier fallback's, append " " and its 1-based chain index ("Inter 1").
  `Font::inter_full()` is named "Inter", and two `@font-face` rules for one
  family without `unicode-range` would hide the subset. The same name is used in
  `font-family` and its `@font-face`.
- **"Used"**: a fallback is used if some text run has a character that the run's
  primary (`options.font`, or `options.semibold_font` for SemiBold, including
  its internal Regular fallback) lacks, and this fallback is the first in
  `scene.fallback_fonts` with `has_own_glyph` for it. Computed once per `write`.
- **Not done**: subsetting fallbacks for embedding (a 9.6 MB Noto Sans JP embeds
  whole, which `SvgOptions::embed_font`'s doc now says) and per-run
  `font-family` lists.

## Escalation Triggers

- Taking the text lock in `Plot::resolve`'s cache check conflicts with a lock
  already held there (the debug lock-rank check panics): report it rather than
  reorder locks.
- The SVG parity check of `fallback_title` fails outside text regions.
- Any golden other than `fallback_title` changes.

## Out of Scope

- The chain itself, laziness and validation: GUP-416.
- A builder-level font option (`gup::scatter(..).font(..)`) and swapping the
  primary font through `Context`. Neither is planned.

## Technical Tasks

- [ ] Re-export; `add_fallback_font`, `fallback_fonts` and the doctest.
- [ ] `TextSystem::generation`; `CacheKey::fonts`; `Measured::fonts`; the
      late-fallback test.
- [ ] `Scene::fallback_fonts`; fill it in `resolve_full`; copy it in `guides`.
- [ ] SVG family list, naming rule, `with_fallbacks` origin, used-font
      embedding; the `tests/svg.rs` cases.
- [ ] `fallback_fonts_png.rs` golden and lazy test; read the PNG.
- [ ] `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-416: gup-text font fallback chain 📋 (hard).
- GUP-405: kerning and glyph-index layout 📋 (through GUP-416).

### Enables Stories

- International labels in every chart and export without a larger default font.

## Testing Strategy

- **Unit tests**: the SVG naming rule (duplicate names get an index).
- **Integration tests**: doctest; late fallback; laziness; SVG family and
  embedding cases.
- **Visual validation**: `fallback_title` read by eye.
- **Performance**: none (no fallback parse for covered text, from AC2).

## Success Metrics

- [ ] A chart with a CJK title draws it on screen and in its SVG with one call.

## Risk Assessment

- **Low**: `Context::shared()` users see one another's fallbacks. _Mitigation_:
  documented; tests use their own contexts.
- **Low**: a user's whole CJK font embedded in an SVG is large. _Mitigation_:
  documented on `embed_font`, and embedding stays off by default.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1` and
      `cargo test --doc -p gup`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`fallback_title.png`)
- [ ] `mask wasm-browser`: not needed (no layout or wasm scene change); say so
- [ ] WASM size: `mask wasm-size` scatter gz bytes
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
