// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text styling and appearance configuration.

use super::*;
use crate::shader_function::Vec4;

/// Text styling configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font size in pixels
    pub font_size: f32,
    /// Text color (RGBA)
    pub color: Vec4,
    /// Text anchor point
    pub anchor: TextAnchor,
    /// Rotation angle in radians
    pub rotation: f32,
    /// Whether to enable anti-aliasing
    pub antialiased: bool,
    /// Font weight (0.0 = thin, 1.0 = bold)
    pub weight: f32,
    /// Letter spacing multiplier
    pub letter_spacing: f32,
    /// Line spacing multiplier
    pub line_spacing: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            }, // Black
            anchor: TextAnchor::TopLeft,
            rotation: 0.0,
            antialiased: true,
            weight: 0.5, // Normal weight
            letter_spacing: 1.0,
            line_spacing: 1.0,
        }
    }
}

impl TextStyle {
    /// Create a new text style with specified font size.
    pub fn new(font_size: f32) -> Self {
        Self {
            font_size,
            ..Default::default()
        }
    }

    /// Set the text color.
    pub fn with_color(mut self, color: Vec4) -> Self {
        self.color = color;
        self
    }

    /// Set the text color from RGBA values.
    pub fn with_rgba(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = Vec4 {
            x: r,
            y: g,
            z: b,
            w: a,
        };
        self
    }

    /// Set the text anchor point.
    pub fn with_anchor(mut self, anchor: TextAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Set the rotation angle in radians.
    pub fn with_rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }

    /// Set the rotation angle in degrees.
    pub fn with_rotation_degrees(mut self, degrees: f32) -> Self {
        self.rotation = degrees * std::f32::consts::PI / 180.0;
        self
    }

    /// Set font weight.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Make text bold.
    pub fn bold(mut self) -> Self {
        self.weight = 1.0;
        self
    }

    /// Make text thin.
    pub fn thin(mut self) -> Self {
        self.weight = 0.0;
        self
    }

    /// Set letter spacing multiplier.
    pub fn with_letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing.max(0.0);
        self
    }

    /// Set line spacing multiplier.
    pub fn with_line_spacing(mut self, spacing: f32) -> Self {
        self.line_spacing = spacing.max(0.1);
        self
    }

    /// Disable anti-aliasing.
    pub fn without_antialiasing(mut self) -> Self {
        self.antialiased = false;
        self
    }

    /// Check if the text should be rotated.
    pub fn is_rotated(&self) -> bool {
        self.rotation.abs() > 0.001
    }

    /// Get the rotation matrix for this style.
    pub fn rotation_matrix(&self) -> [[f32; 2]; 2] {
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();
        [[cos_r, -sin_r], [sin_r, cos_r]]
    }

    /// Apply rotation to a point.
    pub fn rotate_point(&self, point: Vec2) -> Vec2 {
        if !self.is_rotated() {
            return point;
        }

        let matrix = self.rotation_matrix();
        Vec2 {
            x: matrix[0][0] * point.x + matrix[0][1] * point.y,
            y: matrix[1][0] * point.x + matrix[1][1] * point.y,
        }
    }
}

/// Pre-defined text style presets.
impl TextStyle {
    /// Large title style.
    pub fn title() -> Self {
        Self::new(24.0).bold()
    }

    /// Medium heading style.
    pub fn heading() -> Self {
        Self::new(20.0).with_weight(0.7)
    }

    /// Normal body text style.
    pub fn body() -> Self {
        Self::new(16.0)
    }

    /// Small caption style.
    pub fn caption() -> Self {
        Self::new(12.0).with_rgba(0.5, 0.5, 0.5, 1.0)
    }

    /// Axis label style.
    pub fn axis_label() -> Self {
        Self::new(14.0).with_rgba(0.2, 0.2, 0.2, 1.0)
    }

    /// Axis title style.
    pub fn axis_title() -> Self {
        Self::new(16.0)
            .with_weight(0.6)
            .with_rgba(0.1, 0.1, 0.1, 1.0)
    }

    /// Error text style.
    pub fn error() -> Self {
        Self::new(16.0).with_rgba(0.8, 0.2, 0.2, 1.0)
    }

    /// Success text style.
    pub fn success() -> Self {
        Self::new(16.0).with_rgba(0.2, 0.7, 0.2, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_style_default() {
        let style = TextStyle::default();
        assert_eq!(style.font_size, 16.0);
        assert_eq!(style.anchor, TextAnchor::TopLeft);
        assert_eq!(style.rotation, 0.0);
        assert!(style.antialiased);
        assert_eq!(style.weight, 0.5);
        assert_eq!(style.letter_spacing, 1.0);
        assert_eq!(style.line_spacing, 1.0);
    }

    #[test]
    fn test_text_style_builder() {
        let style = TextStyle::new(20.0)
            .with_rgba(1.0, 0.0, 0.0, 1.0)
            .with_anchor(TextAnchor::Center)
            .with_rotation_degrees(45.0)
            .bold();

        assert_eq!(style.font_size, 20.0);
        assert_eq!(
            style.color,
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0
            }
        );
        assert_eq!(style.anchor, TextAnchor::Center);
        assert!((style.rotation - std::f32::consts::PI / 4.0).abs() < 0.001);
        assert_eq!(style.weight, 1.0);
    }

    #[test]
    fn test_rotation_matrix() {
        let style = TextStyle::default().with_rotation_degrees(90.0);
        let matrix = style.rotation_matrix();

        // 90-degree rotation matrix should be approximately [[0, -1], [1, 0]]
        assert!((matrix[0][0] - 0.0).abs() < 0.001);
        assert!((matrix[0][1] - (-1.0)).abs() < 0.001);
        assert!((matrix[1][0] - 1.0).abs() < 0.001);
        assert!((matrix[1][1] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_rotate_point() {
        let style = TextStyle::default().with_rotation_degrees(90.0);
        let point = Vec2 { x: 1.0, y: 0.0 };
        let rotated = style.rotate_point(point);

        // (1,0) rotated 90 degrees should be approximately (0,1)
        assert!(rotated.x.abs() < 0.001);
        assert!((rotated.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_predefined_styles() {
        let title = TextStyle::title();
        assert_eq!(title.font_size, 24.0);
        assert_eq!(title.weight, 1.0);

        let caption = TextStyle::caption();
        assert_eq!(caption.font_size, 12.0);
        assert_eq!(caption.color.x, 0.5); // Gray color

        let axis_label = TextStyle::axis_label();
        assert_eq!(axis_label.font_size, 14.0);
    }

    #[test]
    fn test_weight_clamping() {
        let style = TextStyle::default().with_weight(-1.0);
        assert_eq!(style.weight, 0.0);

        let style = TextStyle::default().with_weight(2.0);
        assert_eq!(style.weight, 1.0);
    }

    #[test]
    fn test_spacing_constraints() {
        let style = TextStyle::default().with_letter_spacing(-1.0);
        assert_eq!(style.letter_spacing, 0.0);

        let style = TextStyle::default().with_line_spacing(0.05);
        assert_eq!(style.line_spacing, 0.1); // Minimum line spacing
    }

    #[test]
    fn test_is_rotated() {
        let style = TextStyle::default();
        assert!(!style.is_rotated());

        let style = style.with_rotation(0.01);
        assert!(style.is_rotated());

        let style = style.with_rotation(0.0005);
        assert!(!style.is_rotated()); // Below threshold
    }
}
