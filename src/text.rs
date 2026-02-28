// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-accelerated text rendering system using SDF (Signed Distance Field) fonts.
//!
//! This module provides high-quality text rendering that looks crisp at all scales
//! and integrates seamlessly with the existing GPU rendering pipeline.
//!
//! # Features
//!
//! * **SDF Font Rendering** - Crisp text at all scale factors
//! * **Font Atlas Caching** - Efficient glyph reuse and memory management
//! * **System Font Loading** - Load fonts by family name from the OS
//! * **Unicode Support** - Full Unicode text rendering with normalization
//! * **Performance** - GPU-accelerated rendering with batching
//!
//! # Font Loading
//!
//! Fonts can be loaded from multiple sources:
//!
//! - **Embedded default font**: `FontAtlas::new(device, queue, font_size)`
//! - **System font by name**: `FontAtlas::with_font(device, queue, font_size, &spec, &font_db)`
//! - **Raw font data**: `FontAtlas::from_data(device, queue, font_size, data)`
//!
//! The `FontDatabase` handles system font discovery and caching across platforms.
//!
//! # Examples
//!
//! ```ignore
//! use gup::text::{TextRenderer, TextStyle, FontAtlas, TextLayoutEngine, TextRenderConfig};
//! use gup::text::{FontSpec, FontDatabase};
//! use gup::text::font::FontWeight;
//! use gup::shader_function::Vec2;
//!
//! // Load default embedded font
//! // let atlas = FontAtlas::new(device, queue, 16.0)?;
//!
//! // Load a system font
//! // let font_db = FontDatabase::new();
//! // let spec = FontSpec::new("Arial").with_weight(FontWeight::Bold);
//! // let atlas = FontAtlas::with_font(device, queue, 16.0, &spec, &font_db)?;
//! ```

pub mod atlas;
pub mod font;
pub mod hover_reveal;
pub mod layout;
pub mod msdf;
pub mod renderer;
pub mod sdf_tuning;
pub mod style;
pub mod tooltip_bg;
pub mod ui_quad;

pub use atlas::*;
pub use font::*;
pub use hover_reveal::*;
pub use layout::*;
pub use msdf::*;
pub use renderer::*;
pub use sdf_tuning::*;
pub use style::*;
pub use tooltip_bg::*;
pub use ui_quad::*;

use crate::shader_function::{Vec2, Vec4};

/// Trait for types that can provide glyph information and font metrics.
///
/// This enables text layout algorithms to work with both the GPU-backed
/// [`FontAtlas`] and lightweight mock implementations for testing.
pub trait GlyphSource {
    /// Get font metrics for layout calculations.
    fn metrics(&self) -> &FontMetrics;

    /// Look up glyph information for a character.
    fn get_glyph(&self, character: char) -> Option<&GlyphInfo>;
}

impl GlyphSource for FontAtlas {
    fn metrics(&self) -> &FontMetrics {
        FontAtlas::metrics(self)
    }

    fn get_glyph(&self, character: char) -> Option<&GlyphInfo> {
        FontAtlas::get_glyph(self, character)
    }
}

/// Text anchor point for positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAnchor {
    /// Top-left corner
    #[default]
    TopLeft,
    /// Top-center
    TopCenter,
    /// Top-right corner
    TopRight,
    /// Center-left
    CenterLeft,
    /// Center
    Center,
    /// Center-right
    CenterRight,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom-center
    BottomCenter,
    /// Bottom-right corner
    BottomRight,
}

impl TextAnchor {
    /// Get the anchor offset as normalized coordinates (0.0 to 1.0).
    pub fn offset(&self) -> Vec2 {
        match self {
            TextAnchor::TopLeft => Vec2 { x: 0.0, y: 0.0 },
            TextAnchor::TopCenter => Vec2 { x: 0.5, y: 0.0 },
            TextAnchor::TopRight => Vec2 { x: 1.0, y: 0.0 },
            TextAnchor::CenterLeft => Vec2 { x: 0.0, y: 0.5 },
            TextAnchor::Center => Vec2 { x: 0.5, y: 0.5 },
            TextAnchor::CenterRight => Vec2 { x: 1.0, y: 0.5 },
            TextAnchor::BottomLeft => Vec2 { x: 0.0, y: 1.0 },
            TextAnchor::BottomCenter => Vec2 { x: 0.5, y: 1.0 },
            TextAnchor::BottomRight => Vec2 { x: 1.0, y: 1.0 },
        }
    }
}

/// Horizontal alignment for multi-line text.
///
/// Controls how each line is positioned horizontally within the text block.
/// This is independent of [`TextAnchor`], which positions the entire block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlignment {
    /// Align each line to the left edge (default).
    #[default]
    Left,
    /// Center each line horizontally.
    Center,
    /// Align each line to the right edge.
    Right,
    /// Distribute extra space between words to fill the line width.
    ///
    /// Single-word lines and the last line of a paragraph fall back to
    /// left alignment.
    Justify,
}

/// Bounding rectangle for text layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBounds {
    /// Left edge
    pub left: f32,
    /// Top edge
    pub top: f32,
    /// Right edge
    pub right: f32,
    /// Bottom edge
    pub bottom: f32,
}

impl TextBounds {
    /// Create new text bounds.
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Get the width of the bounds.
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Get the height of the bounds.
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Get the center point.
    pub fn center(&self) -> Vec2 {
        Vec2 {
            x: (self.left + self.right) * 0.5,
            y: (self.top + self.bottom) * 0.5,
        }
    }

    /// Check if this bounds intersects with another.
    pub fn intersects(&self, other: &TextBounds) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }

    /// Expand bounds to include another bounds.
    pub fn union(&mut self, other: &TextBounds) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }

    /// Check if this bounds fully contains another bounds.
    pub fn contains(&self, other: &TextBounds) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }

    /// Check if this bounds contains a point.
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

impl Default for TextBounds {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }
}

/// Font metrics for layout calculations.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Font size in pixels
    pub size: f32,
    /// Height of a line of text
    pub line_height: f32,
    /// Distance from baseline to top of tallest glyph
    pub ascent: f32,
    /// Distance from baseline to bottom of lowest glyph
    pub descent: f32,
    /// Additional space between lines
    pub line_gap: f32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            size: 16.0,
            line_height: 18.0,
            ascent: 14.0,
            descent: 4.0,
            line_gap: 2.0,
        }
    }
}

/// Information about a glyph in the font atlas.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// Character this glyph represents
    pub character: char,
    /// Position in atlas texture (UV coordinates)
    pub atlas_pos: [f32; 4], // [u_min, v_min, u_max, v_max]
    /// Glyph dimensions in pixels
    pub size: Vec2,
    /// Bearing (offset from baseline)
    pub bearing: Vec2,
    /// Horizontal advance for cursor positioning
    pub advance: f32,
    /// SDF distance scale factor
    pub sdf_scale: f32,
}

impl Default for GlyphInfo {
    fn default() -> Self {
        Self {
            character: ' ',
            atlas_pos: [0.0, 0.0, 0.0, 0.0],
            size: Vec2 { x: 0.0, y: 0.0 },
            bearing: Vec2 { x: 0.0, y: 0.0 },
            advance: 8.0,
            sdf_scale: 1.0,
        }
    }
}

/// Positioned glyph for rendering.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    /// Glyph information
    pub glyph: GlyphInfo,
    /// Position in screen coordinates
    pub position: Vec2,
    /// Color (RGBA)
    pub color: Vec4,
}

/// Collection of positioned glyphs ready for rendering.
pub type GlyphBatch = Vec<PositionedGlyph>;

/// Constants for SDF rendering
pub mod sdf {
    /// SDF texture size for the atlas
    pub const ATLAS_SIZE: u32 = 1024;

    /// Padding around each glyph in the atlas
    pub const GLYPH_PADDING: u32 = 4;

    /// SDF range (distance field extends this many pixels)
    pub const SDF_RANGE: f32 = 8.0;

    /// Legacy smoothing factor for SDF edges.
    ///
    /// **Superseded** by [`SdfTuningParams::for_font_size`] which provides
    /// per-size adaptive smoothing. Kept for reference only.
    pub const SDF_SMOOTHING: f32 = 0.5;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_anchor_offset() {
        assert_eq!(TextAnchor::TopLeft.offset(), Vec2 { x: 0.0, y: 0.0 });
        assert_eq!(TextAnchor::Center.offset(), Vec2 { x: 0.5, y: 0.5 });
        assert_eq!(TextAnchor::BottomRight.offset(), Vec2 { x: 1.0, y: 1.0 });
    }

    #[test]
    fn test_text_bounds_basic() {
        let bounds = TextBounds::new(10.0, 20.0, 50.0, 60.0);
        assert_eq!(bounds.width(), 40.0);
        assert_eq!(bounds.height(), 40.0);
        assert_eq!(bounds.center(), Vec2 { x: 30.0, y: 40.0 });
    }

    #[test]
    fn test_text_bounds_intersection() {
        let bounds1 = TextBounds::new(0.0, 0.0, 20.0, 20.0);
        let bounds2 = TextBounds::new(10.0, 10.0, 30.0, 30.0);
        let bounds3 = TextBounds::new(25.0, 25.0, 35.0, 35.0);

        assert!(bounds1.intersects(&bounds2));
        assert!(!bounds1.intersects(&bounds3));
    }

    #[test]
    fn test_text_bounds_union() {
        let mut bounds1 = TextBounds::new(0.0, 0.0, 20.0, 20.0);
        let bounds2 = TextBounds::new(15.0, 15.0, 35.0, 35.0);

        bounds1.union(&bounds2);
        assert_eq!(bounds1.left, 0.0);
        assert_eq!(bounds1.top, 0.0);
        assert_eq!(bounds1.right, 35.0);
        assert_eq!(bounds1.bottom, 35.0);
    }

    #[test]
    fn test_text_bounds_contains() {
        let outer = TextBounds::new(0.0, 0.0, 100.0, 100.0);
        let inner = TextBounds::new(10.0, 10.0, 50.0, 50.0);
        let overlapping = TextBounds::new(50.0, 50.0, 150.0, 150.0);

        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
        assert!(!outer.contains(&overlapping));
    }

    #[test]
    fn test_text_bounds_contains_point() {
        let bounds = TextBounds::new(10.0, 20.0, 50.0, 60.0);
        assert!(bounds.contains_point(30.0, 40.0)); // Inside
        assert!(bounds.contains_point(10.0, 20.0)); // On edge
        assert!(!bounds.contains_point(5.0, 40.0)); // Outside left
        assert!(!bounds.contains_point(30.0, 70.0)); // Outside bottom
    }

    #[test]
    fn test_font_metrics_default() {
        let metrics = FontMetrics::default();
        assert_eq!(metrics.size, 16.0);
        assert!(metrics.line_height >= metrics.size);
        assert!(metrics.ascent > 0.0);
        assert!(metrics.descent > 0.0);
    }

    #[test]
    fn test_glyph_info_default() {
        let glyph = GlyphInfo::default();
        assert_eq!(glyph.character, ' ');
        assert!(glyph.advance > 0.0);
        assert_eq!(glyph.sdf_scale, 1.0);
    }
}
