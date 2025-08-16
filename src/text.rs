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
//! * **Unicode Support** - Full Unicode text rendering with normalization
//! * **Performance** - GPU-accelerated rendering with batching
//!
//! # Examples
//!
//! ```ignore
//! use gup::text::{TextRenderer, TextStyle, FontAtlas, TextLayoutEngine, TextRenderConfig};
//! use gup::shader_function::Vec2;
//!
//! // Example showing text rendering API (actual implementation requires window setup)
//! async fn example() {
//!     // Initialize text rendering components
//!     // let mut font_atlas = FontAtlas::new(device, queue, "Arial", 16.0)?;
//!     // let mut renderer = TextRenderer::new(device)?;
//!     // let mut layout_engine = TextLayoutEngine::new();
//!
//!     let style = TextStyle::default();
//!     let text = "Hello, World!";
//!     let position = Vec2 { x: 10.0, y: 20.0 };
//!
//!     // Configuration for text rendering
//!     // let config = TextRenderConfig {
//!     //     text,
//!     //     position,
//!     //     style: &style,
//!     //     font_atlas: &mut font_atlas,
//!     //     layout_engine: &mut layout_engine,
//!     //     screen_width: 800.0,
//!     //     screen_height: 600.0,
//!     // };
//!
//!     // Render the text within a frame
//!     // let bounds = renderer.render_text(&mut frame, config)?;
//! }
//! ```

pub mod atlas;
pub mod layout;
pub mod renderer;
pub mod style;

pub use atlas::*;
pub use layout::*;
pub use renderer::*;
pub use style::*;

use crate::shader_function::{Vec2, Vec4};

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

    /// Smoothing factor for SDF edges
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
