// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Structural checks over an [`RgbaImage`] and its [`LayoutMetadata`].
//!
//! These are independent of any golden image, so they catch blank output,
//! missing text, marks outside the plot and wrong colours even when a
//! golden image is stale or missing.

use crate::color::Rgba8;
use crate::image::RgbaImage;
use crate::layout::{LayoutMetadata, PxRect};
use std::fmt;

/// The kinds of check the harness runs. Names are stable: they are used as
/// keys in expected-failure lists.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Check {
    /// The target produced an image (or, for an example, ran to completion)
    /// without an error or panic.
    Render,
    /// The image is neither a single solid colour nor background everywhere.
    NotBlank,
    /// The plot rectangle contains data ink (not just guides).
    MarksPresent,
    /// Every expected text region contains ink.
    TextPresent,
    /// No ink outside the plot rectangle other than guides and text.
    MarksConfined,
    /// Every explicitly configured colour appears in the image.
    ColorPresent,
    /// The image matches its golden image within the perceptual tolerance.
    Golden,
}

impl Check {
    /// Every check, in the order the harness runs them.
    pub const ALL: [Check; 7] = [
        Check::Render,
        Check::NotBlank,
        Check::MarksPresent,
        Check::TextPresent,
        Check::MarksConfined,
        Check::ColorPresent,
        Check::Golden,
    ];

    /// The stable name used in expected-failure files.
    pub fn name(self) -> &'static str {
        match self {
            Check::Render => "render",
            Check::NotBlank => "not_blank",
            Check::MarksPresent => "marks_present",
            Check::TextPresent => "text_present",
            Check::MarksConfined => "marks_confined",
            Check::ColorPresent => "color_present",
            Check::Golden => "golden",
        }
    }

    /// Parse a stable check name.
    pub fn from_name(name: &str) -> Option<Check> {
        Check::ALL.into_iter().find(|c| c.name() == name)
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A failed check and a description of what was wrong.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckFailure {
    /// Which check failed.
    pub check: Check,
    /// What was observed.
    pub message: String,
}

impl CheckFailure {
    /// Create a failure.
    pub fn new(check: Check, message: impl Into<String>) -> Self {
        Self {
            check,
            message: message.into(),
        }
    }
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.check, self.message)
    }
}

/// Thresholds for the structural checks.
///
/// These are deliberately renderer-independent. Do not loosen them to make
/// a known-broken case pass; track it as an expected failure instead.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tolerances {
    /// A pixel counts as "ink" when its CIEDE2000 difference from the
    /// background exceeds this.
    pub ink_delta_e: f32,
    /// A pixel matches an expected colour when its CIEDE2000 difference is
    /// at most this (allows for anti-aliasing and quantisation).
    pub color_delta_e: f32,
    /// Minimum glyph pixels (outside guides) in each expected text region.
    pub min_text_ink_pixels: usize,
    /// A pixel counts as a glyph pixel when it is within this CIEDE2000
    /// distance of the text colour blended over the background ...
    pub text_blend_delta_e: f32,
    /// ... at a coverage of at least this fraction (0–1).
    pub min_text_coverage: f32,
    /// Minimum data-ink pixels (outside guides) inside the plot rectangle.
    pub min_mark_pixels: usize,
    /// Maximum stray ink pixels allowed outside the plot rectangle.
    pub max_stray_pixels: usize,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            ink_delta_e: 2.0,
            color_delta_e: 3.0,
            min_text_ink_pixels: 8,
            text_blend_delta_e: 6.0,
            min_text_coverage: 0.35,
            min_mark_pixels: 20,
            max_stray_pixels: 0,
        }
    }
}

fn is_ink(c: Rgba8, background: Rgba8, tol: &Tolerances) -> bool {
    c != background && c.delta_e(background) > tol.ink_delta_e
}

/// Whether `c` looks like `text` drawn over `background` with partial
/// coverage (anti-aliased glyph edge or solid glyph interior): it must be
/// ink, close to the background–text blend line, and at least
/// [`Tolerances::min_text_coverage`] of the way to the text colour.
fn is_glyph(c: Rgba8, text: Rgba8, background: Rgba8, tol: &Tolerances) -> bool {
    if !is_ink(c, background, tol) {
        return false;
    }
    let (c, t, b) = (c.over(background), text.over(background), background);
    let d = [
        t.r as f32 - b.r as f32,
        t.g as f32 - b.g as f32,
        t.b as f32 - b.b as f32,
    ];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    if len2 < 1.0 {
        // Text indistinguishable from the background: any ink counts.
        return true;
    }
    let p = [
        c.r as f32 - b.r as f32,
        c.g as f32 - b.g as f32,
        c.b as f32 - b.b as f32,
    ];
    let coverage = ((p[0] * d[0] + p[1] * d[1] + p[2] * d[2]) / len2).clamp(0.0, 1.0);
    let mix = |i: usize, base: u8| (base as f32 + coverage * d[i]).round().clamp(0.0, 255.0) as u8;
    let blend = Rgba8::rgb(mix(0, b.r), mix(1, b.g), mix(2, b.b));
    coverage >= tol.min_text_coverage && c.delta_e(blend) <= tol.text_blend_delta_e
}

/// Whether `check` has anything to assert for this layout. Checks that are
/// not applicable are not run (and so can neither fail nor unexpectedly
/// pass).
pub fn is_applicable(check: Check, layout: &LayoutMetadata) -> bool {
    match check {
        Check::TextPresent => !layout.text_regions.is_empty(),
        Check::ColorPresent => !layout.expected_colors.is_empty(),
        _ => true,
    }
}

/// Fail if the image is a single uniform colour, or matches the background
/// everywhere.
pub fn check_not_blank(
    img: &RgbaImage,
    layout: &LayoutMetadata,
    tol: &Tolerances,
) -> Result<(), CheckFailure> {
    if img.pixel_count() == 0 {
        return Err(CheckFailure::new(Check::NotBlank, "image has no pixels"));
    }
    let first = img.pixel(0, 0);
    if img.pixels().all(|(_, _, c)| c == first) {
        return Err(CheckFailure::new(
            Check::NotBlank,
            format!(
                "image is a solid fill of {first:?} ({}x{})",
                img.width(),
                img.height()
            ),
        ));
    }
    if !img
        .pixels()
        .any(|(_, _, c)| is_ink(c, layout.background, tol))
    {
        return Err(CheckFailure::new(
            Check::NotBlank,
            format!(
                "every pixel is within ΔE {} of the background {:?}",
                tol.ink_delta_e, layout.background
            ),
        ));
    }
    Ok(())
}

/// Fail if the plot rectangle contains fewer than
/// [`Tolerances::min_mark_pixels`] ink pixels outside guide regions, i.e.
/// the axes are drawn but no data is.
pub fn check_marks_present(
    img: &RgbaImage,
    layout: &LayoutMetadata,
    tol: &Tolerances,
) -> Result<(), CheckFailure> {
    let mut ink = 0usize;
    if let Some((x0, y0, x1, y1)) = layout.plot_rect.pixel_span(img.width(), img.height()) {
        for y in y0..y1 {
            for x in x0..x1 {
                if !layout.in_guide(x, y) && is_ink(img.pixel(x, y), layout.background, tol) {
                    ink += 1;
                }
            }
        }
    }
    if ink < tol.min_mark_pixels {
        return Err(CheckFailure::new(
            Check::MarksPresent,
            format!(
                "plot rect {} has {ink} data-ink pixels (need >= {}); the chart draws no visible marks",
                fmt_rect(&layout.plot_rect),
                tol.min_mark_pixels
            ),
        ));
    }
    Ok(())
}

/// Fail if any expected text region has fewer than
/// [`Tolerances::min_text_ink_pixels`] glyph pixels (pixels that look like
/// the region's text colour over the background) outside guide regions.
pub fn check_text_present(
    img: &RgbaImage,
    layout: &LayoutMetadata,
    tol: &Tolerances,
) -> Result<(), CheckFailure> {
    let mut missing = Vec::new();
    for region in &layout.text_regions {
        let mut ink = 0usize;
        if let Some((x0, y0, x1, y1)) = region.rect.pixel_span(img.width(), img.height()) {
            for y in y0..y1 {
                for x in x0..x1 {
                    if !layout.in_guide(x, y)
                        && is_glyph(img.pixel(x, y), region.color, layout.background, tol)
                    {
                        ink += 1;
                    }
                }
            }
        }
        if ink < tol.min_text_ink_pixels {
            missing.push(format!(
                "{:?} {:?} at {} ({ink} glyph px of {:?})",
                region.role,
                region.text,
                fmt_rect(&region.rect),
                region.color
            ));
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    let total = layout.text_regions.len();
    let shown: Vec<_> = missing.iter().take(4).cloned().collect();
    Err(CheckFailure::new(
        Check::TextPresent,
        format!(
            "{} of {total} text regions have no text (need >= {} glyph px each): {}{}",
            missing.len(),
            tol.min_text_ink_pixels,
            shown.join("; "),
            if missing.len() > shown.len() {
                "; ..."
            } else {
                ""
            }
        ),
    ))
}

/// Fail if more than [`Tolerances::max_stray_pixels`] ink pixels lie outside
/// the plot rectangle (inflated by [`LayoutMetadata::mark_overhang_px`]) and
/// outside every guide and text region.
pub fn check_marks_confined(
    img: &RgbaImage,
    layout: &LayoutMetadata,
    tol: &Tolerances,
) -> Result<(), CheckFailure> {
    let allowed = layout.plot_rect.inflate(layout.mark_overhang_px);
    let mut count = 0usize;
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    let mut sample = None;
    for (x, y, c) in img.pixels() {
        if allowed.contains_pixel(x, y)
            || !is_ink(c, layout.background, tol)
            || layout.in_guide(x, y)
            || layout.in_text(x, y)
        {
            continue;
        }
        count += 1;
        sample.get_or_insert((x, y, c));
        bbox = Some(match bbox {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }
    if count <= tol.max_stray_pixels {
        return Ok(());
    }
    let (x0, y0, x1, y1) = bbox.unwrap_or_default();
    let (sx, sy, sc) = sample.expect("count > 0 implies a sample");
    Err(CheckFailure::new(
        Check::MarksConfined,
        format!(
            "{count} ink pixels outside plot rect {} (+{} px overhang), spanning x {x0}..={x1}, y {y0}..={y1}; first at ({sx}, {sy}) = {sc:?}",
            fmt_rect(&layout.plot_rect),
            layout.mark_overhang_px
        ),
    ))
}

/// Fail if any explicitly configured colour has no pixel within
/// [`Tolerances::color_delta_e`] of it.
pub fn check_color_present(
    img: &RgbaImage,
    layout: &LayoutMetadata,
    tol: &Tolerances,
) -> Result<(), CheckFailure> {
    let mut missing = Vec::new();
    for expected in &layout.expected_colors {
        let mut best: Option<(f32, Rgba8)> = None;
        let mut found = false;
        // Only ink is a candidate, unless the expected colour is itself
        // indistinguishable from the background.
        let background_like = !is_ink(expected.color, layout.background, tol);
        for (_, _, c) in img.pixels() {
            if background_like || is_ink(c, layout.background, tol) {
                let d = c.delta_e(expected.color);
                if d <= tol.color_delta_e {
                    found = true;
                    break;
                }
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, c));
                }
            }
        }
        if !found {
            missing.push(match best {
                Some((d, c)) => format!(
                    "{} {:?}: closest ink is {c:?} (ΔE {d:.1})",
                    expected.label, expected.color
                ),
                None => format!("{} {:?}: no ink at all", expected.label, expected.color),
            });
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(CheckFailure::new(
        Check::ColorPresent,
        format!(
            "configured colour not rendered within ΔE {}: {}",
            tol.color_delta_e,
            missing.join("; ")
        ),
    ))
}

fn fmt_rect(r: &PxRect) -> String {
    format!("[{:.0},{:.0} {:.0}x{:.0}]", r.x, r.y, r.width, r.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::TextRole;

    const BLUE: Rgba8 = Rgba8::from_hex(0x1f77b4);
    const GREY: Rgba8 = Rgba8::rgb(80, 80, 80);

    /// A synthetic 200x100 "chart": plot rect [40,10 150x60], an axis line
    /// along the bottom of the plot, a title region and one tick label.
    fn synthetic() -> (RgbaImage, LayoutMetadata) {
        let mut img = RgbaImage::filled(200, 100, Rgba8::WHITE);
        let plot = PxRect::new(40.0, 10.0, 150.0, 60.0);
        let axis = PxRect::new(40.0, 70.0, 150.0, 1.0);
        let title = PxRect::new(80.0, 0.0, 60.0, 9.0);
        let tick = PxRect::new(30.0, 74.0, 20.0, 10.0);
        img.fill_rect(axis, GREY);
        // A data mark inside the plot.
        img.fill_rect(PxRect::new(60.0, 30.0, 10.0, 10.0), BLUE);
        // "Glyphs" in the title and tick label regions.
        img.fill_rect(PxRect::new(85.0, 2.0, 30.0, 4.0), Rgba8::BLACK);
        img.fill_rect(PxRect::new(35.0, 76.0, 6.0, 6.0), Rgba8::BLACK);
        let layout = LayoutMetadata::new(plot)
            .with_guide(axis)
            .with_text(TextRole::Title, "Title", title, Rgba8::BLACK)
            .with_text(TextRole::TickLabel, "0", tick, Rgba8::BLACK)
            .with_expected_color("mark fill", BLUE);
        (img, layout)
    }

    fn tol() -> Tolerances {
        Tolerances::default()
    }

    #[test]
    fn well_formed_synthetic_chart_passes_every_check() {
        let (img, layout) = synthetic();
        check_not_blank(&img, &layout, &tol()).unwrap();
        check_marks_present(&img, &layout, &tol()).unwrap();
        check_text_present(&img, &layout, &tol()).unwrap();
        check_marks_confined(&img, &layout, &tol()).unwrap();
        check_color_present(&img, &layout, &tol()).unwrap();
    }

    #[test]
    fn solid_fill_and_background_only_are_blank() {
        let layout = LayoutMetadata::new(PxRect::new(0.0, 0.0, 10.0, 10.0));
        let solid = RgbaImage::filled(10, 10, BLUE);
        let err = check_not_blank(&solid, &layout, &tol()).unwrap_err();
        assert!(err.message.contains("solid fill"), "{err}");

        // Near-background noise is still blank.
        let mut faint = RgbaImage::filled(10, 10, Rgba8::WHITE);
        faint.set_pixel(3, 3, Rgba8::rgb(254, 254, 254));
        let err = check_not_blank(&faint, &layout, &tol()).unwrap_err();
        assert!(err.message.contains("background"), "{err}");
    }

    #[test]
    fn axes_without_data_have_no_marks() {
        let (mut img, layout) = synthetic();
        img.fill_rect(PxRect::new(60.0, 30.0, 10.0, 10.0), Rgba8::WHITE);
        let err = check_marks_present(&img, &layout, &tol()).unwrap_err();
        assert_eq!(err.check, Check::MarksPresent);
        // The axis line inside the plot edge is a guide, not data.
        check_not_blank(&img, &layout, &tol()).unwrap();
    }

    #[test]
    fn missing_text_is_reported_per_region() {
        let (mut img, layout) = synthetic();
        img.fill_rect(PxRect::new(80.0, 0.0, 60.0, 9.0), Rgba8::WHITE);
        let err = check_text_present(&img, &layout, &tol()).unwrap_err();
        assert!(err.message.contains("1 of 2"), "{err}");
        assert!(err.message.contains("Title"), "{err}");
    }

    #[test]
    fn mark_fill_flooding_a_text_region_is_not_text() {
        let (mut img, layout) = synthetic();
        // Remove the title glyphs and flood the region with a pale mark
        // fill, as an exploded area chart does.
        img.fill_rect(PxRect::new(80.0, 0.0, 60.0, 9.0), Rgba8::rgb(204, 176, 225));
        let err = check_text_present(&img, &layout, &tol()).unwrap_err();
        assert!(err.message.contains("Title"), "{err}");
        // A saturated mark colour is not text either.
        img.fill_rect(PxRect::new(80.0, 0.0, 60.0, 9.0), BLUE);
        assert!(check_text_present(&img, &layout, &tol()).is_err());
    }

    #[test]
    fn anti_aliased_glyph_edges_count_as_text() {
        let (mut img, layout) = synthetic();
        // Replace the solid title glyphs with 50% grey coverage of black.
        img.fill_rect(PxRect::new(80.0, 0.0, 60.0, 9.0), Rgba8::WHITE);
        img.fill_rect(PxRect::new(85.0, 2.0, 30.0, 4.0), Rgba8::rgb(128, 128, 128));
        check_text_present(&img, &layout, &tol()).unwrap();
    }

    #[test]
    fn guide_ink_inside_a_text_region_does_not_count_as_text() {
        let (mut img, mut layout) = synthetic();
        // Remove the tick label glyphs, then draw a tick mark through the
        // label region and declare it as a guide.
        img.fill_rect(PxRect::new(35.0, 76.0, 6.0, 6.0), Rgba8::WHITE);
        let tick_mark = PxRect::new(39.0, 70.0, 1.0, 14.0);
        img.fill_rect(tick_mark, GREY);
        layout.guide_regions.push(tick_mark);
        let err = check_text_present(&img, &layout, &tol()).unwrap_err();
        assert!(err.message.contains("TickLabel"), "{err}");
    }

    #[test]
    fn marks_outside_the_plot_are_strays() {
        let (mut img, layout) = synthetic();
        img.fill_rect(PxRect::new(0.0, 90.0, 5.0, 5.0), BLUE);
        let err = check_marks_confined(&img, &layout, &tol()).unwrap_err();
        assert!(err.message.starts_with("25 ink pixels"), "{err}");
        assert!(err.message.contains("x 0..=4, y 90..=94"), "{err}");
    }

    #[test]
    fn overhang_allows_marks_straddling_the_plot_edge() {
        let (mut img, layout) = synthetic();
        // A 6px point centred on the plot's left edge.
        img.fill_rect(PxRect::new(37.0, 37.0, 6.0, 6.0), BLUE);
        assert!(check_marks_confined(&img, &layout, &tol()).is_err());
        let layout = layout.with_mark_overhang(3.0);
        check_marks_confined(&img, &layout, &tol()).unwrap();
    }

    #[test]
    fn double_gamma_encoded_colour_is_not_the_configured_colour() {
        let (mut img, layout) = synthetic();
        // What a double sRGB encode does to #1f77b4.
        img.fill_rect(
            PxRect::new(60.0, 30.0, 10.0, 10.0),
            Rgba8::rgb(142, 189, 219),
        );
        let err = check_color_present(&img, &layout, &tol()).unwrap_err();
        assert!(err.message.contains("closest ink"), "{err}");
    }

    #[test]
    fn applicability_depends_on_layout_content() {
        let layout = LayoutMetadata::new(PxRect::new(0.0, 0.0, 1.0, 1.0));
        assert!(!is_applicable(Check::TextPresent, &layout));
        assert!(!is_applicable(Check::ColorPresent, &layout));
        assert!(is_applicable(Check::MarksConfined, &layout));
    }

    #[test]
    fn check_names_round_trip() {
        for c in Check::ALL {
            assert_eq!(Check::from_name(c.name()), Some(c));
        }
        assert_eq!(Check::from_name("nope"), None);
    }
}
