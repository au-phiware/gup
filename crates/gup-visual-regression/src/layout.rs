// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Layout metadata: what a renderer claims about where things should be.
//!
//! A renderer adapter fills in a [`LayoutMetadata`] from its own layout
//! model (plot rectangle, text placement, axis geometry, configured colours)
//! and the structural checks compare that claim with the pixels.

use crate::color::Rgba8;

/// An axis-aligned rectangle in image pixel coordinates (origin top-left,
/// y down). Edges are continuous coordinates: pixel `(x, y)` covers
/// `[x, x+1) × [y, y+1)` and has its centre at `(x + 0.5, y + 0.5)`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PxRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width (non-negative).
    pub width: f32,
    /// Height (non-negative).
    pub height: f32,
}

impl PxRect {
    /// Create a rectangle from its top-left corner and size.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    /// Create a rectangle from its four edges (in any order).
    pub fn from_edges(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        let (l, r) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (t, b) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        Self::new(l, t, r - l, b - t)
    }

    /// Left edge.
    pub fn left(&self) -> f32 {
        self.x
    }

    /// Top edge.
    pub fn top(&self) -> f32 {
        self.y
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether the centre of pixel `(px, py)` lies inside the rectangle.
    pub fn contains_pixel(&self, px: u32, py: u32) -> bool {
        let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
        cx >= self.left() && cx < self.right() && cy >= self.top() && cy < self.bottom()
    }

    /// Grow (positive) or shrink (negative) the rectangle on every side.
    pub fn inflate(&self, by: f32) -> Self {
        Self::new(
            self.x - by,
            self.y - by,
            self.width + 2.0 * by,
            self.height + 2.0 * by,
        )
    }

    /// Area in square pixels.
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// The half-open pixel index range `(x0, y0, x1, y1)` of pixels whose
    /// centres lie inside the rectangle, clipped to a `width × height`
    /// image. `None` if no pixel is covered.
    pub fn pixel_span(&self, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
        let lo = |edge: f32, max: u32| ((edge - 0.5).ceil().max(0.0) as u32).min(max);
        let x0 = lo(self.left(), width);
        let y0 = lo(self.top(), height);
        let x1 = lo(self.right(), width);
        let y1 = lo(self.bottom(), height);
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }
}

/// The semantic role of a piece of text, used in failure messages and to
/// let adapters describe whatever text their renderer produces.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// The chart title.
    Title,
    /// The chart subtitle.
    Subtitle,
    /// An axis tick label.
    TickLabel,
    /// An axis title.
    AxisTitle,
    /// A legend entry or legend title.
    Legend,
    /// Any other text.
    Other,
}

/// A region where a renderer has placed (or should have placed) text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRegion {
    /// What the text is.
    pub role: TextRole,
    /// The text content (for messages).
    pub text: String,
    /// Where the glyphs should be, in image pixels. Adapters should make
    /// this a reasonably tight estimate of the glyph box.
    pub rect: PxRect,
    /// The configured text colour. Only pixels that look like this colour
    /// blended over the background (glyph coverage) count as text, so data
    /// marks spilling into the region cannot pass for text.
    pub color: Rgba8,
}

/// A colour the renderer was explicitly configured to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpectedColor {
    /// Human-readable description (e.g. `"bar fill"`).
    pub label: String,
    /// The configured colour (sRGB-encoded).
    pub color: Rgba8,
}

/// Everything the structural checks need to know about the intended layout
/// of an image, independent of which renderer produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutMetadata {
    /// The colour the image is cleared to.
    pub background: Rgba8,
    /// The plot (data) rectangle. Data marks must stay inside it.
    pub plot_rect: PxRect,
    /// Expected text regions (title, tick labels, legend, ...).
    pub text_regions: Vec<TextRegion>,
    /// Regions occupied by non-data guides (axis lines, tick marks, grid
    /// lines, colour bars). Ink here is neither a data mark nor text.
    pub guide_regions: Vec<PxRect>,
    /// Colours that were explicitly configured and must appear.
    pub expected_colors: Vec<ExpectedColor>,
    /// How far data marks may legitimately extend past the plot rectangle,
    /// in pixels (e.g. the radius of a point drawn on the domain edge while
    /// marks are not clipped). Keep this tied to a configured mark size.
    pub mark_overhang_px: f32,
}

impl LayoutMetadata {
    /// Metadata for an image with the given plot rectangle, a white
    /// background and no text, guides or expected colours.
    pub fn new(plot_rect: PxRect) -> Self {
        Self {
            background: Rgba8::WHITE,
            plot_rect,
            text_regions: Vec::new(),
            guide_regions: Vec::new(),
            expected_colors: Vec::new(),
            mark_overhang_px: 0.0,
        }
    }

    /// Set the background colour.
    pub fn with_background(mut self, background: Rgba8) -> Self {
        self.background = background;
        self
    }

    /// Add an expected text region drawn in `color`.
    pub fn with_text(
        mut self,
        role: TextRole,
        text: impl Into<String>,
        rect: PxRect,
        color: Rgba8,
    ) -> Self {
        self.text_regions.push(TextRegion {
            role,
            text: text.into(),
            rect,
            color,
        });
        self
    }

    /// Add a guide region (axis line, tick, grid line, colour bar).
    pub fn with_guide(mut self, rect: PxRect) -> Self {
        self.guide_regions.push(rect);
        self
    }

    /// Add an explicitly configured colour that must appear.
    pub fn with_expected_color(mut self, label: impl Into<String>, color: Rgba8) -> Self {
        self.expected_colors.push(ExpectedColor {
            label: label.into(),
            color,
        });
        self
    }

    /// Allow marks to extend this many pixels past the plot rectangle.
    pub fn with_mark_overhang(mut self, px: f32) -> Self {
        self.mark_overhang_px = px.max(0.0);
        self
    }

    /// Whether pixel `(x, y)` lies in any guide region.
    pub fn in_guide(&self, x: u32, y: u32) -> bool {
        self.guide_regions.iter().any(|r| r.contains_pixel(x, y))
    }

    /// Whether pixel `(x, y)` lies in any text region.
    pub fn in_text(&self, x: u32, y: u32) -> bool {
        self.text_regions
            .iter()
            .any(|t| t.rect.contains_pixel(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_span_uses_pixel_centres() {
        let r = PxRect::new(1.0, 1.0, 2.0, 2.0);
        assert_eq!(r.pixel_span(10, 10), Some((1, 1, 3, 3)));
        assert!(r.contains_pixel(1, 1));
        assert!(r.contains_pixel(2, 2));
        assert!(!r.contains_pixel(3, 1));
        // Clipped to the image.
        assert_eq!(
            PxRect::new(-5.0, 8.0, 10.0, 10.0).pixel_span(10, 10),
            Some((0, 8, 5, 10))
        );
        assert_eq!(PxRect::new(20.0, 0.0, 5.0, 5.0).pixel_span(10, 10), None);
    }

    #[test]
    fn from_edges_normalises_order() {
        let r = PxRect::from_edges(5.0, 8.0, 1.0, 2.0);
        assert_eq!(r, PxRect::new(1.0, 2.0, 4.0, 6.0));
    }
}
