// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! # Pattern Rendering Demo
//!
//! Demonstrates pattern-based rendering for accessibility.
//! Patterns provide a color-independent way to distinguish between
//! data categories, essential for colorblind users.
//!
//! ## What You'll Learn
//! - How to use patterns (dots, lines, crosshatch) for data encoding
//! - How to configure pattern parameters (spacing, angle)
//! - How patterns work with the accessibility system
//! - How to switch between pattern and color modes
//!
//! Run with: `cargo run --example pattern_rendering_demo`

use gup::accessibility::{AccessibilitySystem, Color, ContrastMode, Pattern, PatternUniforms};

fn main() {
    println!("Pattern Rendering Demo");
    println!("======================\n");

    // Create accessibility system
    let mut accessibility = AccessibilitySystem::new();

    // Demonstrate different pattern types
    demonstrate_patterns(&accessibility);

    // Switch to pattern mode
    accessibility.set_contrast_mode(ContrastMode::Pattern);
    println!("\nSwitched to Pattern mode for accessibility");
    println!("Current mode: {:?}\n", accessibility.contrast_mode());

    // Show how patterns are assigned to categories
    demonstrate_category_patterns(&accessibility);

    // Show pattern parameters
    demonstrate_pattern_parameters();

    println!("\n✓ Pattern rendering system ready!");
    println!("  Patterns can be used with any mark type");
    println!("  GPU-accelerated for high performance");
    println!("  Fully integrated with accessibility system");
}

fn demonstrate_patterns(accessibility: &AccessibilitySystem) {
    println!("Available Pattern Types:");
    println!("------------------------");

    let pattern_library = accessibility.high_contrast_renderer.pattern_library();

    // Solid pattern
    if let Some(pattern) = pattern_library.get_pattern("solid") {
        println!("• Solid: {:?}", pattern);
    }

    // Dots pattern
    if let Some(pattern) = pattern_library.get_pattern("dots") {
        println!("• Dots: {:?}", pattern);
    }

    // Lines pattern
    if let Some(pattern) = pattern_library.get_pattern("lines") {
        println!("• Lines: {:?}", pattern);
    }

    // Crosshatch pattern
    if let Some(pattern) = pattern_library.get_pattern("crosshatch") {
        println!("• Crosshatch: {:?}", pattern);
    }
}

fn demonstrate_category_patterns(accessibility: &AccessibilitySystem) {
    println!("Pattern Assignment for Categories:");
    println!("----------------------------------");

    for i in 0..6 {
        let pattern = accessibility
            .high_contrast_renderer
            .get_pattern_for_category(i);
        println!("  Category {}: {:?}", i, pattern);
    }
}

fn demonstrate_pattern_parameters() {
    println!("\nPattern Parameters:");
    println!("------------------");

    // Create different pattern configurations
    let patterns = vec![
        (
            "Small Dots",
            Pattern::Dots { spacing: 4.0 },
            Color::BLACK,
            Color::WHITE,
        ),
        (
            "Large Dots",
            Pattern::Dots { spacing: 12.0 },
            Color::BLACK,
            Color::WHITE,
        ),
        (
            "Horizontal Lines",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
            Color::BLACK,
            Color::WHITE,
        ),
        (
            "Diagonal Lines (45°)",
            Pattern::Lines {
                spacing: 6.0,
                angle: std::f32::consts::PI / 4.0,
            },
            Color::BLACK,
            Color::WHITE,
        ),
        (
            "Fine Crosshatch",
            Pattern::Crosshatch { spacing: 6.0 },
            Color::BLACK,
            Color::WHITE,
        ),
        (
            "Wide Crosshatch",
            Pattern::Crosshatch { spacing: 12.0 },
            Color::BLACK,
            Color::WHITE,
        ),
    ];

    for (name, pattern, fg, bg) in patterns {
        let uniforms = PatternUniforms::from_pattern(&pattern, fg, bg);
        println!(
            "  {}: type={}, spacing={:.1}px, angle={:.2}rad",
            name, uniforms.pattern_type, uniforms.spacing, uniforms.angle
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_library_has_standard_patterns() {
        let accessibility = AccessibilitySystem::new();
        let library = accessibility.high_contrast_renderer.pattern_library();

        assert!(library.get_pattern("solid").is_some());
        assert!(library.get_pattern("dots").is_some());
        assert!(library.get_pattern("lines").is_some());
        assert!(library.get_pattern("crosshatch").is_some());
    }

    #[test]
    fn test_pattern_mode_switch() {
        let mut accessibility = AccessibilitySystem::new();

        // Start in standard mode
        assert!(matches!(
            accessibility.contrast_mode(),
            ContrastMode::Standard
        ));

        // Switch to pattern mode
        accessibility.set_contrast_mode(ContrastMode::Pattern);
        assert!(matches!(
            accessibility.contrast_mode(),
            ContrastMode::Pattern
        ));
    }

    #[test]
    fn test_category_pattern_assignment() {
        let accessibility = AccessibilitySystem::new();

        // Get patterns for first 4 categories
        let patterns: Vec<_> = (0..4)
            .map(|i| {
                accessibility
                    .high_contrast_renderer
                    .get_pattern_for_category(i)
            })
            .collect();

        // Verify they cycle through the pattern types
        assert!(matches!(patterns[0], Pattern::Solid));
        assert!(matches!(patterns[1], Pattern::Dots { .. }));
        assert!(matches!(patterns[2], Pattern::Lines { .. }));
        assert!(matches!(patterns[3], Pattern::Crosshatch { .. }));
    }

    #[test]
    fn test_pattern_uniforms_creation() {
        let pattern = Pattern::Dots { spacing: 8.0 };
        let fg = Color::BLACK;
        let bg = Color::WHITE;

        let uniforms = PatternUniforms::from_pattern(&pattern, fg, bg);

        assert_eq!(uniforms.pattern_type, 1); // Dots = 1
        assert_eq!(uniforms.spacing, 8.0);
        assert_eq!(uniforms.foreground_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(uniforms.background_color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_line_pattern_angle() {
        let angle = std::f32::consts::PI / 4.0; // 45 degrees
        let pattern = Pattern::Lines {
            spacing: 6.0,
            angle,
        };

        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        assert_eq!(uniforms.pattern_type, 2); // Lines = 2
        assert_eq!(uniforms.angle, angle);
    }
}
