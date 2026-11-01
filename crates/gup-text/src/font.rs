// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fonts: the bundled default, loading from bytes and line metrics.

use crate::error::{Error, Result};
use crate::opentype::OtLayout;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

/// The bundled default face: a subset of Inter Regular 4.1 (SIL Open Font
/// License 1.1, see `fonts/Inter-OFL.txt`), as TrueType bytes, for loading
/// the same face into another rasteriser (an SVG renderer) or embedding it.
///
/// It covers the characters listed in `fonts/inter-subset.txt`: printable
/// ASCII, Latin-1, Latin Extended-A and Romanian (Western, Central and
/// Eastern European, Baltic and Turkish labels), the basic Greek alphabet,
/// typographic spaces, dashes and quotes, superscript and subscript digits,
/// common currency signs (€ ₹ ₽ ₿ …), letterlike symbols (℃ № ™ Ω),
/// vulgar fractions, arrows, the common mathematical operators (the minus
/// sign U+2212, ≈ ≠ ≤ ≥ √ ∞ …) and the geometric shapes, stars and marks a
/// legend uses. Its glyphs are Inter's, unchanged. It keeps Inter's GPOS
/// kerning (`kern`) and tabular figures (`tnum`) but no other OpenType
/// feature, and no hinting. `mask subset-inter` regenerates it from
/// [`INTER_REGULAR_FULL`].
///
/// A character outside the subset draws as Inter's missing-glyph box: see
/// [`Font`].
pub const INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular-Subset.ttf");

/// The bundled bold face: a subset of Inter SemiBold 4.1 (weight 600), made
/// by `mask subset-inter` with the same features as [`INTER_REGULAR`] but a
/// much shorter character list (`fonts/inter-semibold-subset.txt`): printable
/// ASCII, thin space, narrow no-break space and minus. It draws
/// [`Weight::SemiBold`](crate::Weight::SemiBold) text; [`Font::inter_semibold`]
/// draws any other character from the Regular subset.
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold-Subset.ttf");

/// The complete Inter Regular 4.1, for text the [`INTER_REGULAR`] subset
/// does not cover: it adds Cyrillic, Vietnamese, the rest of Latin and
/// Greek, and more symbols (2,852 characters in all). Load it with
/// [`Font::inter_full`]. It is about 400 KB (200 KB gzipped), and only
/// linked into a program that uses it.
pub const INTER_REGULAR_FULL: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");

/// A font face with an optional chain of fallbacks. Cheap to clone. Loading
/// validates it; its glyphs are parsed on first use.
///
/// A character the face has no glyph for is drawn from its fallbacks in
/// order (the bundled SemiBold falls back to Regular; see
/// [`with_fallbacks`](Self::with_fallbacks) and
/// [`TextSystem::add_fallback`](crate::TextSystem::add_fallback)). When none
/// has it, it is measured and drawn as the face's missing-glyph box
/// (`.notdef`, a visible box in both bundled faces): never as nothing.
/// [`missing_glyphs`](Self::missing_glyphs) finds characters that draw as the
/// box; add a font that covers them (such as [`Font::inter_full`]) to draw them.
#[derive(Clone)]
pub struct Font {
    name: Arc<str>,
    data: Data,
    /// The rasteriser and OpenType layout data, parsed on first use.
    face: Arc<OnceLock<Face>>,
    /// Tried in order for characters this face lacks.
    fallbacks: Arc<[Font]>,
}

/// A face's identity: part of the glyph atlas key, so glyph indices of
/// different faces never collide.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FaceId(u32);

/// One parsed face: its rasteriser, its font file and its OpenType layout
/// data. Built once per font.
pub(crate) struct Face {
    pub id: FaceId,
    pub data: Data,
    pub glyphs: fontdue::Font,
    pub layout: OtLayout,
}

impl std::fmt::Debug for Face {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Face").field("id", &self.id).finish()
    }
}

/// The font file a face was parsed from.
#[derive(Clone)]
pub(crate) enum Data {
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl Data {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Data::Static(bytes) => bytes,
            Data::Owned(bytes) => bytes,
        }
    }
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font").field("name", &self.name).finish()
    }
}

/// Vertical metrics of a font at one size, in pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LineMetrics {
    /// Ascent above the baseline (positive).
    pub ascent: f32,
    /// Descent below the baseline (positive).
    pub descent: f32,
    /// Gap between one line's descent and the next line's ascent.
    pub line_gap: f32,
    /// Height of a capital letter (the ink of `H`) above the baseline.
    pub cap_height: f32,
}

impl Font {
    /// The bundled default face, the Inter Regular subset
    /// ([`INTER_REGULAR`]). It is parsed once per process and shared.
    pub fn inter() -> Self {
        static INTER: OnceLock<Font> = OnceLock::new();
        INTER.get_or_init(|| Font::bundled(INTER_REGULAR)).clone()
    }

    /// The complete Inter Regular ([`INTER_REGULAR_FULL`]), for text
    /// outside the bundled subset: pass it to
    /// [`TextSystem::with_font`](crate::TextSystem::with_font). It is
    /// parsed once per process and shared, and named "Inter" like the
    /// subset, whose glyphs it shares.
    pub fn inter_full() -> Self {
        static INTER_FULL: OnceLock<Font> = OnceLock::new();
        INTER_FULL
            .get_or_init(|| Font::bundled(INTER_REGULAR_FULL))
            .clone()
    }

    /// The bundled semibold face, the Inter SemiBold subset
    /// ([`INTER_SEMIBOLD`]). It is parsed on first use, once per process,
    /// and shared. Named "Inter" like the regular face.
    ///
    /// The SemiBold subset covers printable ASCII and U+2009, U+202F and
    /// U+2212 only. Any other character draws from the Inter Regular subset
    /// (so a non-ASCII label is lighter than its neighbours, not a
    /// missing-glyph box), and a character neither subset has draws as the
    /// box. Pass a fuller face to
    /// [`TextSystem::with_fonts`](crate::TextSystem::with_fonts) to draw
    /// more characters in SemiBold.
    pub fn inter_semibold() -> Self {
        static INTER: OnceLock<Font> = OnceLock::new();
        INTER
            .get_or_init(|| {
                let mut font = Font::bundled(INTER_SEMIBOLD);
                font.fallbacks = Arc::new([Font::inter()]);
                font
            })
            .clone()
    }

    fn bundled(data: &'static [u8]) -> Self {
        // The bundled bytes are a compile-time constant covered by this
        // crate's tests (`bundled_fonts_validate`), so they are not validated
        // here and a failure is a bug in Gup.
        Self::new("Inter", Data::Static(data))
    }

    fn new(name: &str, data: Data) -> Self {
        Self {
            name: name.into(),
            data,
            face: Arc::new(OnceLock::new()),
            fallbacks: Arc::new([]),
        }
    }

    /// Load a TrueType or OpenType font. `name` labels it in errors and
    /// `Debug` output, and is the family name vector targets write. The
    /// bytes are copied, so [`data`](Self::data) can return them.
    ///
    /// The font is validated here but its glyphs are parsed on first use (see
    /// [`is_parsed`](Self::is_parsed)), so adding a large font "just in case"
    /// costs microseconds until a character needs it.
    pub fn from_bytes(name: &str, data: &[u8]) -> Result<Self> {
        let font = Self::new(name, Data::Owned(data.into()));
        font.validate()?;
        Ok(font)
    }

    /// Check what the deferred parse needs: a readable table directory and a
    /// cmap that stays within the glyph count (fontdue panics on others).
    fn validate(&self) -> Result<()> {
        let font_error = |reason: String| Error::Font {
            name: self.name.to_string(),
            reason,
        };
        let face =
            ttf_parser::Face::parse(self.data.bytes(), 0).map_err(|e| font_error(e.to_string()))?;
        let glyphs = face.number_of_glyphs();
        let Some(cmap) = face.tables().cmap else {
            return Ok(());
        };
        for subtable in cmap.subtables {
            let mut bad = None;
            subtable.codepoints(|cp| {
                if bad.is_none()
                    && let Some(g) = subtable.glyph_index(cp)
                    && g.0 >= glyphs
                {
                    bad = Some((cp, g.0));
                }
            });
            if let Some((cp, g)) = bad {
                return Err(font_error(format!(
                    "its cmap maps U+{cp:04X} to glyph {g} of {glyphs}"
                )));
            }
        }
        Ok(())
    }

    /// The parsed face of this font alone (not its fallbacks), parsing it on
    /// first use.
    fn own(&self) -> &Face {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);
        self.face.get_or_init(|| {
            let glyphs = fontdue::Font::from_bytes(self.data.bytes(), Default::default())
                .or_else(|_| {
                    // A GSUB substitute past the glyph count: do without.
                    fontdue::Font::from_bytes(
                        self.data.bytes(),
                        fontdue::FontSettings {
                            load_substitutions: false,
                            ..Default::default()
                        },
                    )
                })
                .expect("validated in from_bytes");
            let layout = OtLayout::parse(
                &ttf_parser::Face::parse(self.data.bytes(), 0).expect("validated in from_bytes"),
            );
            Face {
                id: FaceId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
                data: self.data.clone(),
                glyphs,
                layout,
            }
        })
    }

    /// Whether the glyphs of this font have been parsed. Loading a font
    /// ([`from_bytes`](Self::from_bytes)) does not parse them; the first
    /// character drawn from it does, so a fallback font that no text needs is
    /// never parsed.
    #[doc(hidden)]
    pub fn is_parsed(&self) -> bool {
        self.face.get().is_some()
    }

    /// This font, then its own fallbacks, then `fallbacks`, each tried in
    /// order for characters the ones before lack. Only each fallback's own
    /// face is consulted: a fallback's own fallbacks are not followed. The
    /// glyphs (and their parse) are shared with `self`.
    pub fn with_fallbacks(&self, fallbacks: &[Font]) -> Font {
        Font {
            name: self.name.clone(),
            data: self.data.clone(),
            face: self.face.clone(),
            fallbacks: self.fallbacks.iter().chain(fallbacks).cloned().collect(),
        }
    }

    /// The name the font was loaded under.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The font file this face was parsed from, for embedding it (an SVG
    /// `@font-face`) or loading it into another rasteriser.
    pub fn data(&self) -> &[u8] {
        self.data.bytes()
    }

    /// Whether the font has a glyph for `c` (otherwise `c` draws as the
    /// font's missing-glyph box).
    pub fn has_glyph(&self, c: char) -> bool {
        self.has_own_glyph(c) || self.fallbacks.iter().any(|f| f.has_own_glyph(c))
    }

    /// Whether the font itself has a glyph for `c`, not counting a fallback
    /// face ([`inter_semibold`](Self::inter_semibold) falls back to Regular).
    /// Does not parse the font's glyphs.
    pub fn has_own_glyph(&self, c: char) -> bool {
        match self.face.get() {
            Some(face) => face.glyphs.has_glyph(c),
            None => ttf_parser::Face::parse(self.data.bytes(), 0)
                .is_ok_and(|f| f.glyph_index(c).is_some()),
        }
    }

    /// The distinct characters of `text` this font has no glyph for, in
    /// order of first appearance. Each is measured and drawn as the font's
    /// missing-glyph box. Empty when the font covers `text`.
    ///
    /// ```
    /// use gup_text::Font;
    ///
    /// let subset = Font::inter();
    /// assert!(subset.missing_glyphs("Zürich −3 °C, 1 µs, 5 €").is_empty());
    /// assert_eq!(subset.missing_glyphs("Київ (Kyiv)"), ['К', 'и', 'ї', 'в']);
    /// assert!(Font::inter_full().missing_glyphs("Київ").is_empty());
    /// ```
    pub fn missing_glyphs(&self, text: &str) -> Vec<char> {
        let mut missing = Vec::new();
        for c in text.chars() {
            if !self.has_glyph(c) && !missing.contains(&c) {
                missing.push(c);
            }
        }
        missing
    }

    /// Vertical metrics at `size` pixels. Fonts without horizontal line
    /// metrics fall back to 0.8 / 0.2 / 0.1 of the size.
    pub fn line_metrics(&self, size: f32) -> LineMetrics {
        let (ascent, descent, line_gap) = self
            .own()
            .glyphs
            .horizontal_line_metrics(size)
            .map_or((size * 0.8, size * 0.2, size * 0.1), |m| {
                (m.ascent, -m.descent, m.line_gap)
            });
        let h = self.own().glyphs.metrics('H', size);
        LineMetrics {
            ascent,
            descent,
            line_gap,
            cap_height: h.height as f32 + h.ymin as f32,
        }
    }

    #[cfg(test)]
    pub(crate) fn face(&self) -> &fontdue::Font {
        &self.own().glyphs
    }

    /// The face that draws `c`: this font's own, or the first fallback
    /// with a glyph for it; this font's when none has (its missing-glyph
    /// box). Parses the face it returns.
    pub(crate) fn face_for(&self, c: char) -> &Face {
        if !self.has_own_glyph(c)
            && let Some(fallback) = self.fallbacks.iter().find(|f| f.has_own_glyph(c))
        {
            return fallback.own();
        }
        self.own()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The characters `fonts/inter-subset.txt` lists, read as pyftsubset
    /// reads them: hex code points or `lo-hi` ranges, `#` comments.
    fn listed() -> BTreeSet<char> {
        listed_in(include_str!("../fonts/inter-subset.txt"))
    }

    fn listed_in(list: &str) -> BTreeSet<char> {
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap_or_else(|e| panic!("{s:?}: {e}"));
        let mut chars = BTreeSet::new();
        for line in list.lines() {
            let item = line.split('#').next().unwrap_or_default().trim();
            if item.is_empty() {
                continue;
            }
            let (lo, hi) = item
                .split_once('-')
                .map_or_else(|| (hex(item), hex(item)), |(lo, hi)| (hex(lo), hex(hi)));
            assert!(lo <= hi, "{item}");
            chars.extend((lo..=hi).map(|u| char::from_u32(u).unwrap()));
        }
        chars
    }

    #[test]
    fn inter_is_bundled_and_shared() {
        let a = Font::inter();
        let b = Font::inter();
        assert_eq!(a.name(), "Inter");
        assert!(Arc::ptr_eq(&a.face, &b.face));
        assert_eq!(a.data(), INTER_REGULAR);
        let full = Font::inter_full();
        assert_eq!(full.name(), "Inter");
        assert_eq!(full.data(), INTER_REGULAR_FULL);
        assert!(Arc::ptr_eq(&full.face, &Font::inter_full().face));
    }

    #[test]
    fn subset_maps_exactly_the_listed_characters() {
        let listed = listed();
        let mapped: BTreeSet<char> = Font::inter().face().chars().keys().copied().collect();
        let unmapped: Vec<_> = listed.difference(&mapped).collect();
        let extra: Vec<_> = mapped.difference(&listed).collect();
        assert!(
            unmapped.is_empty() && extra.is_empty(),
            "the subset is stale (run `mask subset-inter`) or the list names characters \
             Inter lacks: listed but unmapped {unmapped:?}, mapped but unlisted {extra:?}"
        );
        assert_eq!(Font::inter_full().face().chars().len(), 2852);
    }

    #[test]
    fn semibold_subset_maps_exactly_its_list() {
        let listed = listed_in(include_str!("../fonts/inter-semibold-subset.txt"));
        let semibold = Font::inter_semibold();
        let mapped: BTreeSet<char> = semibold.face().chars().keys().copied().collect();
        let unmapped: Vec<_> = listed.difference(&mapped).collect();
        let extra: Vec<_> = mapped.difference(&listed).collect();
        assert!(
            unmapped.is_empty() && extra.is_empty(),
            "the SemiBold subset is stale (run `mask subset-inter`) or the list names \
             characters Inter lacks: listed but unmapped {unmapped:?}, mapped but \
             unlisted {extra:?}"
        );
        assert_eq!(semibold.data(), INTER_SEMIBOLD);
        assert!(Arc::ptr_eq(&semibold.face, &Font::inter_semibold().face));
    }

    #[test]
    fn semibold_draws_what_it_lacks_from_regular() {
        let semibold = Font::inter_semibold();
        for c in ['–', 'é', 'µ', 'α'] {
            assert!(!semibold.face().has_glyph(c), "{c:?} is in the subset");
            assert!(semibold.has_glyph(c));
            let (m, bitmap) = semibold.face_for(c).glyphs.rasterize(c, 16.0);
            let regular = Font::inter().face().rasterize(c, 16.0);
            assert_eq!((m, bitmap), regular, "{c:?} comes from Regular");
        }
        // Mapped glyphs are SemiBold's own, and the box is the last resort.
        let a = semibold.face_for('A').glyphs.rasterize('A', 16.0);
        assert_eq!(a, semibold.face().rasterize('A', 16.0));
        assert_ne!(a, Font::inter().face().rasterize('A', 16.0));
        assert_eq!(semibold.missing_glyphs("2024–Q1"), Vec::<char>::new());
        assert_eq!(semibold.missing_glyphs("Київ"), ['К', 'и', 'ї', 'в']);
        // A mixed run has ink for every glyph and measures non-empty.
        let mut ink = Vec::new();
        let run = crate::Run {
            text: "2024–Q1",
            size: 16.0,
            at: [0.0, 20.0],
            anchor: crate::Anchor::new(crate::HAlign::Start, crate::VAlign::Baseline),
            weight: crate::Weight::SemiBold,
            features: crate::Features::default(),
        };
        semibold.walk(&run, 1.0, |g| ink.push((g.c, g.metrics.width > 0)));
        assert_eq!(ink.len(), 7);
        assert!(ink.iter().all(|&(_, has)| has), "{ink:?}");
    }

    #[test]
    fn subset_glyphs_are_the_full_faces() {
        // The same metrics, advances and bitmaps, so no pixel changes.
        let (subset, full) = (Font::inter(), Font::inter_full());
        for px in [11.0, 16.0] {
            assert_eq!(subset.line_metrics(px), full.line_metrics(px));
            for c in listed() {
                assert!(subset.has_glyph(c), "{c:?} (U+{:04X})", c as u32);
                let ours = subset.face().rasterize(c, px);
                assert_eq!(ours, full.face().rasterize(c, px), "{c:?} at {px} px");
                // Every listed character but a space has ink.
                let ink = ours.1.iter().any(|&a| a > 0);
                assert_eq!(ink, !c.is_whitespace(), "{c:?} (U+{:04X})", c as u32);
            }
        }
    }

    #[test]
    fn typical_chart_text_is_covered() {
        let font = Font::inter();
        for text in [
            "0123456789 .,:;%+-±×÷=<>()[]/",
            "1.5k 20M 3G 4T 5µs 6 mm² 10⁻³ 7‰ ½ ¾",
            "−42 °C – 3 °F, 12.5 ℃, 1 m·s⁻¹, 5 Ω, CO₂",
            "$1 €2 £3 ¥4 ₹5 ₽6 ₩7 ₺8 ₿9 ¢",
            "Zürich, São Paulo, Kraków, İstanbul, Łódź, București, Århus, Øresund, Șiret",
            "σ = 0.3, Δt, λ, μ ≈ π, x ≤ y ≥ z ≠ w, √2, ∞, ∑",
            "Revenue — Q1 “actual” vs ‘plan’ … ← ↑ → ↓ «» №5 ™",
            "■ □ ▲ △ ▼ ◆ ◇ ● ○ ★ ✓ ✗",
        ] {
            let missing = font.missing_glyphs(text);
            assert!(missing.is_empty(), "{text:?} lacks {missing:?}");
        }
    }

    #[test]
    fn missing_glyphs_draw_as_a_visible_box() {
        let subset = Font::inter();
        // Cyrillic, CJK and emoji are outside the subset; Cyrillic is in the
        // full face.
        assert_eq!(
            subset.missing_glyphs("Москва 2024"),
            ['М', 'о', 'с', 'к', 'в', 'а']
        );
        assert_eq!(subset.missing_glyphs("東京 🙂 東"), ['東', '京', '🙂']);
        assert!(Font::inter_full().missing_glyphs("Москва").is_empty());
        // The box has an advance and ink, so the character takes space and
        // shows instead of vanishing.
        let notdef = subset.face().rasterize_indexed(0, 16.0);
        assert!(notdef.0.advance_width > 5.0, "{:?}", notdef.0);
        assert!(notdef.1.iter().any(|&a| a > 128), "the box has no ink");
        for c in ['М', '東', '🙂'] {
            assert_eq!(subset.face().rasterize(c, 16.0), notdef, "{c:?}");
        }
    }

    #[test]
    fn line_metrics_are_plausible_for_inter() {
        let m = Font::inter().line_metrics(100.0);
        // Inter's cap height is 0.727 em; its ascent and descent are
        // 0.969 and 0.242 em.
        assert!((m.cap_height - 72.7).abs() < 2.0, "{m:?}");
        assert!(m.ascent > m.cap_height && m.ascent < 110.0, "{m:?}");
        assert!(m.descent > 15.0 && m.descent < 30.0, "{m:?}");
    }

    #[test]
    fn bad_bytes_are_an_error_naming_the_font() {
        let e = Font::from_bytes("Nope", b"not a font").unwrap_err();
        assert!(e.to_string().contains("\"Nope\""), "{e}");
    }

    #[test]
    fn from_bytes_keeps_the_file() {
        let f = Font::from_bytes("Mine", INTER_REGULAR).unwrap();
        assert_eq!(f.data(), INTER_REGULAR);
        assert_eq!(f.name(), "Mine");
    }

    /// A two-glyph-plus-notdef Noto Sans JP subset: "東" and "京".
    const JP: &[u8] = include_bytes!("../tests/fonts/NotoSansJP-Test.ttf");

    fn jp() -> Font {
        Font::from_bytes("Noto Sans JP", JP).unwrap()
    }

    fn id(font: &Font, c: char) -> FaceId {
        font.face_for(c).id
    }

    const MIXED: &str = "Київ — 東京 — Zürich";

    #[test]
    fn fallbacks_are_tried_in_order() {
        let (inter, full, jp) = (Font::inter(), Font::inter_full(), jp());
        let own = |f: &Font| f.own().id;
        let chain = inter.with_fallbacks(&[full.clone(), jp.clone()]);
        assert_eq!(inter.missing_glyphs(MIXED).len(), 6);
        assert!(chain.missing_glyphs(MIXED).is_empty());
        // Each character comes from the first font that has it.
        assert_eq!(id(&chain, 'Z'), own(&inter));
        assert_eq!(id(&chain, 'ü'), own(&inter));
        assert_eq!(id(&chain, 'К'), own(&full));
        assert_eq!(id(&chain, '東'), own(&jp));
        assert_eq!(id(&chain, '京'), own(&jp));
        // The test face has no Cyrillic, so its place in the chain is moot.
        let chain = inter.with_fallbacks(&[jp.clone(), full.clone()]);
        assert!(chain.missing_glyphs(MIXED).is_empty());
        assert_eq!(id(&chain, '東'), own(&jp));
        assert_eq!(id(&chain, 'К'), own(&full));
        // The first fallback with the glyph wins (Latin is in all three).
        assert_eq!(
            id(&inter.with_fallbacks(std::slice::from_ref(&full)), 'Z'),
            own(&inter)
        );
        // A character no font has is the primary's box.
        assert_eq!(chain.missing_glyphs("a🙂"), ['🙂']);
        assert_eq!(id(&chain, '🙂'), own(&inter));
        // Added fonts serve a SemiBold primary after its own Regular.
        let semibold = Font::inter_semibold().with_fallbacks(&[full.clone(), jp.clone()]);
        assert!(semibold.missing_glyphs(MIXED).is_empty());
        assert_eq!(id(&semibold, 'Z'), own(&Font::inter_semibold()));
        assert_eq!(id(&semibold, 'é'), own(&inter));
        assert_eq!(id(&semibold, 'К'), own(&full));
        // A fallback's own fallbacks are not followed.
        let nested = jp.with_fallbacks(std::slice::from_ref(&inter));
        let outer = Font::inter_full().with_fallbacks(&[nested]);
        assert_eq!(id(&outer, 'Z'), own(&full));
        assert_eq!(id(&outer, '東'), own(&jp));
        // ... and with_fallbacks shares the parse with its source.
        assert!(Arc::ptr_eq(&chain.face, &inter.face));
    }

    #[test]
    fn kerning_stops_at_a_face_change() {
        let jp_first = jp().with_fallbacks(&[Font::inter()]);
        let kern = |t: &str, f| jp_first.measure_with(t, 16.0, f).width;
        // "AV" kerns inside Inter; no pair is kerned across the face change.
        assert!(kern("AV", crate::Features::KERN) < kern("AV", crate::Features::NONE));
        for t in ["A東V", "東A", "V東"] {
            assert_eq!(
                kern(t, crate::Features::KERN),
                kern(t, crate::Features::NONE)
            );
        }
    }

    #[test]
    fn a_font_is_parsed_when_a_character_needs_it() {
        let jp = jp();
        assert!(!jp.is_parsed());
        let chain = Font::inter_full().with_fallbacks(std::slice::from_ref(&jp));
        // Covered text, measuring and line metrics never touch the fallback.
        let covered = "Zürich 1.5k −3 °C";
        assert!(chain.missing_glyphs(covered).is_empty());
        chain.measure_with(covered, 16.0, crate::Features::default());
        chain.line_metrics(16.0);
        assert!(!jp.is_parsed());
        // Coverage questions about it do not parse it either.
        assert!(jp.has_own_glyph('東') && !jp.has_own_glyph('Z'));
        assert!(!jp.is_parsed());
        assert!(
            chain
                .measure_with("東京", 16.0, crate::Features::default())
                .width
                > 16.0
        );
        assert!(jp.is_parsed());
        assert!(chain.is_parsed());
    }

    #[test]
    fn the_test_face_has_ink_and_a_taller_ascent() {
        let jp = jp();
        for c in ['東', '京'] {
            let (m, bitmap) = jp.own().glyphs.rasterize(c, 16.0);
            assert!(m.width > 8 && bitmap.iter().any(|&a| a > 128), "{c:?}");
            assert_ne!((m, bitmap), jp.own().glyphs.rasterize_indexed(0, 16.0));
        }
        assert!(jp.line_metrics(16.0).ascent > Font::inter().line_metrics(16.0).ascent);
    }

    #[test]
    fn bundled_fonts_validate() {
        for data in [INTER_REGULAR, INTER_SEMIBOLD, INTER_REGULAR_FULL] {
            Font::from_bytes("Inter", data).unwrap();
        }
    }

    #[test]
    fn a_cmap_past_the_glyph_count_is_an_error_at_from_bytes() {
        // maxp.numGlyphs (u16 at offset 4) of 3 becomes 1, so the cmap's
        // glyphs 1 and 2 are out of range.
        let mut data = JP.to_vec();
        let tables = u16::from_be_bytes([data[4], data[5]]) as usize;
        let record = (0..tables)
            .map(|i| 12 + i * 16)
            .find(|&r| &data[r..r + 4] == b"maxp")
            .expect("a maxp table");
        let offset = u32::from_be_bytes(data[record + 8..record + 12].try_into().unwrap()) as usize;
        assert_eq!(u16::from_be_bytes([data[offset + 4], data[offset + 5]]), 3);
        data[offset + 4..offset + 6].copy_from_slice(&1u16.to_be_bytes());
        let e = Font::from_bytes("Patched", &data).unwrap_err().to_string();
        assert!(e.contains("\"Patched\""), "{e}");
        assert!(e.contains("its cmap maps U+4EAC to glyph"), "{e}");
        assert!(e.contains("of 1"), "{e}");
    }
}
