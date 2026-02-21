// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! High contrast and alternative rendering modes for visual accessibility.
//!
//! This module provides rendering alternatives for users with visual impairments,
//! including high contrast modes, colorblind-friendly palettes, and pattern-based
//! alternatives to color.

use std::collections::HashMap;

/// Color representation for accessibility rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    /// Create a new color.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create a color from RGB values (0-255).
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Calculate relative luminance for contrast calculations.
    pub fn relative_luminance(&self) -> f32 {
        // Simplified luminance calculation
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }
}

/// High contrast renderer for visual accessibility.
#[derive(Debug)]
pub struct HighContrastRenderer {
    /// Current contrast mode
    contrast_mode: ContrastMode,

    /// Color replacement map
    color_replacements: HashMap<String, Color>,

    /// Pattern library for pattern-based rendering
    #[allow(dead_code)]
    pattern_library: PatternLibrary,
}

impl HighContrastRenderer {
    /// Create a new high contrast renderer with the specified mode.
    pub fn new(mode: ContrastMode) -> Self {
        let mut renderer = Self {
            contrast_mode: mode.clone(),
            color_replacements: HashMap::new(),
            pattern_library: PatternLibrary::new(),
        };

        // Initialize color replacements based on mode
        renderer.initialize_color_replacements(&mode);
        renderer
    }

    /// Get the current contrast mode.
    pub fn contrast_mode(&self) -> &ContrastMode {
        &self.contrast_mode
    }

    /// Set a new contrast mode.
    pub fn set_contrast_mode(&mut self, mode: ContrastMode) {
        self.contrast_mode = mode.clone();
        self.initialize_color_replacements(&mode);
    }

    /// Get color replacement for a given color.
    pub fn get_color_replacement(&self, original: &Color) -> Color {
        // For now, use mode-specific replacements
        match &self.contrast_mode {
            ContrastMode::HighContrast => {
                // High contrast: Black on white or white on black
                if original.relative_luminance() > 0.5 {
                    Color::BLACK
                } else {
                    Color::WHITE
                }
            }
            ContrastMode::Colorblind => {
                // Colorblind-friendly palette
                self.map_to_colorblind_safe(original)
            }
            ContrastMode::LowVision => {
                // Enhanced contrast while preserving some color
                self.enhance_contrast(original)
            }
            ContrastMode::Pattern => {
                // Keep original color but add patterns
                *original
            }
            ContrastMode::Standard => {
                // No replacement
                *original
            }
            ContrastMode::Custom(theme) => {
                // Use custom theme
                self.apply_custom_theme(original, theme)
            }
        }
    }

    /// Map color to colorblind-safe alternative.
    fn map_to_colorblind_safe(&self, color: &Color) -> Color {
        // Use a colorblind-friendly palette
        // Based on Paul Tol's palette: https://personal.sron.nl/~pault/
        let luminance = color.relative_luminance();

        if luminance < 0.2 {
            Color::from_rgb(51, 34, 136) // Dark blue
        } else if luminance < 0.4 {
            Color::from_rgb(136, 204, 238) // Light blue
        } else if luminance < 0.6 {
            Color::from_rgb(221, 204, 119) // Light yellow
        } else if luminance < 0.8 {
            Color::from_rgb(204, 102, 119) // Pink
        } else {
            Color::from_rgb(17, 119, 51) // Green
        }
    }

    /// Enhance contrast of a color.
    fn enhance_contrast(&self, color: &Color) -> Color {
        // Increase saturation and adjust brightness
        let luminance = color.relative_luminance();
        let factor = if luminance > 0.5 { 1.3 } else { 0.7 };

        Color::new(
            (color.r * factor).min(1.0),
            (color.g * factor).min(1.0),
            (color.b * factor).min(1.0),
            color.a,
        )
    }

    /// Apply custom theme to a color.
    fn apply_custom_theme(&self, color: &Color, theme: &ContrastTheme) -> Color {
        // Simple mapping based on luminance
        let luminance = color.relative_luminance();

        if luminance < 0.33 {
            theme.dark_color
        } else if luminance < 0.66 {
            theme.mid_color
        } else {
            theme.light_color
        }
    }

    /// Initialize color replacements for a mode.
    fn initialize_color_replacements(&mut self, mode: &ContrastMode) {
        self.color_replacements.clear();

        match mode {
            ContrastMode::HighContrast => {
                // Define standard high contrast replacements
                self.color_replacements
                    .insert("background".to_string(), Color::WHITE);
                self.color_replacements
                    .insert("foreground".to_string(), Color::BLACK);
                self.color_replacements
                    .insert("highlight".to_string(), Color::RED);
            }
            ContrastMode::Colorblind => {
                // Colorblind-friendly palette
                self.color_replacements
                    .insert("primary".to_string(), Color::from_rgb(51, 34, 136));
                self.color_replacements
                    .insert("secondary".to_string(), Color::from_rgb(136, 204, 238));
            }
            _ => {
                // Other modes don't need specific replacements
            }
        }
    }

    /// Get accessibility overrides for rendering.
    pub fn get_accessibility_overrides(&self) -> AccessibilityOverrides {
        match &self.contrast_mode {
            ContrastMode::HighContrast => AccessibilityOverrides {
                background: Color::WHITE,
                foreground: Color::BLACK,
                highlight: Color::BLACK,
                focus: Color::RED,
            },
            ContrastMode::LowVision => AccessibilityOverrides {
                background: Color::WHITE,
                foreground: Color::BLACK,
                highlight: Color::from_rgb(0, 0, 139),
                focus: Color::from_rgb(255, 69, 0),
            },
            _ => AccessibilityOverrides::default(),
        }
    }
}

/// Contrast modes for visual accessibility.
#[derive(Debug, Clone, PartialEq)]
pub enum ContrastMode {
    /// Standard rendering (no accessibility adjustments)
    Standard,

    /// High contrast black/white rendering
    HighContrast,

    /// Enhanced colors for low vision
    LowVision,

    /// Colorblind-friendly palette
    Colorblind,

    /// Pattern-based rendering instead of color
    Pattern,

    /// Custom contrast theme
    Custom(ContrastTheme),
}

/// Custom contrast theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastTheme {
    /// Color for dark elements
    pub dark_color: Color,

    /// Color for mid-tone elements
    pub mid_color: Color,

    /// Color for light elements
    pub light_color: Color,

    /// Background color
    pub background: Color,
}

impl Default for ContrastTheme {
    fn default() -> Self {
        Self {
            dark_color: Color::BLACK,
            mid_color: Color::new(0.5, 0.5, 0.5, 1.0),
            light_color: Color::WHITE,
            background: Color::WHITE,
        }
    }
}

/// Accessibility color overrides for rendering.
#[derive(Debug, Clone, Copy)]
pub struct AccessibilityOverrides {
    /// Background color
    pub background: Color,

    /// Foreground color
    pub foreground: Color,

    /// Highlight color
    pub highlight: Color,

    /// Focus indicator color
    pub focus: Color,
}

impl Default for AccessibilityOverrides {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            foreground: Color::BLACK,
            highlight: Color::BLUE,
            focus: Color::RED,
        }
    }
}

/// Pattern library for pattern-based rendering.
#[derive(Debug)]
pub struct PatternLibrary {
    patterns: HashMap<String, Pattern>,
}

impl PatternLibrary {
    /// Create a new pattern library.
    pub fn new() -> Self {
        let mut library = Self {
            patterns: HashMap::new(),
        };

        // Initialize standard patterns
        library.patterns.insert("solid".to_string(), Pattern::Solid);
        library
            .patterns
            .insert("dots".to_string(), Pattern::Dots { spacing: 4.0 });
        library.patterns.insert(
            "lines".to_string(),
            Pattern::Lines {
                spacing: 4.0,
                angle: 0.0,
            },
        );
        library.patterns.insert(
            "crosshatch".to_string(),
            Pattern::Crosshatch { spacing: 4.0 },
        );

        library
    }

    /// Get a pattern by name.
    pub fn get_pattern(&self, name: &str) -> Option<&Pattern> {
        self.patterns.get(name)
    }

    /// Add a custom pattern.
    pub fn add_pattern(&mut self, name: String, pattern: Pattern) {
        self.patterns.insert(name, pattern);
    }
}

impl Default for PatternLibrary {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern definition for alternative rendering.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Solid fill
    Solid,

    /// Dot pattern
    Dots { spacing: f32 },

    /// Line pattern
    Lines { spacing: f32, angle: f32 },

    /// Crosshatch pattern
    Crosshatch { spacing: f32 },
}

/// Calculate WCAG contrast ratio between two colors.
pub fn calculate_contrast_ratio(color1: Color, color2: Color) -> f32 {
    let l1 = color1.relative_luminance();
    let l2 = color2.relative_luminance();

    let lighter = l1.max(l2);
    let darker = l1.min(l2);

    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_creation() {
        let color = Color::new(0.5, 0.5, 0.5, 1.0);
        assert_eq!(color.r, 0.5);
        assert_eq!(color.g, 0.5);
        assert_eq!(color.b, 0.5);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_color_from_rgb() {
        let color = Color::from_rgb(128, 128, 128);
        assert!((color.r - 128.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::WHITE.r, 1.0);
        assert_eq!(Color::BLACK.r, 0.0);
        assert_eq!(Color::RED.g, 0.0);
    }

    #[test]
    fn test_high_contrast_renderer() {
        let renderer = HighContrastRenderer::new(ContrastMode::HighContrast);
        assert!(matches!(
            renderer.contrast_mode(),
            ContrastMode::HighContrast
        ));
    }

    #[test]
    fn test_contrast_mode_change() {
        let mut renderer = HighContrastRenderer::new(ContrastMode::Standard);
        assert!(matches!(renderer.contrast_mode(), ContrastMode::Standard));

        renderer.set_contrast_mode(ContrastMode::Colorblind);
        assert!(matches!(renderer.contrast_mode(), ContrastMode::Colorblind));
    }

    #[test]
    fn test_color_replacement() {
        let renderer = HighContrastRenderer::new(ContrastMode::HighContrast);

        let light_color = Color::new(0.8, 0.8, 0.8, 1.0);
        let replacement = renderer.get_color_replacement(&light_color);

        // Light colors should be replaced with black in high contrast
        assert_eq!(replacement, Color::BLACK);
    }

    #[test]
    fn test_colorblind_mapping() {
        let renderer = HighContrastRenderer::new(ContrastMode::Colorblind);

        let color = Color::new(0.5, 0.5, 0.5, 1.0);
        let replacement = renderer.get_color_replacement(&color);

        // Should be mapped to a colorblind-safe color
        assert_ne!(replacement, color);
    }

    #[test]
    fn test_contrast_ratio() {
        let white = Color::WHITE;
        let black = Color::BLACK;

        let ratio = calculate_contrast_ratio(white, black);

        // White on black should have maximum contrast (21:1)
        assert!(ratio > 20.0);
        assert!(ratio <= 21.0);
    }

    #[test]
    fn test_pattern_library() {
        let library = PatternLibrary::new();

        assert!(library.get_pattern("solid").is_some());
        assert!(library.get_pattern("dots").is_some());
        assert!(library.get_pattern("lines").is_some());
        assert!(library.get_pattern("nonexistent").is_none());
    }

    #[test]
    fn test_custom_pattern() {
        let mut library = PatternLibrary::new();

        library.add_pattern(
            "custom".to_string(),
            Pattern::Lines {
                spacing: 8.0,
                angle: 45.0,
            },
        );

        assert!(library.get_pattern("custom").is_some());
    }

    #[test]
    fn test_accessibility_overrides() {
        let overrides = AccessibilityOverrides::default();

        assert_eq!(overrides.background, Color::WHITE);
        assert_eq!(overrides.foreground, Color::BLACK);
    }

    #[test]
    fn test_relative_luminance() {
        let white = Color::WHITE;
        let black = Color::BLACK;

        assert!(white.relative_luminance() > 0.9);
        assert!(black.relative_luminance() < 0.1);
    }

    #[test]
    fn test_wcag_aa_compliance() {
        // WCAG AA requires 4.5:1 for normal text
        let white = Color::WHITE;
        let dark_gray = Color::new(0.18, 0.18, 0.18, 1.0);

        let ratio = calculate_contrast_ratio(white, dark_gray);
        assert!(
            ratio >= 4.5,
            "Contrast ratio {} does not meet WCAG AA",
            ratio
        );
    }
}
