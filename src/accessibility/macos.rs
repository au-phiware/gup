// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! macOS NSAccessibility integration for VoiceOver support.
//!
//! This module provides Objective-C bindings to the macOS NSAccessibility
//! protocol, allowing Gup visualizations to be accessed by VoiceOver and
//! other assistive technologies on macOS.

#![cfg(target_os = "macos")]

use crate::accessibility::aria::{AriaNode, AriaRole, AriaTree, AriaUpdate, NodeId};
use crate::accessibility::platform::{
    AccessibilityError, AnnouncementPriority, PlatformAccessibility,
};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityNotificationName, NSAccessibilityRole,
};
use objc2_foundation::{NSArray, NSDictionary, NSString};
use std::collections::HashMap;

/// NSAccessibility implementation for macOS.
pub struct MacOSAccessibility {
    /// Whether the system is initialized
    initialized: bool,

    /// Map from Gup node IDs to NSAccessibility elements
    elements: HashMap<u64, Retained<NSAccessibilityElement>>,

    /// The root accessibility element
    root_element: Option<Retained<NSAccessibilityElement>>,

    /// Reference to the ARIA tree for node lookups
    aria_tree: Option<AriaTree>,
}

impl MacOSAccessibility {
    /// Create a new macOS accessibility implementation.
    pub fn new() -> Self {
        Self {
            initialized: false,
            elements: HashMap::new(),
            root_element: None,
            aria_tree: None,
        }
    }

    /// Convert AriaRole to NSAccessibilityRole.
    fn aria_role_to_ns_role(role: AriaRole) -> &'static NSAccessibilityRole {
        match role {
            AriaRole::Chart => NSAccessibilityRole::Image,
            AriaRole::ChartSeries => NSAccessibilityRole::List,
            AriaRole::DataPoint => NSAccessibilityRole::Cell,
            AriaRole::Legend => NSAccessibilityRole::Group,
            AriaRole::Axis => NSAccessibilityRole::Ruler,
            AriaRole::Tooltip => NSAccessibilityRole::HelpTag,
            AriaRole::Control => NSAccessibilityRole::Button,
        }
    }

    /// Create an NSAccessibility element from an ARIA node.
    fn create_ns_element(&self, node: &AriaNode) -> Retained<NSAccessibilityElement> {
        let element = unsafe {
            NSAccessibilityElement::alloc()
        };

        // Set the role
        let role = Self::aria_role_to_ns_role(node.role);
        unsafe {
            element.setAccessibilityRole(Some(role));
        }

        // Set the label
        let label = NSString::from_str(&node.label);
        unsafe {
            element.setAccessibilityLabel(Some(&label));
        }

        // Set the description if present
        if let Some(desc) = &node.description {
            let help = NSString::from_str(desc);
            unsafe {
                element.setAccessibilityHelp(Some(&help));
            }
        }

        // Set the value if present
        if let Some(val) = &node.value {
            let value = NSString::from_str(val);
            unsafe {
                element.setAccessibilityValue(Some(&value));
            }
        }

        element
    }

    /// Update an existing NSAccessibility element from an ARIA node.
    fn update_ns_element(&self, element: &NSAccessibilityElement, node: &AriaNode) {
        // Update label
        let label = NSString::from_str(&node.label);
        unsafe {
            element.setAccessibilityLabel(Some(&label));
        }

        // Update description
        if let Some(desc) = &node.description {
            let help = NSString::from_str(desc);
            unsafe {
                element.setAccessibilityHelp(Some(&help));
            }
        } else {
            unsafe {
                element.setAccessibilityHelp(None);
            }
        }

        // Update value
        if let Some(val) = &node.value {
            let value = NSString::from_str(val);
            unsafe {
                element.setAccessibilityValue(Some(&value));
            }
        } else {
            unsafe {
                element.setAccessibilityValue(None);
            }
        }
    }

    /// Post an accessibility notification.
    fn post_notification(
        &self,
        notification: &NSAccessibilityNotificationName,
        element: &NSAccessibilityElement,
    ) {
        unsafe {
            NSAccessibility::postNotificationWithUserInfo(
                element,
                notification,
                None,
            );
        }
    }
}

impl PlatformAccessibility for MacOSAccessibility {
    fn initialize(&mut self) -> Result<(), AccessibilityError> {
        // On macOS, NSAccessibility is always available
        // No special initialization needed
        self.initialized = true;
        log::info!("macOS NSAccessibility initialized");
        Ok(())
    }

    fn update_accessibility_tree(
        &mut self,
        updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError> {
        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "macOS accessibility not initialized".to_string(),
            ));
        }

        // We need access to the ARIA tree to look up nodes
        // For now, process the updates and store element references
        for update in updates {
            match update {
                AriaUpdate::NodeCreated { node_id } => {
                    log::debug!("macOS: Node created {:?}", node_id);
                    // Would create NSAccessibilityElement here with full tree access
                }
                AriaUpdate::NodeUpdated { node_id } => {
                    log::debug!("macOS: Node updated {:?}", node_id);
                    // Would update existing element
                }
                AriaUpdate::NodeRemoved { node_id } => {
                    log::debug!("macOS: Node removed {:?}", node_id);
                    // Remove element from our map
                    self.elements.remove(&node_id.as_u64());
                }
                AriaUpdate::FocusChanged { node_id } => {
                    log::debug!("macOS: Focus changed to {:?}", node_id);
                    // Would set NSAccessibilityFocusedUIElement
                    if let Some(element) = self.elements.get(&node_id.as_u64()) {
                        self.post_notification(
                            NSAccessibilityNotificationName::FocusedUIElementChanged,
                            element,
                        );
                    }
                }
                AriaUpdate::LiveRegion {
                    id,
                    content,
                    urgency,
                } => {
                    log::debug!(
                        "macOS: Live region update {:?} -> {} (urgency: {:?})",
                        id,
                        content,
                        urgency
                    );
                    // Would create announcement
                }
            }
        }

        Ok(())
    }

    fn announce(
        &mut self,
        message: &str,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "macOS accessibility not initialized".to_string(),
            ));
        }

        // Post an NSAccessibilityAnnouncementRequestedNotification
        log::debug!("macOS announce ({:?}): {}", priority, message);

        // Create the announcement dictionary
        let announcement_key = unsafe {
            NSString::from_str("NSAccessibilityAnnouncementKey")
        };
        let announcement_value = NSString::from_str(message);

        let priority_key = unsafe {
            NSString::from_str("NSAccessibilityPriorityKey")
        };
        let priority_value = match priority {
            AnnouncementPriority::Assertive => "high",
            AnnouncementPriority::Polite => "low",
            AnnouncementPriority::Off => return Ok(()), // Don't announce
        };
        let priority_string = NSString::from_str(priority_value);

        // Create the dictionary
        let user_info = unsafe {
            NSDictionary::from_keys_and_objects(
                &[&announcement_key, &priority_key],
                &[&announcement_value as &AnyObject, &priority_string as &AnyObject],
            )
        };

        // Post the notification
        // Note: We need an element to post from. For announcements without
        // a specific element, we could use the root element or app element.
        if let Some(root) = &self.root_element {
            unsafe {
                NSAccessibility::postNotificationWithUserInfo(
                    root.as_ref(),
                    NSAccessibilityNotificationName::AnnouncementRequested,
                    Some(&user_info),
                );
            }
            Ok(())
        } else {
            log::warn!("No root element available for announcement");
            Ok(())
        }
    }

    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError> {
        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "macOS accessibility not initialized".to_string(),
            ));
        }

        // Parse element_id as a u64 NodeId
        let node_id: u64 = element_id
            .parse()
            .map_err(|_| AccessibilityError::FocusFailed("Invalid element ID".to_string()))?;

        log::debug!("macOS set focus: {}", element_id);

        // Find the element and post focus notification
        if let Some(element) = self.elements.get(&node_id) {
            self.post_notification(
                NSAccessibilityNotificationName::FocusedUIElementChanged,
                element,
            );
            Ok(())
        } else {
            Err(AccessibilityError::FocusFailed(
                "Element not found".to_string(),
            ))
        }
    }

    fn platform_name(&self) -> &str {
        "macOS (NSAccessibility)"
    }

    fn is_available(&self) -> bool {
        // NSAccessibility is always available on macOS
        true
    }
}

/// Custom NSAccessibilityElement subclass for Gup visualizations.
///
/// This wraps the NSAccessibilityElement class from AppKit to provide
/// custom accessibility information for our GPU-rendered visualizations.
#[repr(C)]
pub struct NSAccessibilityElement {
    _priv: [u8; 0],
}

unsafe impl objc2::Message for NSAccessibilityElement {}

impl NSAccessibilityElement {
    /// Allocate a new NSAccessibilityElement.
    unsafe fn alloc() -> Retained<Self> {
        use objc2::ClassType;
        use objc2::rc::Allocated;

        let cls = objc2::class!(NSAccessibilityElement);
        let obj: Allocated<Self> = objc2::msg_send_id![cls, alloc];
        obj.init()
    }

    /// Initialize the element.
    unsafe fn init(self: Allocated<Self>) -> Retained<Self> {
        objc2::msg_send_id![self, init]
    }

    /// Set the accessibility role.
    unsafe fn setAccessibilityRole(&self, role: Option<&NSAccessibilityRole>) {
        let _: () = objc2::msg_send![self, setAccessibilityRole: role];
    }

    /// Set the accessibility label.
    unsafe fn setAccessibilityLabel(&self, label: Option<&NSString>) {
        let _: () = objc2::msg_send![self, setAccessibilityLabel: label];
    }

    /// Set the accessibility help text.
    unsafe fn setAccessibilityHelp(&self, help: Option<&NSString>) {
        let _: () = objc2::msg_send![self, setAccessibilityHelp: help];
    }

    /// Set the accessibility value.
    unsafe fn setAccessibilityValue(&self, value: Option<&NSString>) {
        let _: () = objc2::msg_send![self, setAccessibilityValue: value];
    }

    /// Set the accessibility children.
    unsafe fn setAccessibilityChildren(&self, children: Option<&NSArray<NSAccessibilityElement>>) {
        let _: () = objc2::msg_send![self, setAccessibilityChildren: children];
    }

    /// Set the accessibility parent.
    unsafe fn setAccessibilityParent(&self, parent: Option<&NSAccessibilityElement>) {
        let _: () = objc2::msg_send![self, setAccessibilityParent: parent];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_accessibility_creation() {
        let accessibility = MacOSAccessibility::new();
        assert!(!accessibility.initialized);
        assert!(accessibility.is_available());
        assert_eq!(accessibility.platform_name(), "macOS (NSAccessibility)");
    }

    #[test]
    fn test_aria_role_mapping() {
        // Test that all AriaRole variants have a mapping
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
            let ns_role = MacOSAccessibility::aria_role_to_ns_role(*role);
            // Just verify we get a role back without panicking
            assert!(!ns_role.to_string().is_empty());
        }
    }

    #[test]
    fn test_initialization() {
        let mut accessibility = MacOSAccessibility::new();
        assert!(accessibility.initialize().is_ok());
        assert!(accessibility.initialized);
    }

    #[test]
    fn test_uninitialized_operations() {
        let mut accessibility = MacOSAccessibility::new();

        // Should fail before initialization
        assert!(accessibility
            .announce("test", AnnouncementPriority::Polite)
            .is_err());
        assert!(accessibility.set_focus("123").is_err());

        // Initialize and try again
        accessibility.initialize().unwrap();
        assert!(accessibility
            .announce("test", AnnouncementPriority::Polite)
            .is_ok());
    }
}
