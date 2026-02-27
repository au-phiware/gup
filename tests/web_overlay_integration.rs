// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Web DOM Overlay accessibility features

// FIXME: wasm_overlay_tests module uses outdated accessibility API
// (AriaTree::create_node, AriaNode { parent }, DomOverlayConfig missing fields).
// Disabled until GUP-237 (WASM Integration Test Suite) updates the tests.
#[cfg(all(target_arch = "wasm32", feature = "__wasm_accessibility_tests"))]
mod wasm_overlay_tests {
    use gup::accessibility::{
        AccessibilitySystem, AriaNode, AriaRole, AriaTree, AriaUpdate, DomOverlayConfig, NodeId,
        WebDomOverlay,
    };
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_dom_overlay_initialization() {
        let overlay = WebDomOverlay::new();
        assert!(
            overlay.is_ok(),
            "WebDomOverlay should initialize successfully"
        );
    }

    #[wasm_bindgen_test]
    fn test_custom_overlay_config() {
        let config = DomOverlayConfig {
            container_id: "test-overlay".to_string(),
            canvas_id: "test-canvas".to_string(),
            keyboard_enabled: true,
            pointer_enabled: true,
            show_focus_indicators: true,
            z_index: 2000,
        };

        let overlay = WebDomOverlay::with_config(config.clone());
        assert!(overlay.is_ok(), "Custom config should work");
    }

    #[wasm_bindgen_test]
    fn test_aria_tree_synchronization() {
        let mut overlay = WebDomOverlay::new().expect("Failed to create overlay");
        overlay.initialize().expect("Failed to initialize");

        let mut aria_tree = AriaTree::new();
        let node_id = aria_tree.create_node(AriaNode {
            id: NodeId::new(),
            role: AriaRole::Chart,
            label: "Test Chart".to_string(),
            description: Some("A test chart for DOM overlay".to_string()),
            value: None,
            parent: None,
            children: Vec::new(),
        });

        let updates = vec![AriaUpdate::NodeCreated { node_id }];

        let result = overlay.update_from_aria_tree(&updates, &aria_tree);
        assert!(
            result.is_ok(),
            "ARIA tree updates should be applied to overlay"
        );
    }

    #[wasm_bindgen_test]
    fn test_accessibility_system_with_overlay() {
        let system = AccessibilitySystem::new();

        // System should initialize successfully with Web platform
        assert!(system.is_enabled());
        assert_eq!(system.platform_name(), "Web (ARIA)");
        assert!(system.is_platform_available());
    }

    #[wasm_bindgen_test]
    fn test_focus_indicators_css() {
        let overlay = WebDomOverlay::new().expect("Failed to create overlay");
        let result = overlay.initialize();
        assert!(result.is_ok(), "Focus indicator CSS should be injected");

        // Check that style element was created
        let window = web_sys::window().expect("No window");
        let document = window.document().expect("No document");
        let style_element = document.get_element_by_id("gup-focus-styles");
        assert!(style_element.is_some(), "Focus styles should be in DOM");
    }

    #[wasm_bindgen_test]
    fn test_keyboard_navigation_setup() {
        let mut overlay = WebDomOverlay::new().expect("Failed to create overlay");
        let result = overlay.initialize();
        assert!(result.is_ok(), "Keyboard navigation should be set up");

        let window = web_sys::window().expect("No window");
        let document = window.document().expect("No document");
        let container = document.get_element_by_id("gup-overlay");
        assert!(container.is_some(), "Overlay container should exist");

        if let Some(element) = container {
            let tabindex = element.get_attribute("tabindex");
            assert_eq!(
                tabindex,
                Some("0".to_string()),
                "Container should be focusable"
            );
        }
    }

    #[wasm_bindgen_test]
    fn test_multiple_aria_nodes() {
        let mut overlay = WebDomOverlay::new().expect("Failed to create overlay");
        overlay.initialize().expect("Failed to initialize");

        let mut aria_tree = AriaTree::new();

        // Create multiple nodes
        let chart_id = aria_tree.create_node(AriaNode {
            id: NodeId::new(),
            role: AriaRole::Chart,
            label: "Main Chart".to_string(),
            description: None,
            value: None,
            parent: None,
            children: Vec::new(),
        });

        let point_id = aria_tree.create_node(AriaNode {
            id: NodeId::new(),
            role: AriaRole::DataPoint,
            label: "Data Point 1".to_string(),
            description: Some("Value: 42".to_string()),
            value: Some("42".to_string()),
            parent: Some(chart_id),
            children: Vec::new(),
        });

        let updates = vec![
            AriaUpdate::NodeCreated { node_id: chart_id },
            AriaUpdate::NodeCreated { node_id: point_id },
        ];

        let result = overlay.update_from_aria_tree(&updates, &aria_tree);
        assert!(result.is_ok(), "Multiple nodes should be created");
    }

    #[wasm_bindgen_test]
    fn test_node_removal() {
        let mut overlay = WebDomOverlay::new().expect("Failed to create overlay");
        overlay.initialize().expect("Failed to initialize");

        let mut aria_tree = AriaTree::new();

        let node_id = aria_tree.create_node(AriaNode {
            id: NodeId::new(),
            role: AriaRole::DataPoint,
            label: "Temporary Point".to_string(),
            description: None,
            value: None,
            parent: None,
            children: Vec::new(),
        });

        // Create then remove
        let updates = vec![
            AriaUpdate::NodeCreated { node_id },
            AriaUpdate::NodeRemoved { node_id },
        ];

        let result = overlay.update_from_aria_tree(&updates, &aria_tree);
        assert!(result.is_ok(), "Node removal should work");
    }

    #[wasm_bindgen_test]
    fn test_high_contrast_css_support() {
        let overlay = WebDomOverlay::new().expect("Failed to create overlay");
        overlay.initialize().expect("Failed to initialize");

        // Verify that high contrast media query is in the CSS
        let window = web_sys::window().expect("No window");
        let document = window.document().expect("No document");

        if let Some(style) = document.get_element_by_id("gup-focus-styles") {
            let content = style.text_content().unwrap_or_default();
            assert!(
                content.contains("prefers-contrast: high"),
                "CSS should include high contrast support"
            );
            assert!(
                content.contains("prefers-reduced-motion"),
                "CSS should include reduced motion support"
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_web_overlay_only_for_wasm() {
    // This test just ensures the module compiles on non-wasm platforms
    // The actual overlay functionality is only available on wasm32
    // No assertions needed - compilation success is the test
}
