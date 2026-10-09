// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fonts: the bundled default, loading from bytes and line metrics.

use crate::error::{Error, Result};
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

/// The complete Inter Regular 4.1, for text the [`INTER_REGULAR`] subset
/// does not cover: it adds Cyrillic, Vietnamese, the rest of Latin and
/// Greek, and more symbols (2,852 characters in all). Load it with
/// [`Font::inter_full`]. It is about 400 KB (200 KB gzipped), and only
/// linked into a program that uses it.
pub const INTER_REGULAR_FULL: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");

/// A parsed font face. Cheap to clone.
///
/// A character the face has no glyph for is measured and drawn as the
/// face's missing-glyph box (`.notdef`, a visible box in both bundled
/// faces): never as nothing, and never as another face's glyph, because
/// there is no font fallback. [`missing_glyphs`](Self::missing_glyphs)
/// finds such characters; load a face that covers them (such as
/// [`Font::inter_full`]) to draw them.
#[derive(Clone)]
pub struct Font {
    name: Arc<str>,
    data: Data,
    face: Arc<fontdue::Font>,
}

/// The font file a face was parsed from.
#[derive(Clone)]
enum Data {
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
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

    fn bundled(data: &'static [u8]) -> Self {
        // The bundled bytes are a compile-time constant covered by this
        // crate's tests, so a failure is a bug in Gup.
        Self::parse("Inter", Data::Static(data)).unwrap_or_else(|e| panic!("bundled font: {e}"))
    }

    /// Parse a TrueType or OpenType font. `name` labels it in errors and
    /// `Debug` output, and is the family name vector targets write. The
    /// bytes are copied, so [`data`](Self::data) can return them.
    pub fn from_bytes(name: &str, data: &[u8]) -> Result<Self> {
        Self::parse(name, Data::Owned(data.into()))
    }

    fn parse(name: &str, data: Data) -> Result<Self> {
        let face = fontdue::Font::from_bytes(data.bytes(), fontdue::FontSettings::default())
            .map_err(|reason| Error::Font {
                name: name.to_owned(),
                reason: reason.to_owned(),
            })?;
        Ok(Self {
            name: name.into(),
            data,
            face: Arc::new(face),
        })
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
        self.face.has_glyph(c)
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
            .face
            .horizontal_line_metrics(size)
            .map_or((size * 0.8, size * 0.2, size * 0.1), |m| {
                (m.ascent, -m.descent, m.line_gap)
            });
        let h = self.face.metrics('H', size);
        LineMetrics {
            ascent,
            descent,
            line_gap,
            cap_height: h.height as f32 + h.ymin as f32,
        }
    }

    pub(crate) fn face(&self) -> &fontdue::Font {
        &self.face
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The characters `fonts/inter-subset.txt` lists, read as pyftsubset
    /// reads them: hex code points or `lo-hi` ranges, `#` comments.
    fn listed() -> BTreeSet<char> {
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap_or_else(|e| panic!("{s:?}: {e}"));
        let mut chars = BTreeSet::new();
        for line in include_str!("../fonts/inter-subset.txt").lines() {
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
}
