// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for platform-specific accessibility.

use gup::accessibility::{AccessibilitySystem, AnnouncementPriority};

#[test]
fn test_platform_accessibility_integration() {
    let mut system = AccessibilitySystem::new();

    // Check that platform is available
    assert!(!system.platform_name().is_empty());

    // Test platform announcements
    let result = system.announce("Test announcement", AnnouncementPriority::Polite);
    assert!(result.is_ok(), "Platform announcement should succeed");

    // Test that we can get platform info
    let platform_name = system.platform_name();
    assert!(
        platform_name.contains("Linux")
            || platform_name.contains("macOS")
            || platform_name.contains("Windows")
            || platform_name.contains("Web")
            || platform_name.contains("Null")
    );
}

#[test]
fn test_platform_focus_management() {
    let mut system = AccessibilitySystem::new();

    // Test setting focus
    let result = system.set_platform_focus("test-element");
    assert!(result.is_ok(), "Setting platform focus should succeed");
}

#[test]
fn test_accessibility_tree_updates() {
    let mut system = AccessibilitySystem::new();

    // Create a chart node in ARIA tree
    let _node_id = system.aria_tree.create_chart_node(
        "Test Chart".to_string(),
        Some("A test visualization".to_string()),
    );

    // Get pending updates - should trigger platform bridge update
    let updates = system.get_pending_aria_updates();
    assert!(!updates.is_empty(), "Should have pending ARIA updates");
}

#[test]
fn test_disabled_accessibility_skips_platform_calls() {
    let mut system = AccessibilitySystem::new();

    // Disable accessibility
    system.set_enabled(false);

    // These should not fail but should be no-ops
    let result = system.announce("Test", AnnouncementPriority::Polite);
    assert!(result.is_ok());

    let result = system.set_platform_focus("test");
    assert!(result.is_ok());
}

#[test]
fn test_announcement_priorities() {
    let mut system = AccessibilitySystem::new();

    // Test all priority levels
    assert!(
        system
            .announce("Polite", AnnouncementPriority::Polite)
            .is_ok()
    );
    assert!(
        system
            .announce("Assertive", AnnouncementPriority::Assertive)
            .is_ok()
    );
    assert!(system.announce("Off", AnnouncementPriority::Off).is_ok());
}
