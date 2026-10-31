// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Straight-line horizontal layout: measuring, anchoring and placing the
//! glyphs of one run.
//!
//! Glyphs are placed on whole pixels, because they are drawn 1:1 from a
//! bitmap atlas. Text is laid out by glyph index: each character maps to a
//! glyph (a tabular one when [`Features::TNUM`] asks), advances by that
//! glyph's width and kerns against its neighbour. There is no wrapping,
//! ligature or rotation yet.

use crate::font::{Face, Font};

/// Horizontal anchor of a text run.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HAlign {
    /// The anchor point is the left edge.
    Start,
    /// The anchor point is the centre.
    Middle,
    /// The anchor point is the right edge.
    End,
}

/// Vertical anchor of a text run.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VAlign {
    /// The anchor point is the cap-height line.
    Top,
    /// The anchor point is halfway up the capitals.
    Middle,
    /// The anchor point is the baseline.
    Baseline,
    /// The anchor point is the descender line.
    Bottom,
}

/// Where a run's anchor point sits on its text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    /// Horizontal.
    pub h: HAlign,
    /// Vertical.
    pub v: VAlign,
}

impl Anchor {
    /// The point is the centre of the text: halfway along it and halfway
    /// up its capitals.
    pub const CENTER: Self = Self::new(HAlign::Middle, VAlign::Middle);
    /// The point is the middle of the text's top (cap-height) edge, so the
    /// text hangs below it.
    pub const TOP: Self = Self::new(HAlign::Middle, VAlign::Top);
    /// The point is the middle of the text's bottom (descender) edge, so
    /// the text sits above it.
    pub const BOTTOM: Self = Self::new(HAlign::Middle, VAlign::Bottom);
    /// The point is the middle of the text's left edge, so the text runs to
    /// its right.
    pub const LEFT: Self = Self::new(HAlign::Start, VAlign::Middle);
    /// The point is the middle of the text's right edge, so the text ends at
    /// it.
    pub const RIGHT: Self = Self::new(HAlign::End, VAlign::Middle);
    /// The point is the text's top-left corner: the text lies below and to
    /// the right.
    pub const TOP_LEFT: Self = Self::new(HAlign::Start, VAlign::Top);
    /// The point is the text's top-right corner: the text lies below and to
    /// the left.
    pub const TOP_RIGHT: Self = Self::new(HAlign::End, VAlign::Top);
    /// The point is the text's bottom-left corner: the text lies above and
    /// to the right.
    pub const BOTTOM_LEFT: Self = Self::new(HAlign::Start, VAlign::Bottom);
    /// The point is the text's bottom-right corner: the text lies above and
    /// to the left.
    pub const BOTTOM_RIGHT: Self = Self::new(HAlign::End, VAlign::Bottom);

    /// An anchor.
    pub const fn new(h: HAlign, v: VAlign) -> Self {
        Self { h, v }
    }
}

/// Line metrics of a run, in pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TextMetrics {
    /// Advance width of the whole run.
    pub width: f32,
    /// Height of a capital letter above the baseline.
    pub cap_height: f32,
    /// Descent below the baseline (positive).
    pub descent: f32,
}

/// An axis-aligned box in pixels, origin top-left, y down.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// OpenType features a run asks for. The default is [`Features::KERN`].
///
/// Combine with `|`: `Features::KERN | Features::TNUM`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Features(u8);

impl Features {
    /// No features: glyph advances only.
    pub const NONE: Self = Self(0);
    /// GPOS `kern` pair adjustments, or the legacy `kern` table for a face
    /// without GPOS kerning. Only pair kerning applies: contextual kerning
    /// and mark positioning are ignored.
    pub const KERN: Self = Self(1);
    /// GSUB `tnum`: tabular (equal-advance) figures.
    pub const TNUM: Self = Self(2);

    /// The features in `self` or `other`.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every feature of `other` is in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl Default for Features {
    fn default() -> Self {
        Self::KERN
    }
}

impl std::ops::BitOr for Features {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        self.union(other)
    }
}

/// One line of horizontal text to measure or lay out, in logical pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Run<'a> {
    /// The text.
    pub text: &'a str,
    /// Font size.
    pub size: f32,
    /// The anchor point.
    pub at: [f32; 2],
    /// How `at` relates to the text.
    pub anchor: Anchor,
    /// Which bundled face draws the text.
    pub weight: Weight,
    /// OpenType features to apply (kerning by default).
    pub features: Features,
}

/// The weight of a [`Run`]: which of the two bundled faces draws it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weight {
    /// Inter Regular.
    #[default]
    Regular,
    /// Inter SemiBold (CSS weight 600). The bundled SemiBold covers printable
    /// ASCII and U+2009, U+202F and U+2212 only; any other character draws
    /// from the Regular face (and as the missing-glyph box if Regular lacks
    /// it too). [`TextSystem::with_fonts`](crate::TextSystem::with_fonts)
    /// takes a fuller face.
    SemiBold,
}

impl Weight {
    pub(crate) const COUNT: usize = 2;

    pub(crate) fn index(self) -> usize {
        match self {
            Weight::Regular => 0,
            Weight::SemiBold => 1,
        }
    }

    /// The CSS `font-weight` value.
    pub fn css(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::SemiBold => 600,
        }
    }
}

/// One glyph of a laid-out run, in physical pixels.
#[derive(Copy, Clone, Debug)]
pub(crate) struct PlacedGlyph<'f> {
    pub c: char,
    /// The face that draws the glyph, and its glyph index there.
    pub face: &'f Face,
    pub glyph: u16,
    /// The glyph's size in physical pixels (the atlas key).
    pub px: f32,
    pub metrics: fontdue::Metrics,
    /// Top-left of the glyph's bitmap.
    pub x: f32,
    pub y: f32,
}

impl Font {
    /// Measure `text` at `size` pixels, with the default [`Features`].
    pub fn measure(&self, text: &str, size: f32) -> TextMetrics {
        self.measure_with(text, size, Features::default())
    }

    /// Measure `text` at `size` pixels with `features`.
    pub fn measure_with(&self, text: &str, size: f32, features: Features) -> TextMetrics {
        let mut width = 0.0;
        self.shape(text, size, features, |g| width += g.kern + g.advance);
        let line = self.line_metrics(size);
        TextMetrics {
            width,
            cap_height: line.cap_height,
            descent: line.descent,
        }
    }

    /// The ink box of `run` in logical pixels: the union of its glyph
    /// bitmaps as drawn at scale 1. A run with no ink (empty or blank) has
    /// an empty box at its anchor point.
    pub fn ink_bounds(&self, run: &Run<'_>) -> Bounds {
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        self.walk(run, 1.0, |g| {
            if g.metrics.width == 0 || g.metrics.height == 0 {
                return;
            }
            let (x1, y1) = (g.x + g.metrics.width as f32, g.y + g.metrics.height as f32);
            let b = bounds.get_or_insert((g.x, g.y, x1, y1));
            *b = (b.0.min(g.x), b.1.min(g.y), b.2.max(x1), b.3.max(y1));
        });
        let (x0, y0, x1, y1) = bounds.unwrap_or((run.at[0], run.at[1], run.at[0], run.at[1]));
        Bounds {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    /// Where `run`'s baseline starts, `[x, y]` in logical pixels: the pen
    /// position of its first glyph, before rounding to whole pixels.
    /// Vector targets place `<text>` here (or at the same `y` with a
    /// matching `text-anchor`), so they agree with the GPU layout.
    pub fn baseline_origin(&self, run: &Run<'_>) -> [f32; 2] {
        self.origin(run, 1.0)
    }

    /// [`baseline_origin`](Self::baseline_origin) at `scale` physical
    /// pixels per logical pixel, in physical pixels.
    fn origin(&self, run: &Run<'_>, scale: f32) -> [f32; 2] {
        let m = self.measure_with(run.text, run.size * scale, run.features);
        let [ax, ay] = run.at.map(|v| v * scale);
        let x = match run.anchor.h {
            HAlign::Start => ax,
            HAlign::Middle => ax - m.width / 2.0,
            HAlign::End => ax - m.width,
        };
        let y = match run.anchor.v {
            VAlign::Top => ay + m.cap_height,
            VAlign::Middle => ay + m.cap_height / 2.0,
            VAlign::Baseline => ay,
            VAlign::Bottom => ay - m.descent,
        };
        [x, y]
    }

    /// Visit the glyphs of `run` laid out at `scale` physical pixels per
    /// logical pixel, in physical pixels.
    pub(crate) fn walk<'f>(
        &'f self,
        run: &Run<'_>,
        scale: f32,
        mut f: impl FnMut(PlacedGlyph<'f>),
    ) {
        let px = run.size * scale;
        let [x, y] = self.origin(run, scale);
        // Whole pixels: bitmap glyphs are drawn 1:1.
        let (mut pen, baseline) = (x.round(), y.round());
        self.shape(run.text, px, run.features, |g| {
            pen += g.kern;
            f(PlacedGlyph {
                c: g.c,
                face: g.face,
                glyph: g.glyph,
                px,
                metrics: g.metrics,
                x: (pen + g.metrics.xmin as f32).round(),
                y: baseline - (g.metrics.ymin as f32 + g.metrics.height as f32),
            });
            pen += g.advance;
        });
    }

    /// The one place text becomes glyphs: maps each character to a glyph
    /// index, substitutes tabular figures when asked, and measures the
    /// advance and the kerning against the previous glyph (zero unless both
    /// come from one face). `measure_with` and `walk` both read it, so
    /// measuring and drawing agree.
    fn shape<'f>(&'f self, text: &str, px: f32, features: Features, mut f: impl FnMut(Shaped<'f>)) {
        let mut prev: Option<(&Face, u16)> = None;
        for c in text.chars() {
            let face = self.face_for(c);
            let mut glyph = face.glyphs.lookup_glyph_index(c);
            if features.contains(Features::TNUM) {
                glyph = face.layout.tabular(glyph);
            }
            let kern = match prev {
                Some((pface, p)) if features.contains(Features::KERN) && pface.id == face.id => {
                    if face.layout.has_gpos_kern() {
                        face.layout.kern(p, glyph, px)
                    } else {
                        face.glyphs
                            .horizontal_kern_indexed(p, glyph, px)
                            .unwrap_or(0.0)
                    }
                }
                _ => 0.0,
            };
            let metrics = face.glyphs.metrics_indexed(glyph, px);
            f(Shaped {
                c,
                face,
                glyph,
                kern,
                advance: metrics.advance_width,
                metrics,
            });
            prev = Some((face, glyph));
        }
    }
}

/// One glyph out of [`Font::shape`].
struct Shaped<'f> {
    c: char,
    face: &'f Face,
    glyph: u16,
    /// Added to the pen before the glyph.
    kern: f32,
    /// Added to the pen after the glyph.
    advance: f32,
    metrics: fontdue::Metrics,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, anchor: Anchor) -> Run<'_> {
        Run {
            text,
            size: 16.0,
            at: [100.0, 50.0],
            anchor,
            weight: Weight::Regular,
            features: Features::default(),
        }
    }

    #[test]
    fn measure_grows_with_text_and_size() {
        let f = Font::inter();
        let a = f.measure("10", 12.0);
        let b = f.measure("1000", 12.0);
        let c = f.measure("10", 24.0);
        assert!(a.width > 0.0 && b.width > a.width && c.width > a.width);
        assert!(a.cap_height > 5.0 && a.cap_height < 12.0, "{a:?}");
        assert!(a.descent > 1.0 && a.descent < 6.0, "{a:?}");
    }

    #[test]
    fn anchors_place_the_ink_box() {
        let f = Font::inter();
        let middle_top = f.ink_bounds(&run("Title", Anchor::new(HAlign::Middle, VAlign::Top)));
        // Centred on x = 100 (within rounding), starting at y = 50.
        let centre = middle_top.x + middle_top.width / 2.0;
        assert!((centre - 100.0).abs() <= 1.5, "{middle_top:?}");
        assert!((middle_top.y - 50.0).abs() <= 1.0, "{middle_top:?}");
        let end = f.ink_bounds(&run("42", Anchor::new(HAlign::End, VAlign::Middle)));
        let right = end.x + end.width;
        assert!((97.0..=101.0).contains(&right), "{end:?}");
        let mid = end.y + end.height / 2.0;
        assert!((mid - 50.0).abs() <= 1.0, "{end:?}");
    }

    #[test]
    fn baseline_origin_is_where_the_glyphs_sit() {
        let f = Font::inter();
        let r = run("Hx", Anchor::new(HAlign::Middle, VAlign::Top));
        let [x, y] = f.baseline_origin(&r);
        let ink = f.ink_bounds(&r);
        // The H sits on the baseline with its top on the anchor line.
        assert!((ink.y + ink.height - y).abs() <= 1.0, "{ink:?} {y}");
        assert!((x - ink.x).abs() <= 2.0, "{ink:?} {x}");
        assert!((x + f.measure("Hx", 16.0).width / 2.0 - 100.0).abs() < 1e-3);
    }

    #[test]
    fn blank_runs_have_an_empty_box_at_the_anchor() {
        let b = Font::inter().ink_bounds(&run(" ", Anchor::new(HAlign::Start, VAlign::Top)));
        assert_eq!(
            b,
            Bounds {
                x: 100.0,
                y: 50.0,
                width: 0.0,
                height: 0.0
            }
        );
    }

    #[test]
    fn scale_lays_out_in_physical_pixels() {
        let f = Font::inter();
        let r = run("Title", Anchor::new(HAlign::Middle, VAlign::Top));
        let mut one = Vec::new();
        let mut two = Vec::new();
        f.walk(&r, 1.0, |g| one.push(g));
        f.walk(&r, 2.0, |g| two.push(g));
        assert_eq!(one.len(), two.len());
        assert_eq!(two[0].px, 32.0);
        // The first glyph lands at about twice the logical position.
        assert!((two[0].x - 2.0 * one[0].x).abs() <= 2.0, "{one:?} {two:?}");
        assert!((two[0].y - 2.0 * one[0].y).abs() <= 2.0, "{one:?} {two:?}");
    }

    const CHART_TEXT: [&str; 8] = [
        "0123456789 .,:;%+-±×÷=<>()[]/",
        "1.5k 20M 3G 4T 5µs 6 mm² 10⁻³ 7‰ ½ ¾",
        "−42 °C – 3 °F, 12.5 ℃, 1 m·s⁻¹, 5 Ω, CO₂",
        "$1 €2 £3 ¥4 ₹5 ₽6 ₩7 ₺8 ₿9 ¢",
        "Zürich, São Paulo, Kraków, İstanbul, Łódź, București, Århus, Øresund, Șiret",
        "σ = 0.3, Δt, λ, μ ≈ π, x ≤ y ≥ z ≠ w, √2, ∞, ∑",
        "Revenue — Q1 “actual” vs ‘plan’ … ← ↑ → ↓ «» №5 ™",
        "■ □ ▲ △ ▼ ◆ ◇ ● ○ ★ ✓ ✗",
    ];

    /// Glyph indices, unkerned, are the `char` layout this crate had before
    /// it laid text out by glyph: same advances, same positions.
    #[test]
    fn without_features_layout_is_the_character_layout() {
        for (font, weight) in [
            (Font::inter(), Weight::Regular),
            (Font::inter_semibold(), Weight::SemiBold),
        ] {
            for text in CHART_TEXT {
                for px in [11.0, 16.0] {
                    let advances: Vec<f32> = text
                        .chars()
                        .map(|c| font.face_for(c).glyphs.metrics(c, px).advance_width)
                        .collect();
                    let width: f32 = advances.iter().sum();
                    let measured = font.measure_with(text, px, Features::NONE).width;
                    assert_eq!(measured, width, "{text:?} at {px} {weight:?}");

                    let r = Run {
                        text,
                        size: px,
                        at: [10.0, 30.0],
                        anchor: Anchor::new(HAlign::Middle, VAlign::Baseline),
                        weight,
                        features: Features::NONE,
                    };
                    let mut placed = Vec::new();
                    font.walk(&r, 1.0, |g| placed.push((g.c, g.x, g.y, g.metrics.width)));
                    let (mut pen, baseline) = (font.baseline_origin(&r)[0].round(), 30.0);
                    for (i, c) in text.chars().enumerate() {
                        let m = font.face_for(c).glyphs.metrics(c, px);
                        let want = (
                            c,
                            (pen + m.xmin as f32).round(),
                            baseline - (m.ymin as f32 + m.height as f32),
                            m.width,
                        );
                        assert_eq!(placed[i], want, "{text:?} glyph {i} at {px} {weight:?}");
                        pen += advances[i];
                    }
                }
            }
        }
    }

    #[test]
    fn kerning_pairs_are_narrower_than_their_advances() {
        let f = Font::inter();
        for pair in ["AV", "To"] {
            let kerned = f.measure_with(pair, 16.0, Features::KERN).width;
            let plain = f.measure_with(pair, 16.0, Features::NONE).width;
            assert!(kerned < plain, "{pair}: {kerned} vs {plain}");
        }
        // The default is kerned.
        assert_eq!(
            f.measure("AV", 16.0).width,
            f.measure_with("AV", 16.0, Features::KERN).width
        );
        // Drawing follows measuring: the second glyph moves left by the kern.
        let walk = |features| {
            let r = Run {
                features,
                ..run("AV", Anchor::new(HAlign::Start, VAlign::Baseline))
            };
            let mut xs = Vec::new();
            f.walk(&r, 1.0, |g| xs.push(g.x));
            xs
        };
        assert!(walk(Features::KERN)[1] < walk(Features::NONE)[1]);
    }

    #[test]
    fn tabular_figures_share_one_advance() {
        for font in [Font::inter(), Font::inter_semibold()] {
            let tnum = Features::KERN | Features::TNUM;
            let digit = |c: char| font.measure_with(&c.to_string(), 16.0, tnum).width;
            for c in '0'..='9' {
                assert_eq!(digit(c), digit('0'), "{c}");
            }
            assert_eq!(
                font.measure_with("1000", 16.0, tnum).width,
                font.measure_with("1111", 16.0, tnum).width
            );
            // Proportional figures differ, so the feature does something.
            assert_ne!(
                font.measure_with("1", 16.0, Features::KERN).width,
                font.measure_with("0", 16.0, Features::KERN).width
            );
        }
    }
}
