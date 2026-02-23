// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for pattern-based rendering functionality.

use gup::accessibility::{Color, Pattern, PatternUniforms};

#[test]
fn test_pattern_type_ids() {
    let solid = Pattern::Solid;
    let dots = Pattern::Dots { spacing: 8.0 };
    let lines = Pattern::Lines {
        spacing: 6.0,
        angle: 0.0,
    };
    let crosshatch = Pattern::Crosshatch { spacing: 8.0 };

    assert_eq!(solid.pattern_type_id(), 0);
    assert_eq!(dots.pattern_type_id(), 1);
    assert_eq!(lines.pattern_type_id(), 2);
    assert_eq!(crosshatch.pattern_type_id(), 3);
}

#[test]
fn test_pattern_spacing() {
    let dots = Pattern::Dots { spacing: 10.0 };
    let lines = Pattern::Lines {
        spacing: 5.0,
        angle: 1.0,
    };
    let crosshatch = Pattern::Crosshatch { spacing: 7.0 };

    assert_eq!(dots.spacing(), 10.0);
    assert_eq!(lines.spacing(), 5.0);
    assert_eq!(crosshatch.spacing(), 7.0);
}

#[test]
fn test_pattern_angle() {
    let angle = std::f32::consts::PI / 3.0;
    let lines = Pattern::Lines {
        spacing: 6.0,
        angle,
    };

    assert_eq!(lines.angle(), angle);

    // Other patterns should return 0.0
    let dots = Pattern::Dots { spacing: 8.0 };
    assert_eq!(dots.angle(), 0.0);
}

#[test]
fn test_pattern_thickness() {
    let lines = Pattern::Lines {
        spacing: 10.0,
        angle: 0.0,
    };
    let crosshatch = Pattern::Crosshatch { spacing: 8.0 };

    // Thickness should be 20% of spacing for line patterns
    assert_eq!(lines.thickness(), 2.0);
    assert_eq!(crosshatch.thickness(), 1.6);
}

#[test]
fn test_pattern_uniforms_solid() {
    let color = Color::RED;
    let uniforms = PatternUniforms::solid(color);

    assert_eq!(uniforms.pattern_type, 0);
    assert_eq!(uniforms.foreground_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(uniforms.background_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(uniforms.spacing, 0.0);
    assert_eq!(uniforms.angle, 0.0);
}

#[test]
fn test_pattern_uniforms_dots() {
    let pattern = Pattern::Dots { spacing: 8.0 };
    let fg = Color::BLACK;
    let bg = Color::WHITE;

    let uniforms = PatternUniforms::from_pattern(&pattern, fg, bg);

    assert_eq!(uniforms.pattern_type, 1);
    assert_eq!(uniforms.spacing, 8.0);
    assert_eq!(uniforms.foreground_color, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(uniforms.background_color, [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn test_pattern_uniforms_lines() {
    let angle = std::f32::consts::PI / 4.0;
    let pattern = Pattern::Lines {
        spacing: 6.0,
        angle,
    };
    let fg = Color::BLUE;
    let bg = Color::YELLOW;

    let uniforms = PatternUniforms::from_pattern(&pattern, fg, bg);

    assert_eq!(uniforms.pattern_type, 2);
    assert_eq!(uniforms.spacing, 6.0);
    assert_eq!(uniforms.angle, angle);
    assert_eq!(uniforms.foreground_color, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(uniforms.background_color, [1.0, 1.0, 0.0, 1.0]);
    assert_eq!(uniforms.thickness, 1.2); // 20% of 6.0
}

#[test]
fn test_pattern_uniforms_crosshatch() {
    let pattern = Pattern::Crosshatch { spacing: 10.0 };
    let fg = Color::GREEN;
    let bg = Color::WHITE;

    let uniforms = PatternUniforms::from_pattern(&pattern, fg, bg);

    assert_eq!(uniforms.pattern_type, 3);
    assert_eq!(uniforms.spacing, 10.0);
    assert_eq!(uniforms.foreground_color, [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(uniforms.background_color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(uniforms.thickness, 2.0); // 20% of 10.0
}

#[test]
fn test_pattern_uniforms_alignment() {
    // Verify that PatternUniforms has the correct size for GPU alignment
    // Must be a multiple of 16 bytes for uniform buffers
    let size = std::mem::size_of::<PatternUniforms>();
    assert_eq!(size, 64, "PatternUniforms must be 64 bytes for alignment");
    assert_eq!(size % 16, 0, "Size must be multiple of 16 bytes");
}

#[test]
fn test_pattern_uniforms_pod() {
    // Verify PatternUniforms can be used with bytemuck
    let pattern = Pattern::Dots { spacing: 8.0 };
    let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

    // Should be able to convert to bytes
    let bytes: &[u8] = bytemuck::bytes_of(&uniforms);
    assert_eq!(bytes.len(), std::mem::size_of::<PatternUniforms>());

    // Should be able to convert back
    let decoded: &PatternUniforms = bytemuck::from_bytes(bytes);
    assert_eq!(decoded.pattern_type, uniforms.pattern_type);
    assert_eq!(decoded.spacing, uniforms.spacing);
}

#[test]
fn test_different_angles() {
    // Test various angles
    let angles = [
        0.0,                              // Horizontal
        std::f32::consts::PI / 4.0,       // 45 degrees
        std::f32::consts::PI / 2.0,       // Vertical
        std::f32::consts::PI,             // 180 degrees
        std::f32::consts::PI * 3.0 / 4.0, // 135 degrees
    ];

    for angle in angles {
        let pattern = Pattern::Lines {
            spacing: 8.0,
            angle,
        };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        assert_eq!(uniforms.angle, angle);
        assert_eq!(uniforms.pattern_type, 2);
    }
}

#[test]
fn test_different_spacings() {
    // Test various spacings
    let spacings = [2.0, 4.0, 8.0, 12.0, 16.0, 32.0];

    for spacing in spacings {
        let pattern = Pattern::Dots { spacing };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        assert_eq!(uniforms.spacing, spacing);
        assert_eq!(uniforms.pattern_type, 1);
    }
}

#[test]
fn test_color_conversion() {
    // Test that colors are correctly converted to RGBA arrays
    let colors = [
        (Color::BLACK, [0.0, 0.0, 0.0, 1.0]),
        (Color::WHITE, [1.0, 1.0, 1.0, 1.0]),
        (Color::RED, [1.0, 0.0, 0.0, 1.0]),
        (Color::GREEN, [0.0, 1.0, 0.0, 1.0]),
        (Color::BLUE, [0.0, 0.0, 1.0, 1.0]),
        (Color::YELLOW, [1.0, 1.0, 0.0, 1.0]),
    ];

    for (color, expected) in colors {
        let uniforms = PatternUniforms::solid(color);
        assert_eq!(uniforms.foreground_color, expected);
    }
}

#[test]
fn test_pattern_clone() {
    // Verify patterns can be cloned
    let pattern1 = Pattern::Lines {
        spacing: 6.0,
        angle: 1.0,
    };
    let pattern2 = pattern1.clone();

    assert_eq!(pattern1.pattern_type_id(), pattern2.pattern_type_id());
    assert_eq!(pattern1.spacing(), pattern2.spacing());
    assert_eq!(pattern1.angle(), pattern2.angle());
}

#[test]
fn test_pattern_debug() {
    // Verify patterns have Debug trait
    let pattern = Pattern::Dots { spacing: 8.0 };
    let debug_str = format!("{:?}", pattern);
    assert!(debug_str.contains("Dots"));
    assert!(debug_str.contains("8.0"));
}

#[test]
fn test_all_pattern_variants() {
    // Test that we can create all pattern types
    let patterns = [Pattern::Solid,
        Pattern::Dots { spacing: 8.0 },
        Pattern::Lines {
            spacing: 6.0,
            angle: 0.0,
        },
        Pattern::Crosshatch { spacing: 8.0 }];

    for (i, pattern) in patterns.iter().enumerate() {
        assert_eq!(pattern.pattern_type_id(), i as u32);
    }
}
