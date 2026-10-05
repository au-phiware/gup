// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Straight-line horizontal layout: measuring, anchoring and placing the
//! glyphs of one run.
//!
//! Glyphs are placed on whole pixels, because they are drawn 1:1 from a
//! bitmap atlas. Kerning comes from the font's `kern` table when it has
//! one. There is no shaping, wrapping or rotation yet.

use crate::font::Font;

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
}

/// One glyph of a laid-out run, in physical pixels.
#[derive(Copy, Clone, Debug)]
pub(crate) struct PlacedGlyph {
    pub c: char,
    /// The glyph's size in physical pixels (the atlas key).
    pub px: f32,
    pub metrics: fontdue::Metrics,
    /// Top-left of the glyph's bitmap.
    pub x: f32,
    pub y: f32,
}

impl Font {
    /// Measure `text` at `size` pixels.
    pub fn measure(&self, text: &str, size: f32) -> TextMetrics {
        let face = self.face();
        let mut width = 0.0;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                width += face.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            width += face.metrics(c, size).advance_width;
            prev = Some(c);
        }
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

    /// Visit the glyphs of `run` laid out at `scale` physical pixels per
    /// logical pixel, in physical pixels.
    pub(crate) fn walk(&self, run: &Run<'_>, scale: f32, mut f: impl FnMut(PlacedGlyph)) {
        let face = self.face();
        let px = run.size * scale;
        let m = self.measure(run.text, px);
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
        // Whole pixels: bitmap glyphs are drawn 1:1.
        let (mut pen, baseline) = (x.round(), y.round());
        let mut prev = None;
        for c in run.text.chars() {
            if let Some(p) = prev {
                pen += face.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            let metrics = face.metrics(c, px);
            f(PlacedGlyph {
                c,
                px,
                metrics,
                x: (pen + metrics.xmin as f32).round(),
                y: baseline - (metrics.ymin as f32 + metrics.height as f32),
            });
            pen += metrics.advance_width;
            prev = Some(c);
        }
    }
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
}
