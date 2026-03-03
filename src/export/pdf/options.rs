// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF export options: page size, orientation, and margins.

/// Page orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Taller than wide.
    Portrait,
    /// Wider than tall.
    Landscape,
}

/// Options controlling PDF document generation.
///
/// Use the named constructors [`PdfOptions::a4`] or [`PdfOptions::letter`]
/// for common page sizes, or [`PdfOptions::custom`] for an arbitrary size
/// in millimetres.
///
/// # Examples
///
/// ```rust
/// use gup::export::pdf::{Orientation, PdfOptions};
///
/// let opts = PdfOptions::a4()
///     .orientation(Orientation::Landscape)
///     .margin_mm(15.0);
/// ```
#[derive(Debug, Clone)]
pub struct PdfOptions {
    /// Page width in millimetres (before orientation is applied).
    pub width_mm: f32,
    /// Page height in millimetres (before orientation is applied).
    pub height_mm: f32,
    /// Page orientation.
    pub page_orientation: Orientation,
    /// Uniform margin on all four sides, in millimetres.
    pub margin: f32,
}

impl PdfOptions {
    /// ISO A4 (210 × 297 mm), portrait, 10 mm margin.
    pub fn a4() -> Self {
        Self {
            width_mm: 210.0,
            height_mm: 297.0,
            page_orientation: Orientation::Portrait,
            margin: 10.0,
        }
    }

    /// US Letter (215.9 × 279.4 mm), portrait, 10 mm margin.
    pub fn letter() -> Self {
        Self {
            width_mm: 215.9,
            height_mm: 279.4,
            page_orientation: Orientation::Portrait,
            margin: 10.0,
        }
    }

    /// Custom page size in millimetres, portrait, 10 mm margin.
    pub fn custom(width_mm: f32, height_mm: f32) -> Self {
        Self {
            width_mm,
            height_mm,
            page_orientation: Orientation::Portrait,
            margin: 10.0,
        }
    }

    /// Set the page orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.page_orientation = orientation;
        self
    }

    /// Set a uniform margin (in millimetres) on all four sides.
    pub fn margin_mm(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    /// Effective page width in mm after applying orientation.
    pub fn effective_width_mm(&self) -> f32 {
        match self.page_orientation {
            Orientation::Portrait => self.width_mm,
            Orientation::Landscape => self.height_mm,
        }
    }

    /// Effective page height in mm after applying orientation.
    pub fn effective_height_mm(&self) -> f32 {
        match self.page_orientation {
            Orientation::Portrait => self.height_mm,
            Orientation::Landscape => self.width_mm,
        }
    }

    /// Drawable width in mm (page width minus left and right margins).
    pub fn drawable_width_mm(&self) -> f32 {
        self.effective_width_mm() - 2.0 * self.margin
    }

    /// Drawable height in mm (page height minus top and bottom margins).
    pub fn drawable_height_mm(&self) -> f32 {
        self.effective_height_mm() - 2.0 * self.margin
    }

    /// Compute the uniform scale factor that fits a chart of the given
    /// pixel dimensions into the drawable area while preserving aspect
    /// ratio.  Returns `(scale, offset_x_mm, offset_y_mm)` where the
    /// offsets centre the chart within the drawable region.
    pub fn fit_scale(&self, chart_width_px: f32, chart_height_px: f32) -> (f32, f32, f32) {
        let dw = self.drawable_width_mm();
        let dh = self.drawable_height_mm();

        let scale_x = dw / chart_width_px;
        let scale_y = dh / chart_height_px;
        let scale = scale_x.min(scale_y);

        let rendered_w = chart_width_px * scale;
        let rendered_h = chart_height_px * scale;

        // Centre the chart within the drawable area.
        let offset_x = self.margin + (dw - rendered_w) / 2.0;
        let offset_y = self.margin + (dh - rendered_h) / 2.0;

        (scale, offset_x, offset_y)
    }
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self::a4()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a4_dimensions() {
        let opts = PdfOptions::a4();
        assert!((opts.width_mm - 210.0).abs() < f32::EPSILON);
        assert!((opts.height_mm - 297.0).abs() < f32::EPSILON);
        assert_eq!(opts.page_orientation, Orientation::Portrait);
        assert!((opts.margin - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_letter_dimensions() {
        let opts = PdfOptions::letter();
        assert!((opts.width_mm - 215.9).abs() < f32::EPSILON);
        assert!((opts.height_mm - 279.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_custom_dimensions() {
        let opts = PdfOptions::custom(100.0, 200.0);
        assert!((opts.width_mm - 100.0).abs() < f32::EPSILON);
        assert!((opts.height_mm - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_landscape_swaps_dimensions() {
        let opts = PdfOptions::a4().orientation(Orientation::Landscape);
        // A4 landscape: effective width = 297, effective height = 210
        assert!((opts.effective_width_mm() - 297.0).abs() < f32::EPSILON);
        assert!((opts.effective_height_mm() - 210.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_portrait_keeps_dimensions() {
        let opts = PdfOptions::a4().orientation(Orientation::Portrait);
        assert!((opts.effective_width_mm() - 210.0).abs() < f32::EPSILON);
        assert!((opts.effective_height_mm() - 297.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_drawable_area() {
        let opts = PdfOptions::a4(); // 210×297, 10mm margin
        // drawable = 210 - 20 = 190, 297 - 20 = 277
        assert!((opts.drawable_width_mm() - 190.0).abs() < f32::EPSILON);
        assert!((opts.drawable_height_mm() - 277.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_custom_margin() {
        let opts = PdfOptions::a4().margin_mm(20.0);
        assert!((opts.margin - 20.0).abs() < f32::EPSILON);
        assert!((opts.drawable_width_mm() - 170.0).abs() < f32::EPSILON);
        assert!((opts.drawable_height_mm() - 257.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fit_scale_exact_fit() {
        // Chart is 190×277 px → drawable 190×277 mm → scale = 1.0
        let opts = PdfOptions::a4();
        let (scale, ox, oy) = opts.fit_scale(190.0, 277.0);
        assert!((scale - 1.0).abs() < 1e-4, "scale={scale}");
        assert!((ox - 10.0).abs() < 1e-4, "ox={ox}");
        assert!((oy - 10.0).abs() < 1e-4, "oy={oy}");
    }

    #[test]
    fn test_fit_scale_wide_chart() {
        // Chart is 800×600 px on A4 portrait (drawable 190×277 mm).
        // scale_x = 190/800 = 0.2375, scale_y = 277/600 ≈ 0.4617
        // min → scale = 0.2375
        let opts = PdfOptions::a4();
        let (scale, _ox, _oy) = opts.fit_scale(800.0, 600.0);
        assert!((scale - 190.0 / 800.0).abs() < 1e-4, "scale={scale}");
    }

    #[test]
    fn test_fit_scale_centres_chart() {
        // Chart 400×400 on A4 (drawable 190×277).
        // scale = min(190/400, 277/400) = 0.475
        // rendered = 190×190 mm, centred in 190×277:
        // offset_x = 10 + 0 = 10, offset_y = 10 + (277-190)/2 = 53.5
        let opts = PdfOptions::a4();
        let (scale, ox, oy) = opts.fit_scale(400.0, 400.0);
        assert!((scale - 0.475).abs() < 1e-4, "scale={scale}");
        assert!((ox - 10.0).abs() < 1e-4, "ox={ox}");
        assert!((oy - 53.5).abs() < 1e-4, "oy={oy}");
    }

    #[test]
    fn test_default_is_a4() {
        let opts = PdfOptions::default();
        assert!((opts.width_mm - 210.0).abs() < f32::EPSILON);
        assert!((opts.height_mm - 297.0).abs() < f32::EPSILON);
    }
}
