// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fonts: the bundled default, loading from bytes and line metrics.

use crate::error::{Error, Result};
use std::sync::{Arc, OnceLock};

/// Inter Regular 4.1, the bundled default face (SIL Open Font License
/// 1.1, see `fonts/Inter-OFL.txt`).
const INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");

/// A parsed font face. Cheap to clone.
#[derive(Clone)]
pub struct Font {
    name: Arc<str>,
    face: Arc<fontdue::Font>,
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
    /// The bundled default face, Inter Regular. It is parsed once per
    /// process and shared.
    pub fn inter() -> Self {
        static INTER: OnceLock<Font> = OnceLock::new();
        INTER
            .get_or_init(|| {
                // The bundled bytes are a compile-time constant covered by
                // this crate's tests, so a failure is a bug in Gup.
                Font::from_bytes("Inter", INTER_REGULAR)
                    .unwrap_or_else(|e| panic!("bundled font: {e}"))
            })
            .clone()
    }

    /// Parse a TrueType or OpenType font. `name` labels it in errors and
    /// `Debug` output.
    pub fn from_bytes(name: &str, data: &[u8]) -> Result<Self> {
        let face = fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).map_err(
            |reason| Error::Font {
                name: name.to_owned(),
                reason: reason.to_owned(),
            },
        )?;
        Ok(Self {
            name: name.into(),
            face: Arc::new(face),
        })
    }

    /// The name the font was loaded under.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the font has a glyph for `c` (otherwise `c` draws as the
    /// font's missing-glyph box).
    pub fn has_glyph(&self, c: char) -> bool {
        self.face.has_glyph(c)
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

    #[test]
    fn inter_is_bundled_and_shared() {
        let a = Font::inter();
        let b = Font::inter();
        assert_eq!(a.name(), "Inter");
        assert!(Arc::ptr_eq(&a.face, &b.face));
        for c in ('0'..='9').chain(['-', '.', ',', 'e', '+', '−', '×']) {
            assert!(a.has_glyph(c), "Inter lacks {c:?}");
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
}
