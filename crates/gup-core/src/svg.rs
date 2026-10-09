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
//! **Data marks are not written yet.** Evaluating mark positions and
//! colours on the CPU (`MarkBatch::vector()`, with a rasterised fallback
//! above a mark-count threshold) needs the column store and scale mirrors
//! of RFC-001 S4/S5. Until then a scene with an `ItemKind::Marks` item is
//! an error, never a silent drop; pass [`Scene::guides`] to write the rest.
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
use crate::scene::{GradientDirection, HAlign, ItemKind, Scene};
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

/// A scene's guides as an SVG document.
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
    /// Embed `font` in the document as an `@font-face` with a base64 data
    /// URL (only when the scene has text). Default: off; the document
    /// then names the font and relies on the viewer having it. The
    /// embedded file is the font's [`data`](gup_text::Font::data) as
    /// loaded, so a large face makes a large SVG; the bundled subset adds
    /// about 82 KB. The bundled Inter is under the SIL Open Font License,
    /// which allows embedding it in documents.
    pub embed_font: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            font: gup_text::Font::inter(),
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
                r#"<style>@font-face{{font-family:"{}";src:url(data:font/ttf;base64,{}) format("truetype")}}</style>"#,
                escape(&css_string(self.options.font.name())),
                base64(self.options.font.data())
            )?;
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
        for item in &scene.items {
            if let Some(clip) = item.clip {
                writeln!(s, r#"<g clip-path="url(#clip{})">"#, clip.0)?;
            }
            match &item.kind {
                ItemKind::Marks(_) => unreachable!("rejected before writing"),
                ItemKind::Rules(rules) => {
                    for r in rules {
                        // Square caps: the GPU extends each rule by half its
                        // width at both ends.
                        writeln!(
                            s,
                            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="square"{}/>"#,
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
                        let [x, y] = self.options.font.baseline_origin(&run.layout_run());
                        let anchor_x = run.at.x + (x.round() - x);
                        let baseline = y.round();
                        let anchor = match run.anchor.h {
                            HAlign::Start => "start",
                            HAlign::Middle => "middle",
                            HAlign::End => "end",
                        };
                        writeln!(
                            s,
                            r#"<text x="{}" y="{}" font-family="{}, sans-serif" font-size="{}" text-anchor="{anchor}" fill="{}"{}>{}</text>"#,
                            num(anchor_x),
                            num(baseline),
                            escape(self.options.font.name()),
                            num(run.style.size.0),
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
    /// Write `scene`'s guides. Errors (and writes nothing) if the scene
    /// has a data layer: mark export is RFC-001 S4/S5.
    fn render(&mut self, scene: &Scene) -> Result<()> {
        let marks = scene
            .items
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Marks(_)))
            .count();
        if marks > 0 {
            return Err(Error::config(
                "svg scene",
                format!(
                    "it has {marks} data layer(s); SvgTarget writes guides (rules, rects, \
                     gradients, text) only until mark export lands (RFC-001 S4/S5). Render \
                     `scene.guides()` to write everything else"
                ),
            ));
        }
        self.svg = self
            .write(scene)
            .map_err(|e| Error::config("svg target", e.to_string()))?;
        Ok(())
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
