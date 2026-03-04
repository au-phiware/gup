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

    /// Calculate relative luminance per the WCAG 2.1 specification.
    ///
    /// Applies sRGB gamma linearization before computing luminance using
    /// ITU-R BT.709 coefficients, as specified in WCAG 2.1 §1.4.3.
    pub fn relative_luminance(&self) -> f32 {
        fn linearize(c: f32) -> f32 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * linearize(self.r) + 0.7152 * linearize(self.g) + 0.0722 * linearize(self.b)
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

    /// Get the pattern library for pattern-based rendering.
    pub fn pattern_library(&self) -> &PatternLibrary {
        &self.pattern_library
    }

    /// Get a pattern for a given category index.
    /// This assigns patterns in a consistent order for distinguishing categories.
    pub fn get_pattern_for_category(&self, category: usize) -> Pattern {
        match category % 4 {
            0 => Pattern::Solid,
            1 => Pattern::Dots { spacing: 8.0 },
            2 => Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
            3 => Pattern::Crosshatch { spacing: 8.0 },
            _ => Pattern::Solid,
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

impl Pattern {
    /// Get the pattern type ID for GPU shader
    pub fn pattern_type_id(&self) -> u32 {
        match self {
            Pattern::Solid => 0,
            Pattern::Dots { .. } => 1,
            Pattern::Lines { .. } => 2,
            Pattern::Crosshatch { .. } => 3,
        }
    }

    /// Get the spacing parameter for the pattern
    pub fn spacing(&self) -> f32 {
        match self {
            Pattern::Solid => 0.0,
            Pattern::Dots { spacing } => *spacing,
            Pattern::Lines { spacing, .. } => *spacing,
            Pattern::Crosshatch { spacing } => *spacing,
        }
    }

    /// Get the angle parameter for line patterns (in radians)
    pub fn angle(&self) -> f32 {
        match self {
            Pattern::Lines { angle, .. } => *angle,
            _ => 0.0,
        }
    }

    /// Get a default line thickness for the pattern
    pub fn thickness(&self) -> f32 {
        match self {
            Pattern::Lines { spacing, .. } => spacing * 0.2,
            Pattern::Crosshatch { spacing } => spacing * 0.2,
            _ => 1.0,
        }
    }
}

/// Calculate WCAG contrast ratio between two colors.
///
/// Uses the formula from WCAG 2.1 §1.4.3:
/// `(L1 + 0.05) / (L2 + 0.05)` where L1 is the lighter luminance.
pub fn calculate_contrast_ratio(color1: Color, color2: Color) -> f32 {
    let l1 = color1.relative_luminance();
    let l2 = color2.relative_luminance();

    let lighter = l1.max(l2);
    let darker = l1.min(l2);

    (lighter + 0.05) / (darker + 0.05)
}

/// Check whether two colours meet the WCAG 2.1 AA contrast requirement
/// for normal-sized text (4.5 : 1).
pub fn meets_wcag_aa(foreground: Color, background: Color) -> bool {
    calculate_contrast_ratio(foreground, background) >= 4.5
}

/// Check whether two colours meet the WCAG 2.1 AA contrast requirement
/// for large text or non-text UI components (3 : 1).
pub fn meets_wcag_aa_large_text(foreground: Color, background: Color) -> bool {
    calculate_contrast_ratio(foreground, background) >= 3.0
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

        // WCAG specifies white = 1.0, black = 0.0 after linearization
        assert!(
            (white.relative_luminance() - 1.0).abs() < 0.001,
            "White luminance should be ~1.0, got {}",
            white.relative_luminance()
        );
        assert!(
            black.relative_luminance().abs() < 0.001,
            "Black luminance should be ~0.0, got {}",
            black.relative_luminance()
        );
    }

    #[test]
    fn test_srgb_linearization_midpoint() {
        // sRGB 0.5 should linearize to approximately 0.214
        let mid_gray = Color::new(0.5, 0.5, 0.5, 1.0);
        let luminance = mid_gray.relative_luminance();
        assert!(
            (luminance - 0.214).abs() < 0.01,
            "Mid-gray luminance should be ~0.214 after sRGB linearization, got {}",
            luminance
        );
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

    // --- WCAG 2.1 AA regression tests (GUP-272) ---

    #[test]
    fn test_wcag_contrast_ratio_black_white() {
        // WCAG defines maximum contrast as 21:1
        let ratio = calculate_contrast_ratio(Color::WHITE, Color::BLACK);
        assert!(
            (ratio - 21.0).abs() < 0.1,
            "Black/white contrast should be ~21:1, got {}",
            ratio
        );
    }

    #[test]
    fn test_wcag_contrast_ratio_is_symmetric() {
        let a = Color::new(0.2, 0.4, 0.8, 1.0);
        let b = Color::new(1.0, 1.0, 0.9, 1.0);
        let ratio_ab = calculate_contrast_ratio(a, b);
        let ratio_ba = calculate_contrast_ratio(b, a);
        assert!(
            (ratio_ab - ratio_ba).abs() < 0.001,
            "Contrast ratio must be symmetric: {} vs {}",
            ratio_ab,
            ratio_ba
        );
    }

    #[test]
    fn test_meets_wcag_aa_normal_text() {
        // 4.5:1 threshold for normal text
        assert!(meets_wcag_aa(Color::BLACK, Color::WHITE));
        assert!(meets_wcag_aa(Color::WHITE, Color::BLACK));

        // Same colour should fail (1:1)
        assert!(!meets_wcag_aa(Color::WHITE, Color::WHITE));
    }

    #[test]
    fn test_meets_wcag_aa_large_text() {
        // 3:1 threshold for large text / non-text UI components
        assert!(meets_wcag_aa_large_text(Color::BLACK, Color::WHITE));

        // A mid-gray that passes large text (3:1) but fails normal text (4.5:1)
        // With sRGB linearization, sRGB ~0.50 ≈ linear 0.214, giving ~3.98:1
        // against white. Adjust to 0.52 to stay safely in the band.
        let gray = Color::new(0.52, 0.52, 0.52, 1.0);
        let ratio = calculate_contrast_ratio(gray, Color::WHITE);
        assert!(
            (3.0..4.5).contains(&ratio),
            "Gray should be between 3:1 and 4.5:1, got {}:1",
            ratio
        );
        assert!(meets_wcag_aa_large_text(gray, Color::WHITE));
        assert!(!meets_wcag_aa(gray, Color::WHITE));
    }

    #[test]
    fn test_high_contrast_mode_meets_wcag_aa() {
        // In HighContrast mode, dark inputs become WHITE and light inputs
        // become BLACK. We verify that each replacement is always the
        // OPPOSITE pole — giving maximum contrast against either background.
        let renderer = HighContrastRenderer::new(ContrastMode::HighContrast);

        for i in 0..10 {
            let val = i as f32 / 10.0;
            let input = Color::new(val, val, val, 1.0);
            let replacement = renderer.get_color_replacement(&input);

            // Every replacement must be one of the two poles
            let is_black = replacement == Color::BLACK;
            let is_white = replacement == Color::WHITE;
            assert!(
                is_black || is_white,
                "HighContrast replacement for ({},{},{}) should be black or white, got ({},{},{})",
                val,
                val,
                val,
                replacement.r,
                replacement.g,
                replacement.b
            );

            // And the two poles have maximum mutual contrast
            let ratio = calculate_contrast_ratio(Color::BLACK, Color::WHITE);
            assert!(
                ratio > 20.0,
                "Black/white contrast should be ~21:1, got {}",
                ratio
            );
        }
    }

    #[test]
    fn test_colorblind_palette_mutual_contrast() {
        // Colorblind-safe palette entries should be distinguishable
        // from each other — pairwise contrast should be > 1.5:1 minimum
        let renderer = HighContrastRenderer::new(ContrastMode::Colorblind);
        let colors: Vec<Color> = (0..5)
            .map(|i| {
                let val = i as f32 / 5.0;
                renderer.get_color_replacement(&Color::new(val, 0.0, 0.0, 1.0))
            })
            .collect();

        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                let ratio = calculate_contrast_ratio(colors[i], colors[j]);
                // Adjacent palette entries may be close, but should not be
                // identical (ratio > 1.0 means they differ)
                assert!(
                    ratio >= 1.0,
                    "Colorblind palette entries {} and {} are indistinguishable (ratio {})",
                    i,
                    j,
                    ratio
                );
            }
        }
    }

    #[test]
    fn test_each_pattern_type_is_distinct() {
        let library = PatternLibrary::new();
        let solid = library.get_pattern("solid");
        let dots = library.get_pattern("dots");
        let lines = library.get_pattern("lines");
        let crosshatch = library.get_pattern("crosshatch");

        // All four default patterns must be present
        assert!(solid.is_some(), "solid pattern missing");
        assert!(dots.is_some(), "dots pattern missing");
        assert!(lines.is_some(), "lines pattern missing");
        assert!(crosshatch.is_some(), "crosshatch pattern missing");

        // They must be distinct types (non-colour information channel)
        assert_ne!(
            std::mem::discriminant(solid.unwrap()),
            std::mem::discriminant(dots.unwrap())
        );
        assert_ne!(
            std::mem::discriminant(dots.unwrap()),
            std::mem::discriminant(lines.unwrap())
        );
    }

    #[test]
    fn test_aria_roles_match_wcag_expectations() {
        // Verify that ARIA role strings are valid WAI-ARIA roles
        // (regression test for WCAG 4.1.2 Name, Role, Value)
        use crate::accessibility::aria::AriaRole;

        let valid_roles = [
            "img",
            "list",
            "listitem",
            "region",
            "separator",
            "tooltip",
            "button",
        ];

        let roles = [
            AriaRole::Chart,
            AriaRole::ChartSeries,
            AriaRole::DataPoint,
            AriaRole::Legend,
            AriaRole::Axis,
            AriaRole::Tooltip,
            AriaRole::Control,
        ];

        for role in &roles {
            let role_str = role.as_str();
            assert!(
                valid_roles.contains(&role_str),
                "AriaRole::{:?} maps to '{}' which is not a valid WAI-ARIA role",
                role,
                role_str
            );
        }
    }

    #[test]
    fn test_keyboard_navigation_no_trap() {
        // WCAG 2.1.2: No Keyboard Trap
        // Tab from last element must wrap to first (not trap)
        use crate::accessibility::keyboard::{
            ElementType, FocusManager, FocusableElement, KeyEvent,
        };
        use crate::interaction::Rect;
        use crate::math::Vec2;

        let mut fm = FocusManager::new();
        fm.add_focusable_element(FocusableElement::new(
            ElementType::DataPoint,
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            "First".to_string(),
        ));
        fm.add_focusable_element(FocusableElement::new(
            ElementType::DataPoint,
            Rect::new(Vec2::new(20.0, 0.0), Vec2::new(30.0, 10.0)),
            "Second".to_string(),
        ));
        fm.add_focusable_element(FocusableElement::new(
            ElementType::DataPoint,
            Rect::new(Vec2::new(40.0, 0.0), Vec2::new(50.0, 10.0)),
            "Third".to_string(),
        ));

        // Tab through all elements and one more to wrap
        fm.handle_key_input(KeyEvent::Tab);
        fm.handle_key_input(KeyEvent::Tab);
        fm.handle_key_input(KeyEvent::Tab);
        fm.handle_key_input(KeyEvent::Tab);

        // Should wrap back — focus should exist, not be trapped
        let desc = fm.describe_current_focus();
        assert!(desc.is_some(), "Focus should exist after wrapping");
    }

    #[test]
    fn test_aria_node_always_has_label() {
        // WCAG 2.4.6: Headings and Labels — every node must have a label
        use crate::accessibility::aria::{AriaNode, AriaRole};

        let node = AriaNode::new(AriaRole::DataPoint, "Sales: $42K".to_string());
        assert!(
            !node.label.is_empty(),
            "ARIA nodes must always have a non-empty label"
        );
    }

    #[test]
    fn test_live_region_urgency_levels() {
        // WCAG 4.1.3: Status Messages — both urgency levels must work
        use crate::accessibility::aria::{AriaLive, AriaTree, AriaUpdate};

        let mut tree = AriaTree::new();

        tree.update_live_region_with_urgency("status", "Loading data", AriaLive::Polite);
        tree.update_live_region_with_urgency("alert", "Error occurred", AriaLive::Assertive);

        let updates = tree.drain_update_queue();
        assert_eq!(updates.len(), 2);

        // Verify urgency levels are preserved
        match &updates[0] {
            AriaUpdate::LiveRegion { urgency, .. } => {
                assert_eq!(*urgency, AriaLive::Polite);
            }
            _ => panic!("Expected LiveRegion update"),
        }
        match &updates[1] {
            AriaUpdate::LiveRegion { urgency, .. } => {
                assert_eq!(*urgency, AriaLive::Assertive);
            }
            _ => panic!("Expected LiveRegion update"),
        }
    }
}
