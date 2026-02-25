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

// --- Selection focus integration tests ---

#[test]
fn test_selection_focus_bridge_with_circle_selection() {
    use gup::accessibility::selection_focus::{
        DataDimension, FocusPointDescriptor, SelectionFocusBridge,
    };
    use gup::mark::Circle;
    use gup::mark::circle::CircleAttributes;
    use gup::prelude::Selection;
    use gup::shader_function::{Vec2 as SfVec2, Vec4};

    let data = vec![
        CircleAttributes {
            center: SfVec2 { x: 0.0, y: 0.0 },
            radius: 0.1,
            fill_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        },
        CircleAttributes {
            center: SfVec2 { x: 0.5, y: 0.3 },
            radius: 0.2,
            fill_color: Vec4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        },
        CircleAttributes {
            center: SfVec2 { x: -0.3, y: 0.7 },
            radius: 0.15,
            fill_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        },
    ];

    let selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

    let mut bridge = SelectionFocusBridge::new(Default::default());
    let mut fm = FocusManager::new();

    let count =
        selection.register_focus_elements(&mut bridge, &mut fm, |attr, idx| FocusPointDescriptor {
            position: [attr.center.x, attr.center.y],
            label: format!("Circle {}: r={:.2}", idx, attr.radius),
            value: Some(attr.radius as f64),
        });

    assert_eq!(count, 3);

    // Sequential navigation through all points.
    fm.handle_key_input(KeyEvent::Tab);
    assert!(fm.describe_current_focus().unwrap().contains("Circle 0"));

    fm.handle_key_input(KeyEvent::Tab);
    assert!(fm.describe_current_focus().unwrap().contains("Circle 1"));

    fm.handle_key_input(KeyEvent::Tab);
    assert!(fm.describe_current_focus().unwrap().contains("Circle 2"));

    // Wrap around.
    fm.handle_key_input(KeyEvent::Tab);
    assert!(fm.describe_current_focus().unwrap().contains("Circle 0"));

    // Sort by value (radius) and verify order: 0.1, 0.15, 0.2
    bridge.sort_by_dimension(&mut fm, DataDimension::Value);

    fm.handle_key_input(KeyEvent::Tab);
    let first = fm.describe_current_focus().unwrap();
    assert!(
        first.contains("r=0.10"),
        "First by value should have r=0.10, got: {first}"
    );
}

#[test]
fn test_selection_focus_with_aria() {
    use gup::accessibility::AriaRole;
    use gup::accessibility::selection_focus::{FocusPointDescriptor, SelectionFocusBridge};
    use gup::mark::Circle;
    use gup::mark::circle::CircleAttributes;
    use gup::prelude::Selection;
    use gup::shader_function::{Vec2 as SfVec2, Vec4};

    let mut system = AccessibilitySystem::new();
    let chart_id = system
        .aria_tree
        .create_chart_node("Focus Test Chart".to_string(), None);

    let data = vec![
        CircleAttributes {
            center: SfVec2 { x: 0.0, y: 0.0 },
            radius: 0.1,
            fill_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        },
        CircleAttributes {
            center: SfVec2 { x: 0.5, y: 0.5 },
            radius: 0.2,
            fill_color: Vec4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        },
    ];

    let selection: Selection<CircleAttributes, Circle> = Selection::from_data(data);

    let mut bridge = SelectionFocusBridge::new(Default::default());

    let count = bridge.sync_focus_elements_with_aria(
        selection.data(),
        &mut system.focus_manager,
        &mut system.aria_tree,
        chart_id,
        |attr, idx| FocusPointDescriptor {
            position: [attr.center.x, attr.center.y],
            label: format!("Point {}", idx),
            value: None,
        },
    );

    assert_eq!(count, 2);

    // Verify ARIA nodes were created.
    assert_eq!(bridge.aria_node_ids().len(), 2);
    for node_id in bridge.aria_node_ids() {
        let node = system.aria_tree.get_node(*node_id).unwrap();
        assert_eq!(node.role, AriaRole::DataPoint);
    }

    // Verify keyboard navigation works with the accessibility system.
    system.focus_manager.handle_key_input(KeyEvent::Tab);
    let desc = system.describe_current_focus().unwrap();
    assert!(desc.contains("Point 0"));
}

#[test]
fn test_focus_updates_on_data_change() {
    use gup::accessibility::selection_focus::{FocusPointDescriptor, SelectionFocusBridge};
    use gup::mark::Circle;
    use gup::mark::circle::CircleAttributes;
    use gup::prelude::Selection;
    use gup::shader_function::{Vec2 as SfVec2, Vec4};

    fn make_circle(x: f32, y: f32) -> CircleAttributes {
        CircleAttributes {
            center: SfVec2 { x, y },
            radius: 0.1,
            fill_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        }
    }

    let mut selection: Selection<CircleAttributes, Circle> =
        Selection::from_data(vec![make_circle(0.0, 0.0), make_circle(0.5, 0.5)]);

    let mut bridge = SelectionFocusBridge::new(Default::default());
    let mut fm = FocusManager::new();

    selection.register_focus_elements(&mut bridge, &mut fm, |attr, idx| FocusPointDescriptor {
        position: [attr.center.x, attr.center.y],
        label: format!("Point {}", idx),
        value: None,
    });

    assert_eq!(bridge.last_sync_count(), 2);
    assert!(!bridge.needs_sync(2));

    // Simulate data change.
    selection.set_data(vec![
        make_circle(0.0, 0.0),
        make_circle(0.5, 0.5),
        make_circle(1.0, 1.0),
    ]);

    // Bridge detects the change.
    assert!(bridge.needs_sync(selection.len()));

    // Re-sync.
    let count =
        selection.register_focus_elements(&mut bridge, &mut fm, |attr, idx| FocusPointDescriptor {
            position: [attr.center.x, attr.center.y],
            label: format!("Updated Point {}", idx),
            value: None,
        });

    assert_eq!(count, 3);
    assert!(!bridge.needs_sync(3));

    // Verify new labels.
    fm.handle_key_input(KeyEvent::Tab);
    assert!(
        fm.describe_current_focus()
            .unwrap()
            .contains("Updated Point 0")
    );
}

#[test]
fn test_data_dimension_navigation_with_selection() {
    use gup::accessibility::NavigationMode;
    use gup::accessibility::keyboard::AccessibilityAction;
    use gup::accessibility::selection_focus::{
        FocusPointDescriptor, SelectionFocusBridge, SelectionFocusConfig,
    };

    // Use DataDimension navigation mode.
    let config = SelectionFocusConfig {
        default_navigation_mode: NavigationMode::DataDimension,
        ..Default::default()
    };

    let mut bridge = SelectionFocusBridge::new(config);
    let mut fm = FocusManager::new();

    let data = vec![
        (100.0_f32, 200.0_f32, 5.0_f64),
        (50.0, 300.0, 2.0),
        (200.0, 100.0, 8.0),
    ];

    bridge.sync_focus_elements(&data, &mut fm, |item, idx| FocusPointDescriptor {
        position: [item.0, item.1],
        label: format!("Item {} at ({}, {})", idx, item.0, item.1),
        value: Some(item.2),
    });

    fm.set_focus(0);

    // Arrow Right moves sequentially.
    let action = fm.handle_key_input(KeyEvent::ArrowRight);
    assert_eq!(action, Some(AccessibilityAction::FocusChanged));

    // Arrow Down requests dimension cycle.
    let action = fm.handle_key_input(KeyEvent::ArrowDown);
    assert_eq!(
        action,
        Some(AccessibilityAction::DimensionCycleRequested { forward: true })
    );
}

#[test]
fn test_focus_ring_style_variants() {
    use gup::accessibility::FocusRingStyle;

    let default_style = FocusRingStyle::default();
    assert_eq!(default_style.width, 2.0);
    assert_eq!(default_style.animation_speed, 0.0);

    let hc = FocusRingStyle::high_contrast();
    assert_eq!(hc.width, 3.0);
    assert_eq!(hc.color, [1.0, 1.0, 0.0, 1.0]);

    let animated = FocusRingStyle::animated();
    assert!(animated.animation_speed > 0.0);
    assert!(!animated.dash_pattern.is_empty());
}

#[test]
fn test_performance_1000_focus_elements() {
    use gup::accessibility::selection_focus::{FocusPointDescriptor, SelectionFocusBridge};
    use std::time::Instant;

    let data: Vec<(f32, f32)> = (0..1000)
        .map(|i| (i as f32, (i * 7 % 500) as f32))
        .collect();

    let mut bridge = SelectionFocusBridge::new(Default::default());
    let mut fm = FocusManager::new();

    let start = Instant::now();
    let count = bridge.sync_focus_elements(&data, &mut fm, |item, idx| FocusPointDescriptor {
        position: [item.0, item.1],
        label: format!("Point {}", idx),
        value: Some(item.0 as f64),
    });
    let elapsed = start.elapsed();

    assert_eq!(count, 1000);
    assert!(
        elapsed.as_millis() < 50,
        "Registering 1000 focus elements took {}ms (should be <50ms)",
        elapsed.as_millis()
    );

    // Test that navigation is fast.
    let nav_start = Instant::now();
    for _ in 0..100 {
        fm.handle_key_input(KeyEvent::Tab);
    }
    let nav_elapsed = nav_start.elapsed();

    assert!(
        nav_elapsed.as_millis() < 10,
        "100 Tab navigations took {}ms (should be <10ms)",
        nav_elapsed.as_millis()
    );
}
