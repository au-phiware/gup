// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for accessibility features.
//!
//! Tests the integration of accessibility features with core Gup components.

use gup::accessibility::{
    AccessibilitySystem, AriaRole, ContrastMode, FocusManager, FocusableElement, KeyEvent,
};
use gup::interaction::{Rect, Vec2};

#[test]
fn test_accessibility_system_integration() {
    let mut system = AccessibilitySystem::new();

    // Verify default state
    assert!(system.is_enabled());
    assert!(system.is_keyboard_navigation_enabled());
    assert!(system.is_screen_reader_enabled());

    // Test configuration changes
    system.set_contrast_mode(ContrastMode::HighContrast);
    assert!(matches!(system.contrast_mode(), ContrastMode::HighContrast));
}

#[test]
fn test_aria_chart_description() {
    let mut system = AccessibilitySystem::new();

    // Create a chart node
    let chart_id = system.aria_tree.create_chart_node(
        "Sales Chart".to_string(),
        Some("Quarterly sales data for 2024".to_string()),
    );

    // Verify the node was created
    let node = system.aria_tree.get_node(chart_id).unwrap();
    assert_eq!(node.role, AriaRole::Chart);
    assert_eq!(node.label, "Sales Chart");
    assert_eq!(
        node.description,
        Some("Quarterly sales data for 2024".to_string())
    );
}

#[test]
fn test_keyboard_focus_navigation() {
    let mut focus_manager = FocusManager::new();

    // Create test elements in a grid pattern
    let elements = vec![
        FocusableElement::new(
            gup::accessibility::ElementType::DataPoint,
            Rect::from_center_size(Vec2::new(100.0, 100.0), Vec2::new(10.0, 10.0)),
            "Point A".to_string(),
        ),
        FocusableElement::new(
            gup::accessibility::ElementType::DataPoint,
            Rect::from_center_size(Vec2::new(200.0, 100.0), Vec2::new(10.0, 10.0)),
            "Point B".to_string(),
        ),
        FocusableElement::new(
            gup::accessibility::ElementType::DataPoint,
            Rect::from_center_size(Vec2::new(100.0, 200.0), Vec2::new(10.0, 10.0)),
            "Point C".to_string(),
        ),
    ];

    focus_manager.add_focusable_elements(elements);

    // Test sequential navigation
    focus_manager.handle_key_input(KeyEvent::Tab);
    assert!(focus_manager.describe_current_focus().is_some());

    focus_manager.handle_key_input(KeyEvent::Tab);
    let desc = focus_manager.describe_current_focus().unwrap();
    assert!(desc.contains("Point B"));
}

#[test]
fn test_high_contrast_mode() {
    use gup::{HighContrastRenderer, calculate_contrast_ratio};

    let renderer = HighContrastRenderer::new(ContrastMode::HighContrast);

    // Verify contrast ratios meet WCAG AA
    let overrides = renderer.get_accessibility_overrides();
    let ratio = calculate_contrast_ratio(overrides.background, overrides.foreground);

    assert!(ratio >= 4.5, "High contrast mode should meet WCAG AA");
}

#[test]
fn test_colorblind_rendering() {
    use gup::{AccessibilityColor, HighContrastRenderer};

    let renderer = HighContrastRenderer::new(ContrastMode::Colorblind);

    // Test that colorblind mode maps colors correctly
    let test_color = AccessibilityColor::new(0.5, 0.5, 0.5, 1.0);
    let mapped = renderer.get_color_replacement(&test_color);

    // Should be different from original
    assert_ne!(mapped, test_color);
}

#[test]
fn test_screen_reader_updates() {
    let mut system = AccessibilitySystem::new();

    // Create a chart
    let _chart_id = system.aria_tree.create_chart_node(
        "Data Visualization".to_string(),
        Some("Interactive chart with 100 points".to_string()),
    );

    // Update a live region
    system
        .aria_tree
        .update_live_region("status", "Data updated with new values");

    // Get pending updates
    let updates = system.get_pending_aria_updates();
    assert!(!updates.is_empty());

    // Verify update content
    let has_live_update = updates.iter().any(|u| {
        matches!(
            u,
            gup::accessibility::AriaUpdate::LiveRegion { content, .. }
                if content.contains("Data updated")
        )
    });
    assert!(has_live_update);
}

#[test]
fn test_sonification_data_narration() {
    use gup::SonificationEngine;

    let engine = SonificationEngine::new();

    // Test narration for different dataset sizes
    let small_data: Vec<i32> = vec![1, 2, 3];
    let narration = engine.create_data_narration(&small_data);
    assert!(narration.contains("3 data points"));

    let large_data: Vec<i32> = vec![0; 1000];
    let narration = engine.create_data_narration(&large_data);
    assert!(narration.contains("1000 data points"));
    assert!(narration.contains("Large"));
}

#[test]
fn test_accessibility_with_empty_data() {
    let system = AccessibilitySystem::new();

    // Test with empty dataset
    let empty_data: Vec<f32> = vec![];
    let pattern = system.aria_tree.analyze_data_patterns(&empty_data);
    assert_eq!(pattern, "Empty dataset");
}

#[test]
fn test_focus_history() {
    let mut focus_manager = FocusManager::new();

    let elements = vec![
        FocusableElement::new(
            gup::accessibility::ElementType::Control,
            Rect::from_center_size(Vec2::new(50.0, 50.0), Vec2::new(10.0, 10.0)),
            "Button 1".to_string(),
        ),
        FocusableElement::new(
            gup::accessibility::ElementType::Control,
            Rect::from_center_size(Vec2::new(150.0, 50.0), Vec2::new(10.0, 10.0)),
            "Button 2".to_string(),
        ),
    ];

    focus_manager.add_focusable_elements(elements);

    // Navigate forward
    focus_manager.handle_key_input(KeyEvent::Tab);
    focus_manager.handle_key_input(KeyEvent::Tab);

    // Escape should go back
    focus_manager.handle_key_input(KeyEvent::Escape);

    let desc = focus_manager.describe_current_focus().unwrap();
    assert!(desc.contains("Button 1"));
}

#[test]
fn test_wcag_compliance_standards() {
    use gup::{AccessibilityColor, calculate_contrast_ratio};

    // Test various color combinations for WCAG compliance
    let white = AccessibilityColor::WHITE;
    let black = AccessibilityColor::BLACK;

    // Black on white should have maximum contrast
    let ratio = calculate_contrast_ratio(white, black);
    assert!(
        ratio > 20.0,
        "Black on white contrast should be near maximum (got {})",
        ratio
    );

    // Test WCAG AA minimum (4.5:1)
    let dark_gray = AccessibilityColor::new(0.18, 0.18, 0.18, 1.0);
    let ratio_aa = calculate_contrast_ratio(white, dark_gray);
    assert!(ratio_aa >= 4.5);

    // Test WCAG AAA minimum (7:1)
    let darker_gray = AccessibilityColor::new(0.05, 0.05, 0.05, 1.0);
    let ratio_aaa = calculate_contrast_ratio(white, darker_gray);
    assert!(
        ratio_aaa >= 7.0,
        "WCAG AAA ratio {} does not meet 7:1",
        ratio_aaa
    );
}

#[test]
fn test_multiple_contrast_modes() {
    use gup::HighContrastRenderer;

    let modes = vec![
        ContrastMode::Standard,
        ContrastMode::HighContrast,
        ContrastMode::LowVision,
        ContrastMode::Colorblind,
        ContrastMode::Pattern,
    ];

    for mode in modes {
        let renderer = HighContrastRenderer::new(mode.clone());
        assert_eq!(renderer.contrast_mode(), &mode);
    }
}

#[test]
fn test_sonification_mapping() {
    use gup::accessibility::{AudioParameter, MappingFunction, SonificationEngine};

    let mut engine = SonificationEngine::new();

    // Add a mapping
    engine.add_mapping(
        "temperature".to_string(),
        AudioParameter::Pitch,
        MappingFunction::Linear,
    );

    assert_eq!(engine.mappings().len(), 1);
    assert!(engine.mappings().contains_key("temperature"));
}

#[test]
fn test_accessibility_disabled_mode() {
    let mut system = AccessibilitySystem::new();

    system.set_enabled(false);
    assert!(!system.is_enabled());

    // When disabled, updates should return empty
    let updates = system.get_pending_aria_updates();
    assert!(updates.is_empty());

    // Description should return None
    let desc = system.describe_current_focus();
    assert!(desc.is_none());
}
