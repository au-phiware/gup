---
implementer: sonnet
implementer-reason:
  data structures, lookup order and budgets fixed in Decisions and checked
  against rustybuzz on every pair of the bundled subsets; an oracle test and a
  lossless-refactor check guard the rest
touches:
  [
    crates/gup-text/,
    crates/gup-core/src/scene.rs,
    crates/gup-core/src/text_layer.rs,
    crates/gup-core/wasm-size/,
    tests/golden/,
    Cargo.lock,
  ]
---

# GUP-405: gup-text OpenType Kerning and Glyph-Index Layout

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress Planned **Created**: 2026-10-06 (decision-complete rewrite 2026-10-31)

## Context

[GUP-400](GUP-400_RFC_001_S2_Extract_Gup_Text.md) extracted `gup-text`, and
GUP-407 made a subset of Inter its bundled face. Layout walks the run's `char`s
(`Font::walk`, `crates/gup-text/src/layout.rs`) and kerns through fontdue's
`horizontal_kern`, which reads only the legacy `kern` table. Inter 4 keeps its
kerning in GPOS and has no `kern` table, so every run is unkerned today. "AV" in
the Regular subset is 2,826 font units unkerned and 2,686 kerned. Even "11"
kerns (1,732 to 1,666).

The subsets keep two OpenType features: GPOS `kern` and GSUB `tnum` (GUP-407,
`mask subset-inter`). This story applies `kern` to every run, lays text out by
glyph index instead of by `char`, and adds a `Features` set to `Run` with `TNUM`
in it. [GUP-579](GUP-579_Tabular_Figures_For_Tick_Labels.md) then turns `TNUM`
on for tick labels. The story was planned around `rustybuzz`. It was re-planned
on 2026-10-31 after measuring the cost. Built for `wasm32` like the size harness
(`opt-level = "z"`, LTO), fontdue alone is 92,027 B gz. Adding rustybuzz 0.20.1
makes it 302,560 B gz (+210,533). gup-core has 90,600 B of headroom under the
400,000 B ceiling over bare wgpu (`wasm.over_wgpu.gz_bytes` 309,400), so
rustybuzz cannot ship on the default build. A rustybuzz behind an off-by-default
feature would leave the web build unkerned.

We don't need all of rustybuzz for these fonts. fontdue already links
`ttf-parser` 0.21.1 with its `opentype-layout` feature, which parses GSUB and
GPOS. A walk over GPOS pair adjustments and GSUB single substitutions on that
parser cost +4,492 B gz in the same build. It gives exactly rustybuzz's total
advance on all 530,450 two-character runs of the Regular subset and all 19,208
of the SemiBold subset, with and without `tnum`. This is true only if lookups
are summed in lookup-index order. Applying only the first lookup that matches
gets "−42 °C" wrong by 23 units. The measurement crate was a scratch build and
is not committed. Its procedure is the Oracle test below.

## Read First

- `crates/gup-text/src/layout.rs` lines 100–262: `Run`, `Weight`, `PlacedGlyph`,
  and `Font::measure`, `ink_bounds`, `baseline_origin`, `origin` and `walk`.
- `crates/gup-text/src/font.rs` lines 40–245: `Font`, `Data`, `parse`,
  `has_glyph`, `line_metrics` and `face_for` (the SemiBold-to-Regular fallback).
- `crates/gup-text/src/atlas.rs` lines 45–140: the `glyphs` key and `glyph`.
- `crates/gup-text/src/system.rs` lines 280–345: `font_weight`, `measure*`,
  `ink_bounds`, `atlas_glyphs` and `layout`.
- `crates/gup-text/src/error.rs` (`AtlasFull`).
- `crates/gup-core/src/scene.rs` lines 307–352: `TextRole`, `TextRun`,
  `layout_run`.
- `crates/gup-core/src/text_layer.rs` lines 356–392 (`prepare`) and 660–720 (the
  `a_translated_ink_box_equals_the_laid_out_one` test, GUP-555, which must keep
  passing).
- ttf-parser 0.21.1 API (in the cargo registry): `tables/gpos.rs`
  (`PairAdjustment`, `PairSets`, `ClassMatrix`, `ValueRecord::x_advance`),
  `tables/gsub.rs` (`SingleSubstitution`), `ggg/mod.rs` (`Coverage`,
  `ClassDefinition`), `ggg/layout_table.rs` (`LayoutTable::features`,
  `lookups`).
- `tests/golden/README.md` "Blessing".

## User Story

> "As a Gup user reading a chart, I want titles and labels set with the font's
> kerning, so that text looks like a well-set UI face and the SVG a browser
> draws (which kerns) matches the PNG."

## Acceptance Criteria

### AC1: Glyph-index layout, losslessly

- [ ] `Font::walk` and `Font::measure_with` both go through one private shaping
      function. It maps chars to glyph indices (substituting `tnum` when
      requested), advances by `fontdue::Font::metrics_indexed` and kerns between
      neighbours drawn by the same face.
- [ ] With `Features::NONE`, layout matches today's `char` layout: a unit test
      compares `walk` positions and `measure` widths with the old formula
      (`face_for(c).metrics(c, px)` advances, no kerning) for the eight strings
      in `font::tests::typical_chart_text_is_covered`, in both weights.
- [ ] Checked once and recorded in the retrospective: with `layout_run` and the
      text layer passing `Features::NONE`, `cargo test -p gup-core` passes with
      no golden re-blessed. Then switch them to `Features::default()`.

### AC2: Kerning

- [ ] `Features::KERN` (the default) applies GPOS `kern` pair adjustments, or
      fontdue's legacy `kern` table for a face that has no GPOS `kern`. A unit
      test: "AV" and "To" in the Regular subset at 16 px measure narrower with
      `KERN` than with `NONE`.
- [ ] Oracle test `crates/gup-text/tests/shaping_oracle.rs`, with `rustybuzz` as
      a dev-dependency: for both bundled subsets, every two-character string of
      printable ASCII (U+0020–U+007E, 9,025 strings), plus "−42 °C", "2024–Q1",
      "Zürich", "Revenue (USD)" and "AVAST Today", with `Features::KERN` and
      with `KERN | TNUM`. `measure_with(s, upem, f).width` at
      `size = units_per_em` (2048 for Inter) equals the sum of rustybuzz's
      `x_advance`s exactly (`==` on `f32`), with rustybuzz's `tnum` set to
      match.
- [ ] Seeded failure, recorded in the retrospective: applying only the first
      matching lookup makes the oracle test fail on "−42 °C".

### AC3: `tnum` is available (not yet used by gup-core)

- [ ] `measure_with(d, 16.0, Features::KERN | Features::TNUM)` gives one advance
      for every digit `0`–`9`, and "1000" and "1111" measure the same width, in
      both bundled weights.
- [ ] The atlas keys glyphs by face, glyph index and size: a unit test lays out
      "1" with and without `TNUM`, and `atlas_glyphs()` counts two entries.
      Laying out "1" again with either set adds none.

### AC4: Drawing still agrees with measuring

- [ ] `crates/gup-text/tests/draw_into_pass.rs` passes. A new case draws "AVATAR
      To 11" and finds all of its ink inside `ink_bounds`.
- [ ] `text_layer::tests::a_translated_ink_box_equals_the_laid_out_one`
      (GUP-555) passes unchanged.

### AC5: Goldens (user-visible), with a cap

- [ ] Re-bless with `GUP_BLESS=1`. Before blessing, every golden failure is a
      `golden` check failure: no structural check (`text_present`,
      `marks_confined`, …) and no `svg_marks` parity check fails.
- [ ] Only goldens that draw text change. Run `git diff --stat tests/golden`,
      give the count in the retrospective, and check that no listed case lacks
      text.
- [ ] Read `scatter.png`, `secondary_y.png`, `text_annotations.png` and
      `tick_format_currency.png` before and after, and their pre-bless
      `*.diff.png`. Describe what moved: kerned pairs in titles and labels,
      margins changed by at most 1 px.

### AC6: Size and speed

- [ ] `mask wasm-size`: `wasm.scatter.gz_bytes` grows by at most 6,000 B gz over
      the Start commit's measurement (recorded 341,760; the +2% row tolerance is
      348,595). Report both figures. The orchestrator re-records the row.
- [ ] `crates/gup-core/wasm-size/scatter/Cargo.lock` gains `ttf-parser` only in
      gup-text's dependency list: no new package, and one `ttf-parser` 0.21.1.
- [ ] `crates/gup-text/tests/layout_cost.rs` (`#[ignore]`, run with
      `cargo test -p gup-text --release --test layout_cost -- --ignored --nocapture`)
      is committed first, so it also runs on the Start tree. It measures 26 tick
      labels (`"0"`, `"100"`, …, `"2500"`) and the title "Zoom benchmark
      (100,000 points)" at 11 px, `Font::inter().measure` four times each per
      frame, over 1,000 frames after 10 warm-up frames. Per-frame time rises by
      at most 30 µs from the Start tree. Both figures go in the retrospective.
- [ ] `mask wasm-browser` passes (margins may move by a pixel).

## Decisions

- **No rustybuzz at runtime.** It costs +210,533 B gz (see Context). Kerning and
  `tnum` come from `ttf-parser` 0.21, which fontdue already links. gup-text adds
  `ttf-parser = { version = "0.21", default-features = false, features = ["opentype-layout"] }`,
  the same features fontdue enables, so the unified crate does not change.
  `rustybuzz = "0.20"` is a `[dev-dependencies]` entry only, already in
  `Cargo.lock` at 0.20.1 through usvg. Nothing goes behind a feature flag:
  kerning costs about 5 KB.
- **`Features`** (new, `crates/gup-text/src/layout.rs`, re-exported from the
  crate root):

  ```rust
  /// OpenType features a run asks for. The default is [`Features::KERN`].
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  pub struct Features(u8);
  impl Features {
      pub const NONE: Self = Self(0);
      /// GPOS `kern` pair adjustments, or the legacy `kern` table.
      pub const KERN: Self = Self(1);
      /// GSUB `tnum`: tabular (equal-advance) figures.
      pub const TNUM: Self = Self(2);
      pub const fn union(self, other: Self) -> Self { Self(self.0 | other.0) }
      pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
  }
  impl Default for Features { fn default() -> Self { Self::KERN } }
  impl std::ops::BitOr for Features { type Output = Self; fn bitor(self, o: Self) -> Self { self.union(o) } }
  ```

  A bit set rather than a slice: it is `Copy` and `Hash` with no lifetime, which
  suits GUP-555's per-label cache. This replaces the 2026-10-24 note's
  `&[Feature]`.

- **`Run` gains `pub features: Features`.** Update every literal: the crate doc
  example in `lib.rs`, the gup-text tests, `TextRun::layout_run` in gup-core
  `scene.rs`, and the two `gup_text::Run` literals in `text_layer.rs`. gup-core
  sets `Features::default()` everywhere (AC1 checks `NONE` first). TNUM in
  gup-core is GUP-579.
- **Measuring API**:
  `Font::measure_with(&self, text: &str, size: f32, features: Features) -> TextMetrics`
  and
  `TextSystem::measure_with(&self, text: &str, size: f32, weight: Weight, features: Features) -> TextMetrics`.
  `measure` and `measure_weight` keep their signatures and use
  `Features::default()`. `ink_bounds`, `baseline_origin` and `layout` read
  `run.features`.
- **Faces.** `font.rs` gains `pub(crate) struct Face`:

  ```rust
  pub(crate) struct Face {
      pub id: FaceId,               // from a static AtomicU32, set at parse
      pub data: Data,               // a clone of the font's bytes (Static or Arc)
      pub glyphs: fontdue::Font,
      pub layout: crate::opentype::OtLayout,
  }
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  pub(crate) struct FaceId(u32);
  ```

  `Font` holds `face: Arc<Face>` in place of `Arc<fontdue::Font>`. `face_for`
  returns `&Face`. `Font::face()`, used by the tests, returns
  `&self.face.glyphs`. Keep fontdue's default `FontSettings`:
  `load_substitutions: true` is what loads the `tnum` glyphs, so
  `metrics_indexed` and `rasterize_indexed` work for them.

- **`OtLayout`** (new `crates/gup-text/src/opentype.rs`):

  ```rust
  pub(crate) struct OtLayout {
      units_per_em: f32,
      /// GSUB `tnum` single substitutions, flattened: glyph -> tabular glyph.
      tnum: HashMap<u16, u16>,
      /// Lookup indices of every GPOS feature tagged `kern`, deduplicated, ascending.
      kern_lookups: Vec<u16>,
      /// Pair results so far, in font units (0 = the pair does not kern).
      pairs: Mutex<HashMap<(u16, u16), i16>>,
  }
  impl OtLayout {
      pub(crate) fn parse(face: &ttf_parser::Face<'_>) -> Self;
      pub(crate) fn tabular(&self, glyph: u16) -> u16;        // identity when absent
      pub(crate) fn has_gpos_kern(&self) -> bool;             // !kern_lookups.is_empty()
      /// Kerning between `a` and `b` in pixels at `px`; parses `data` with
      /// `ttf_parser::Face::parse(data, 0)` on a cache miss.
      pub(crate) fn kern(&self, data: &[u8], a: u16, b: u16, px: f32) -> f32;
  }
  ```

  - `tnum`: use the lookups of every GSUB feature tagged `tnum`. For each
    `SubstitutionSubtable::Single`, `Format1 { coverage, delta }` maps each
    covered glyph `g` to `g + delta` (wrapping `i32` to `u16`).
    `Format2 { coverage, substitutes }` maps the glyph at coverage index `i` to
    `substitutes.get(i)`. Iterate `Coverage::Format1 { glyphs }` and
    `Format2 { records }` (each record covers `start..=end`, coverage indices
    from `value` upward). Other subtable types are ignored.
  - `kern`, the procedure checked against rustybuzz: start from total = 0. For
    each lookup in `kern_lookups`, in ascending order, take its subtables as
    `PositioningSubtable`. For the first `Pair(p)` whose
    `p.coverage().get(GlyphId(a))` is `Some(ci)`, find a value.
    `Format1 { sets }` gives `sets.get(ci)?.get(GlyphId(b))`.
    `Format2 { classes, matrix }` gives
    `matrix.get((classes.0.get(GlyphId(a)), classes.1.get(GlyphId(b))))`. If
    there is a value, add its first `ValueRecord::x_advance` to the total and go
    on to the next lookup. If there is none, try the lookup's next subtable.
    Second-glyph values, placements, device tables and lookup flags are ignored;
    the oracle shows none of them matters for the bundled subsets.
  - The kerning features of every script are used (their union), because runs
    are not split by script. This matches rustybuzz on Inter, whose `kern`
    features all list lookups 0 and 1.
  - The cache is a `std::sync::Mutex` locked once per pair (on poison, take
    `into_inner`). It is cleared when it would exceed 65,536 entries. On a miss,
    compute the pair from a freshly parsed `ttf_parser::Face` (a parse is 3–7
    µs) and insert it. Pixels are `units as f32 * px / units_per_em`.

- **Kerning between faces.** Kern only between neighbours whose `Face::id`s are
  equal, as now. Use GPOS when `face.layout.has_gpos_kern()`, otherwise
  `face.glyphs.horizontal_kern_indexed(a, b, px).unwrap_or(0.0)`. Add the
  kerning to the pen before placing the right-hand glyph, as `walk` does today.
- **Atlas key** `(FaceId, u16 glyph, u32 px bits)`, replacing
  `(char, u32, Weight)`. Signature:
  `GlyphAtlas::glyph(&mut self, face: &Face, glyph: u16, ch: char, px: f32) -> Result<AtlasRect>`,
  rasterising with `face.glyphs.rasterize_indexed(glyph, px)`. `ch` only fills
  `Error::AtlasFull::ch`, which stays a `char`: every glyph comes from one char,
  because there are no ligatures. `PlacedGlyph` gains `glyph: u16` and
  `face: &Face`, and the struct gets a lifetime. Update the `atlas_glyphs` doc:
  "(face, glyph and physical size)".
- **Not changed**: `SvgTarget`. Browsers and resvg kern by default
  (`font-kerning: auto`), so the SVG agrees better with the GPU than before. The
  SVG parity check masks text regions anyway.
- **Test strategy**: unit tests in `opentype.rs` (Inter Regular subset: `tnum`
  maps all ten digits, `kern_lookups == [0, 1]`, `kern(A, V) < 0`) and
  `layout.rs` (AC1–AC3), the oracle integration test (AC2), `draw_into_pass`
  (AC4) and the goldens (AC5). Two seeded failures go in the retrospective. The
  first-lookup-only one is in AC2. The other: make `measure_with` skip kerning
  while `walk` keeps it, and confirm the new `draw_into_pass` case or the AC1
  agreement test fails.

## Escalation Triggers

- `wasm.scatter.gz_bytes` grows by more than 6,000 B gz, or the harness lock
  gains a package: stop and report `mask wasm-attrib`.
- The oracle test disagrees with rustybuzz on any string, after the lookup order
  above: report the string and both glyph lists. Don't add GPOS cases until the
  disagreement is understood.
- `layout_cost` rises by more than 30 µs per frame: report both figures and
  whether the pair cache is hit.
- A golden fails a structural check or the SVG parity check. Or a non-golden
  gup-core test fails on a changed pixel value: update it only if the change is
  at most 2 px and comes from a label width; otherwise stop.
- `ttf-parser` 0.21's `PairSet` or `ClassMatrix` API differs from the Decisions
  (checked against the registry source on 2026-10-31).

## Out of Scope

- Turning `TNUM` on for tick labels, and the SVG `tabular-nums` style: GUP-579.
- Font fallback chains: GUP-416, which builds on `Face` and the atlas key.
- Ligatures, contextual alternates, mark positioning and complex scripts
  (Arabic, Indic). Those would need rustybuzz behind an off-by-default feature;
  no story is written for that yet.
- Changing `mask subset-inter`'s feature list.

## Technical Tasks

- [ ] Commit `tests/layout_cost.rs` first and run it on the Start tree (AC6).
- [ ] Add the `ttf-parser` dependency and the `rustybuzz` dev-dependency.
- [ ] `opentype.rs`: `OtLayout` with its unit tests.
- [ ] `font.rs`: `Face`, `FaceId`, `face_for -> &Face`.
- [ ] `layout.rs`: `Features`, `Run::features`, one shaping function behind
      `measure_with` and `walk`.
- [ ] `atlas.rs` and `system.rs`: the new key, `rasterize_indexed`,
      `TextSystem::measure_with`.
- [ ] Oracle test; `draw_into_pass` case; AC1 lossless check.
- [ ] gup-core: `features` in `layout_run` and `text_layer.rs`. Run the goldens
      with `NONE` first, then with the default, then re-bless.
- [ ] `mask wasm-size`, `mask wasm-browser`, `layout_cost`; commit the harness
      lock.

## Dependencies

### Prerequisite Stories

- GUP-400: RFC-001 S2, extract `gup-text` ✅
- GUP-407: Subset the bundled Inter ✅. The subsets keep GPOS `kern` and GSUB
  `tnum`.
- GUP-542: Text layer ✅. It edits the same layout path.

### Enables Stories

- GUP-579: tabular figures for tick labels.
- GUP-416: font fallback (per-face glyph indices and atlas keys).
- GUP-527: label halos and overlap culling (soft: measured widths are final).

## Testing Strategy

- **Unit tests**: `OtLayout` on the Regular subset; lossless `NONE` layout;
  kerned pairs narrower; `tnum` digit advances; atlas entries per face and
  glyph.
- **Integration**: the rustybuzz oracle; `draw_into_pass`; gup-core's goldens
  and `text_layer` tests.
- **Visual validation**: four goldens read before and after, and their diffs.
- **Performance**: `layout_cost` before and after; `mask wasm-size`.

## Success Metrics

- [ ] Every run is kerned exactly as rustybuzz would kern it with the bundled
      faces.
- [ ] WASM grows by at most 6,000 B gz, against +210,533 for rustybuzz.

## Risk Assessment

- **Medium**: the golden re-bless touches about 60 PNGs and conflicts with any
  parallel story that blesses a golden. _Mitigation_: `touches` lists
  `tests/golden/`, so the orchestrator runs this story alone among
  golden-changing stories.
- **Low**: user fonts with GPOS features this walk ignores (contextual kerning,
  marks). _Mitigation_: they still get pair kerning. That is documented on
  `Features::KERN`.
- **Low**: the pair cache's lock across threads. _Mitigation_: it is held for
  one hash lookup. Gup lays out text under the context's text lock anyway.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-text -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (the four goldens and their diffs)
- [ ] `mask wasm-browser` (margins can move)
- [ ] WASM size: `mask wasm-size` scatter gz bytes before and after
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
