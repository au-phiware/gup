// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vector output (RFC-001 §7): [`VectorTarget`] and [`SvgTarget`].
//!
//! The SVG target writes a scene's guides directly: rules as `<line>`,
//! rects as `<rect>`, gradient legends as `<linearGradient>` fills and
//! text runs as `<text>`, clipped with `<clipPath>`. Text is placed with
//! the same `gup_text::Font` measurements the GPU path lays out with, so
//! margins and anchors match the PNG.
//!
//! **Data marks are vector shapes** (GUP-467, feature `svg`, on by default;
//! without it a scene with a data layer is an error and only guides are
//! written): one `<g>` per layer, with
//! the layer's clip, holding a `<circle>`, `<rect>`, `<line>` or
//! `<polyline>` per mark, in the order the GPU draws them. Positions,
//! sizes and colours come from the CPU mirrors picking uses
//! (`crate::vector`), so the file and the picks agree; rows hidden by a
//! null are not written, and neither are chunks culled for lying outside
//! the plot (they could not reach a pixel; the file is the view, not the
//! data). Coordinates have at most two decimals.
//!
//! A layer that cannot be evaluated on the CPU is **skipped with an XML
//! comment**, `<!-- layer N skipped: <reason> -->`, never an error: a
//! user-defined `#[derive(Mark)]` mark has no CPU description; a layer
//! whose [`Retain`](crate::Retain) policy drops the CPU columns
//! (`GpuOnly`, `Rows`, or `Auto` above `Retain::AUTO_MAX_ROWS` rows) has
//! nothing to read; a colour function without a CPU mirror cannot be
//! coloured. Layers count in `Scene` order, from 0. A scene's
//! [`Scene::guides`] is still the way to write only the guides.
//!
//! Marks are evaluated as they were when the scene was resolved, except
//! that a shared [`ScaleRef`](crate::ScaleRef) is read live: write the
//! scene out before changing its scales.
//!
//! **Fonts are referenced, and optionally embedded.** Text is set in
//! `font-family="Inter, sans-serif"`. By default the font is not
//! embedded: viewers with Inter installed match the PNG, and others fall
//! back to a sans-serif face at the same anchor points (`text-anchor`
//! keeps centred and right-aligned labels in place). With
//! [`SvgOptions::embed_font`] the document declares the font as an
//! `@font-face` with a base64 data URL, so browsers draw the same glyphs
//! as the PNG everywhere. For the bundled Inter subset that adds about
//! 82 KB to the file (37 KB gzipped). Renderers without web-font
//! support (resvg, many editors) ignore the declaration and use their own
//! font lookup, as without embedding.

use crate::channel::Color;
use crate::error::{Error, Result};
#[cfg(feature = "svg")]
use crate::scene::MarkBatch;
use crate::scene::{GradientDirection, HAlign, ItemKind, Scene, Weight};
#[cfg(feature = "svg")]
use crate::vector::{LayerVector, VectorMark};
use std::fmt::Write as _;

/// Something that writes a [`Scene`] as vector graphics (SVG, PDF).
pub trait VectorTarget {
    /// Write `scene`, replacing anything written before.
    fn render(&mut self, scene: &Scene) -> Result<()>;
}

/// Gradient stops written per legend bar. The GPU samples a 256-entry
/// LUT; 64 evenly spaced stops keep the piecewise-linear SVG gradient
/// within a fraction of a ΔE of it for smooth palettes.
const GRADIENT_STOPS: usize = 64;

/// A resolved scene as an SVG document: axes, grid, text and legends, and, with
/// the default `svg` feature, the data marks.
///
/// ```
/// use gup_core::scene::{Item, ItemKind, Rule};
/// use gup_core::{Color, Px, Scene, SvgTarget, VectorTarget, geom::Point};
///
/// let mut scene = Scene::new(100.0, 50.0, Color::WHITE);
/// scene.push(Item {
///     z: 0,
///     clip: None,
///     kind: ItemKind::Rules(vec![Rule {
///         p0: Point::new(10.0, 40.5),
///         p1: Point::new(90.0, 40.5),
///         width: Px(1.0),
///         color: Color::hex(0x333333),
///     }]),
/// });
/// let mut svg = SvgTarget::new();
/// svg.render(&scene)?;
/// assert!(svg.svg().contains(r##"<line x1="10" y1="40.5" x2="90" y2="40.5" stroke="#333333""##));
/// # Ok::<(), gup_core::Error>(())
/// ```
#[derive(Debug)]
pub struct SvgTarget {
    options: SvgOptions,
    svg: String,
}

/// How an [`SvgTarget`] writes text.
///
/// ```
/// use gup_core::{SvgOptions, SvgTarget};
///
/// // Self-contained: the bundled Inter subset travels with the file.
/// let svg = SvgTarget::with_options(SvgOptions {
///     embed_font: true,
///     ..SvgOptions::default()
/// });
/// # let _ = svg;
/// ```
#[derive(Clone, Debug)]
pub struct SvgOptions {
    /// The font text is measured with and named by in the document (its
    /// [`name`](gup_text::Font::name) is the `font-family`). Default: the
    /// bundled Inter subset, as every [`Context`](crate::Context)'s text
    /// system uses.
    pub font: gup_text::Font,
    /// The face of [`Weight::SemiBold`](crate::scene::Weight::SemiBold)
    /// runs (major tick labels), written with `font-weight="600"`.
    /// Default: the bundled Inter SemiBold subset, which covers printable
    /// ASCII and U+2009, U+202F and U+2212 only (the GPU path draws other
    /// characters from Regular; an embedded copy does not). Name it like
    /// `font`, so both share one `font-family`.
    pub semibold_font: gup_text::Font,
    /// Embed `font` in the document as an `@font-face` with a base64 data
    /// URL (only when the scene has text). Default: off; the document
    /// then names the font and relies on the viewer having it. The
    /// embedded file is the font's [`data`](gup_text::Font::data) as
    /// loaded, so a large face makes a large SVG; the bundled subset adds
    /// about 82 KB. The bundled Inter is under the SIL Open Font License,
    /// which allows embedding it in documents. `semibold_font` follows as
    /// a second `@font-face` with `font-weight:600`, when the scene has a
    /// semibold run.
    pub embed_font: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            font: gup_text::Font::inter(),
            semibold_font: gup_text::Font::inter_semibold(),
            embed_font: false,
        }
    }
}

impl Default for SvgTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl SvgTarget {
    /// A target with the default [`SvgOptions`]: the bundled Inter,
    /// referenced but not embedded.
    pub fn new() -> Self {
        Self::with_options(SvgOptions::default())
    }

    /// A target with `options`.
    pub fn with_options(options: SvgOptions) -> Self {
        Self {
            options,
            svg: String::new(),
        }
    }

    /// The document written by the last [`render`](VectorTarget::render).
    pub fn svg(&self) -> &str {
        &self.svg
    }

    /// The document, consuming the target.
    pub fn into_string(self) -> String {
        self.svg
    }

    /// Write the document to `path`.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let path = path.as_ref();
        if self.svg.is_empty() {
            return Err(Error::config(
                "svg target",
                "nothing to save: render a scene first",
            ));
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.display().to_string(),
                source,
            })?;
        }
        std::fs::write(path, &self.svg).map_err(|source| Error::Io {
            path: path.display().to_string(),
            source,
        })
    }

    fn write(&self, scene: &Scene) -> std::result::Result<String, std::fmt::Error> {
        let (w, h) = (num(scene.width), num(scene.height));
        let mut s = String::new();
        writeln!(
            s,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
        )?;
        // Definitions: one clip path per scene clip, one gradient per bar.
        s.push_str("<defs>\n");
        if self.options.embed_font && scene.text_runs().next().is_some() {
            // The family is a CSS string inside XML text: CSS-escaped,
            // then XML-escaped. Base64 needs neither.
            writeln!(
                s,
                r#"<style>@font-face{{font-family:"{}";font-weight:400;src:url(data:font/ttf;base64,{}) format("truetype")}}</style>"#,
                escape(&css_string(self.options.font.name())),
                base64(self.options.font.data())
            )?;
            if scene
                .text_runs()
                .any(|r| r.style.weight == Weight::SemiBold)
            {
                writeln!(
                    s,
                    r#"<style>@font-face{{font-family:"{}";font-weight:600;src:url(data:font/ttf;base64,{}) format("truetype")}}</style>"#,
                    escape(&css_string(self.options.semibold_font.name())),
                    base64(self.options.semibold_font.data())
                )?;
            }
        }
        for (i, c) in scene.clips.iter().enumerate() {
            writeln!(
                s,
                r#"<clipPath id="clip{i}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath>"#,
                num(c.x),
                num(c.y),
                num(c.width),
                num(c.height)
            )?;
        }
        let bars = scene.items.iter().filter_map(|i| match &i.kind {
            ItemKind::Gradient(bar) => Some(bar),
            _ => None,
        });
        for (i, bar) in bars.enumerate() {
            let r = bar.rect;
            // From the domain minimum to the maximum, in user space.
            let (x1, y1, x2, y2) = match bar.direction {
                GradientDirection::Horizontal => (r.left(), r.top(), r.right(), r.top()),
                GradientDirection::Vertical => (r.left(), r.bottom(), r.left(), r.top()),
            };
            writeln!(
                s,
                r#"<linearGradient id="gradient{i}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                num(x1),
                num(y1),
                num(x2),
                num(y2)
            )?;
            for k in 0..GRADIENT_STOPS {
                let t = k as f64 / (GRADIENT_STOPS - 1) as f64;
                writeln!(
                    s,
                    r#"<stop offset="{}" stop-color="{}"{}/>"#,
                    num(t as f32),
                    hex(bar.color_at(t)),
                    opacity("stop-opacity", bar.color_at(t))
                )?;
            }
            s.push_str("</linearGradient>\n");
        }
        s.push_str("</defs>\n");

        if scene.background.a > 0.0 {
            writeln!(
                s,
                r#"<rect width="100%" height="100%" fill="{}"{}/>"#,
                hex(scene.background),
                opacity("fill-opacity", scene.background)
            )?;
        }
        let mut gradient = 0;
        #[cfg(feature = "svg")]
        let mut layer = 0;
        for item in &scene.items {
            if let Some(clip) = item.clip {
                writeln!(s, r#"<g clip-path="url(#clip{})">"#, clip.0)?;
            }
            match &item.kind {
                #[cfg(not(feature = "svg"))]
                ItemKind::Marks(_) => unreachable!("rejected before writing"),
                #[cfg(feature = "svg")]
                ItemKind::Marks(batch) => {
                    if item.clip.is_none() {
                        s.push_str("<g>\n");
                    }
                    write_layer(&mut s, layer, batch)?;
                    if item.clip.is_none() {
                        s.push_str("</g>\n");
                    }
                    layer += 1;
                }
                ItemKind::Rules(rules) => {
                    for r in rules {
                        // Square caps: the GPU extends each rule by half its
                        // width at both ends. An axis-aligned rule is crisp
                        // there (no anti-aliasing), so say so here: a
                        // rasteriser that anti-aliases a long hairline can
                        // leave a stray half-covered pixel on it.
                        let crisp = if r.p0.x == r.p1.x || r.p0.y == r.p1.y {
                            r#" shape-rendering="crispEdges""#
                        } else {
                            ""
                        };
                        writeln!(
                            s,
                            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="square"{crisp}{}/>"#,
                            num(r.p0.x),
                            num(r.p0.y),
                            num(r.p1.x),
                            num(r.p1.y),
                            hex(r.color),
                            num(r.width.0),
                            opacity("stroke-opacity", r.color)
                        )?;
                    }
                }
                ItemKind::Rects(rects) => {
                    for r in rects {
                        writeln!(
                            s,
                            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"{}/>"#,
                            num(r.rect.x),
                            num(r.rect.y),
                            num(r.rect.width),
                            num(r.rect.height),
                            hex(r.color),
                            opacity("fill-opacity", r.color)
                        )?;
                    }
                }
                ItemKind::Gradient(bar) => {
                    writeln!(
                        s,
                        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#gradient{gradient})"/>"#,
                        num(bar.rect.x),
                        num(bar.rect.y),
                        num(bar.rect.width),
                        num(bar.rect.height)
                    )?;
                    gradient += 1;
                }
                ItemKind::Text(runs) => {
                    for run in runs {
                        // Horizontal placement by `text-anchor` (so a
                        // fallback face stays centred or right-aligned);
                        // the baseline from Inter's metrics. Like the GPU
                        // path, the pen start and baseline snap to whole
                        // pixels, so glyphs land on the same pixels.
                        let font = match run.style.weight {
                            Weight::Regular => &self.options.font,
                            Weight::SemiBold => &self.options.semibold_font,
                        };
                        let [x, y] = font.baseline_origin(&run.layout_run());
                        let anchor_x = run.at.x + (x.round() - x);
                        let baseline = y.round();
                        let anchor = match run.anchor.h {
                            HAlign::Start => "start",
                            HAlign::Middle => "middle",
                            HAlign::End => "end",
                        };
                        writeln!(
                            s,
                            r#"<text x="{}" y="{}" font-family="{}, sans-serif" font-size="{}"{} text-anchor="{anchor}" fill="{}"{}>{}</text>"#,
                            num(anchor_x),
                            num(baseline),
                            escape(self.options.font.name()),
                            num(run.style.size.0),
                            match run.style.weight {
                                Weight::Regular => String::new(),
                                w => format!(r#" font-weight="{}""#, w.css()),
                            },
                            hex(run.style.color),
                            opacity("fill-opacity", run.style.color),
                            escape(&run.text)
                        )?;
                    }
                }
            }
            if item.clip.is_some() {
                s.push_str("</g>\n");
            }
        }
        s.push_str("</svg>\n");
        Ok(s)
    }
}

impl VectorTarget for SvgTarget {
    /// Write `scene`: its guides and its data marks. A layer that cannot be
    /// evaluated on the CPU is skipped with an XML comment, never an error.
    fn render(&mut self, scene: &Scene) -> Result<()> {
        #[cfg(not(feature = "svg"))]
        {
            let marks = scene
                .items
                .iter()
                .filter(|i| matches!(i.kind, ItemKind::Marks(_)))
                .count();
            if marks > 0 {
                return Err(Error::config(
                    "svg scene",
                    format!(
                        "it has {marks} data layer(s) and gup-core was built without its \
                         `svg` feature, so SvgTarget writes guides only; render \
                         `scene.guides()` or enable the feature"
                    ),
                ));
            }
        }
        self.svg = self
            .write(scene)
            .map_err(|e| Error::config("svg target", e.to_string()))?;
        Ok(())
    }
}

#[cfg(feature = "svg")]
/// One layer's `<g>` contents, or the comment saying why there are none.
fn write_layer(
    s: &mut String,
    layer: usize,
    batch: &MarkBatch,
) -> std::result::Result<(), std::fmt::Error> {
    let marks = match &*batch.vector {
        LayerVector::Cpu(cpu) => cpu.marks(&batch.visible),
        LayerVector::Skipped(why) => Err(why.clone()),
    };
    let marks = match marks {
        Ok(marks) => marks,
        Err(why) => {
            // `--` may not appear in a comment.
            return writeln!(
                s,
                "<!-- layer {layer} skipped: {} -->",
                why.replace("--", "- -")
            );
        }
    };
    for mark in &marks {
        match mark {
            VectorMark::Circle {
                c,
                r,
                fill,
                stroke,
                stroke_width,
            } => {
                writeln!(
                    s,
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{}"{}/>"#,
                    num2(c[0]),
                    num2(c[1]),
                    num2(*r),
                    hex(*fill),
                    opacity("fill-opacity", *fill)
                )?;
                // The stroke lies inside the edge, over the fill.
                let w = stroke_width.min(*r);
                if w > 0.0 && stroke.a > 0.0 {
                    writeln!(
                        s,
                        r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="{}"{}/>"#,
                        num2(c[0]),
                        num2(c[1]),
                        num2(r - w / 2.0),
                        hex(*stroke),
                        num2(w),
                        opacity("stroke-opacity", *stroke)
                    )?;
                }
            }
            VectorMark::Rect {
                lo,
                hi,
                fill,
                stroke,
                stroke_width,
            } => {
                let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
                writeln!(
                    s,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"{}/>"#,
                    num2(lo[0]),
                    num2(lo[1]),
                    num2(w),
                    num2(h),
                    hex(*fill),
                    opacity("fill-opacity", *fill)
                )?;
                let sw = stroke_width.min(w / 2.0).min(h / 2.0);
                if sw > 0.0 && stroke.a > 0.0 {
                    writeln!(
                        s,
                        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="{}"{}/>"#,
                        num2(lo[0] + sw / 2.0),
                        num2(lo[1] + sw / 2.0),
                        num2(w - sw),
                        num2(h - sw),
                        hex(*stroke),
                        num2(sw),
                        opacity("stroke-opacity", *stroke)
                    )?;
                }
            }
            VectorMark::Segment { a, b, width, color } => {
                writeln!(
                    s,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="square"{}/>"#,
                    num2(a[0]),
                    num2(a[1]),
                    num2(b[0]),
                    num2(b[1]),
                    hex(*color),
                    num2(*width),
                    opacity("stroke-opacity", *color)
                )?;
            }
            VectorMark::Polyline {
                points,
                width,
                color,
            } => {
                s.push_str(r#"<polyline points=""#);
                for (i, p) in points.iter().enumerate() {
                    if i > 0 {
                        s.push(' ');
                    }
                    write!(s, "{},{}", num2(p.x), num2(p.y))?;
                }
                writeln!(
                    s,
                    r#"" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"{}/>"#,
                    hex(*color),
                    num2(*width),
                    opacity("stroke-opacity", *color)
                )?;
            }
            VectorMark::Cubic {
                start,
                segments,
                width,
                color,
            } => {
                write!(s, r#"<path d="M{},{}"#, num2(start.x), num2(start.y))?;
                for [k1, k2, b] in segments {
                    write!(
                        s,
                        " C{},{} {},{} {},{}",
                        num2(k1.x),
                        num2(k1.y),
                        num2(k2.x),
                        num2(k2.y),
                        num2(b.x),
                        num2(b.y)
                    )?;
                }
                writeln!(
                    s,
                    r#"" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"{}/>"#,
                    hex(*color),
                    num2(*width),
                    opacity("stroke-opacity", *color)
                )?;
            }
            VectorMark::Area { top, base, fill } => {
                // The top edge, then the baseline back: the default nonzero
                // rule fills both lobes where the edge crosses the baseline.
                s.push_str(r#"<path d=""#);
                for (i, p) in top.iter().chain(base.iter().rev()).enumerate() {
                    let op = if i == 0 { 'M' } else { 'L' };
                    if i > 0 {
                        s.push(' ');
                    }
                    write!(s, "{op}{},{}", num2(p.x), num2(p.y))?;
                }
                writeln!(
                    s,
                    r#" Z" fill="{}"{}/>"#,
                    hex(*fill),
                    opacity("fill-opacity", *fill)
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "svg")]
/// A mark coordinate or size with at most two decimals and no trailing
/// zeros.
fn num2(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// A coordinate or size with at most three decimals and no trailing
/// zeros.
fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// `#rrggbb` (sRGB, as Gup colours are).
fn hex(c: Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// ` attr="a"` for translucent colours, nothing for opaque ones.
fn opacity(attr: &str, c: Color) -> String {
    if c.a >= 1.0 {
        String::new()
    } else {
        format!(r#" {attr}="{}""#, num(c.a))
    }
}

/// The contents of a double-quoted CSS string (without the quotes).
fn css_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // A newline cannot appear in a CSS string; escape it as a
            // code point.
            '\n' => out.push_str("\\a "),
            c => out.push(c),
        }
    }
    out
}

/// Standard base64 (RFC 4648 §4) with padding.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// XML text and attribute escaping.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short() {
        assert_eq!(num(10.0), "10");
        assert_eq!(num(40.5), "40.5");
        assert_eq!(num(1.0 / 3.0), "0.333");
        assert_eq!(num(-0.0001), "0");
    }

    #[test]
    fn text_is_escaped() {
        assert_eq!(escape(r#"a<b & "c"'"#), "a&lt;b &amp; &quot;c&quot;&apos;");
        assert_eq!(css_string(r#"My "Font"\2"#), r#"My \"Font\"\\2"#);
        assert_eq!(css_string("a\nb"), "a\\a b");
    }

    #[test]
    fn base64_matches_rfc_4648() {
        for (raw, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(raw.as_bytes()), encoded, "{raw:?}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xbf]), "+/+/");
    }

    #[test]
    fn a_scene_without_text_embeds_no_font() {
        use crate::geom::Point;
        use crate::scene::{Item, Rule};
        let mut scene = Scene::new(10.0, 10.0, Color::WHITE);
        scene.push(Item {
            z: 0,
            clip: None,
            kind: ItemKind::Rules(vec![Rule {
                p0: Point::new(0.0, 5.0),
                p1: Point::new(10.0, 5.0),
                width: crate::Px(1.0),
                color: Color::BLACK,
            }]),
        });
        let mut svg = SvgTarget::with_options(SvgOptions {
            embed_font: true,
            ..SvgOptions::default()
        });
        svg.render(&scene).unwrap();
        assert!(!svg.svg().contains("@font-face"), "{}", svg.svg());
    }

    #[test]
    fn colours_are_hex_with_opacity_only_when_translucent() {
        assert_eq!(hex(Color::hex(0x1f77b4)), "#1f77b4");
        assert_eq!(opacity("fill-opacity", Color::WHITE), "");
        let half = Color {
            a: 0.5,
            ..Color::BLACK
        };
        assert_eq!(opacity("fill-opacity", half), r#" fill-opacity="0.5""#);
    }
}
