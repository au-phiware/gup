// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Windows UI Automation integration for NVDA and JAWS support.
//!
//! This module provides Windows API bindings to the UI Automation framework,
//! allowing Gup visualizations to be accessed by NVDA, JAWS, and other
//! assistive technologies on Windows.

#![cfg(target_os = "windows")]

use crate::accessibility::aria::{AriaNode, AriaRole, AriaTree, AriaUpdate, NodeId};
use crate::accessibility::platform::{
    AccessibilityError, AnnouncementPriority, PlatformAccessibility,
};
use std::collections::HashMap;
use windows::Win32::UI::Accessibility::{
    UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId,
    UIA_DataItemControlTypeId, UIA_GroupControlTypeId, UIA_ImageControlTypeId,
    UIA_ListControlTypeId, UIA_NamePropertyId, UIA_SeparatorControlTypeId, UIA_TextControlTypeId,
    UIA_ToolTipControlTypeId,
};

/// Windows UI Automation implementation.
///
/// # Integration with AccessibilitySystem
///
/// This implementation is designed to be called from `AccessibilitySystem`,
/// which maintains the ARIA tree and generates updates. The typical flow is:
///
/// 1. `AccessibilitySystem` creates or updates ARIA nodes
/// 2. Updates are queued in the ARIA tree
/// 3. `AccessibilitySystem` drains the update queue and passes updates here
/// 4. This implementation creates/updates corresponding UI Automation providers
///
/// # Provider Implementation
///
/// Windows UI Automation uses the provider pattern where applications implement
/// IRawElementProviderSimple and related interfaces. These providers expose
/// properties and patterns that screen readers query.
///
/// # Window Integration
///
/// For NVDA/JAWS to discover accessibility elements, they must be attached
/// to the window's automation tree. This is done via:
/// - Getting the HWND from winit's `raw_window_handle()`
/// - Creating UI Automation providers for each element
/// - Raising automation events when content changes
pub struct WindowsAccessibility {
    /// Whether the system is initialized
    initialized: bool,

    /// Map from Gup node IDs to UI Automation element data
    elements: HashMap<u64, UIAElementData>,

    /// The root accessibility element
    root_element: Option<UIAElementData>,

    /// Reference to the ARIA tree for node lookups
    aria_tree: Option<AriaTree>,
}

/// Data for a UI Automation element.
///
/// This stores the information needed to implement IRawElementProviderSimple
/// and respond to property queries from screen readers.
#[derive(Debug, Clone)]
struct UIAElementData {
    /// Node ID from ARIA tree
    node_id: u64,

    /// Control type ID for this element
    control_type: i32,

    /// Name property (label)
    name: String,

    /// Automation ID
    automation_id: String,

    /// Description/help text
    description: Option<String>,

    /// Value for data elements
    value: Option<String>,

    /// Child element IDs
    children: Vec<u64>,
}

impl WindowsAccessibility {
    /// Create a new Windows accessibility implementation.
    pub fn new() -> Self {
        Self {
            initialized: false,
            elements: HashMap::new(),
            root_element: None,
            aria_tree: None,
        }
    }

    /// Convert AriaRole to UI Automation control type ID.
    fn aria_role_to_control_type(role: AriaRole) -> i32 {
        match role {
            AriaRole::Chart => UIA_ImageControlTypeId,
            AriaRole::ChartSeries => UIA_ListControlTypeId,
            AriaRole::DataPoint => UIA_DataItemControlTypeId,
            AriaRole::Legend => UIA_GroupControlTypeId,
            AriaRole::Axis => UIA_SeparatorControlTypeId,
            AriaRole::Tooltip => UIA_ToolTipControlTypeId,
            AriaRole::Control => UIA_ButtonControlTypeId,
        }
    }

    /// Create UI Automation element data from an ARIA node.
    fn create_uia_element(&self, node: &AriaNode) -> UIAElementData {
        UIAElementData {
            node_id: node.id.as_u64(),
            control_type: Self::aria_role_to_control_type(node.role),
            name: node.label.clone(),
            automation_id: format!("gup-{}", node.id.as_u64()),
            description: node.description.clone(),
            value: node.value.clone(),
            children: node.children.iter().map(|id| id.as_u64()).collect(),
        }
    }

    /// Update element data from an ARIA node.
    fn update_element_from_node(&mut self, node: &AriaNode) -> Result<(), AccessibilityError> {
        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "Windows accessibility not initialized".to_string(),
            ));
        }

        let element_data = self.create_uia_element(node);
        let node_id = node.id.as_u64();

        // Update or insert element
        self.elements.insert(node_id, element_data.clone());

        // If this is the first element, make it the root
        if self.root_element.is_none() {
            self.root_element = Some(element_data.clone());
        }

        log::debug!(
            "Updated UI Automation element: {} (type: {})",
            element_data.name,
            element_data.control_type
        );

        Ok(())
    }

    /// Create an element for a node from the ARIA tree.
    pub fn create_element_for_node(&mut self, node: &AriaNode) -> Result<(), AccessibilityError> {
        self.update_element_from_node(node)
    }

    /// Update an existing element with new node data.
    pub fn update_element_for_node(&mut self, node: &AriaNode) -> Result<(), AccessibilityError> {
        self.update_element_from_node(node)
    }

    /// Get element data for a node ID.
    pub fn get_element(&self, node_id: u64) -> Option<&UIAElementData> {
        self.elements.get(&node_id)
    }

    /// Get the root element.
    pub fn get_root_element(&self) -> Option<&UIAElementData> {
        self.root_element.as_ref()
    }

    /// Set the ARIA tree reference for node lookups.
    pub fn set_aria_tree(&mut self, tree: AriaTree) {
        self.aria_tree = Some(tree);
    }

    /// Raise a UI Automation notification event.
    ///
    /// In a full implementation, this would call UiaRaiseNotificationEvent
    /// to trigger screen reader announcements.
    fn raise_notification(
        &self,
        message: &str,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        // TODO: Call UiaRaiseNotificationEvent with appropriate parameters
        // This requires creating an IRawElementProviderSimple implementation
        // and using the UIA notification API

        log::debug!(
            "Windows UI Automation notification ({:?}): {}",
            priority,
            message
        );

        Ok(())
    }

    /// Raise a focus change event.
    ///
    /// In a full implementation, this would call UiaRaiseAutomationEvent
    /// with UIA_AutomationFocusChangedEventId.
    fn raise_focus_event(&self, element_id: u64) -> Result<(), AccessibilityError> {
        // TODO: Call UiaRaiseAutomationEvent for focus change
        // This requires getting the IRawElementProviderSimple for the element

        log::debug!(
            "Windows UI Automation focus changed to element: {}",
            element_id
        );

        Ok(())
    }
}

impl Default for WindowsAccessibility {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAccessibility for WindowsAccessibility {
    fn initialize(&mut self) -> Result<(), AccessibilityError> {
        // TODO: Initialize COM and UI Automation
        // This would include:
        // - CoInitializeEx for COM initialization
        // - Creating UIA provider objects
        // - Registering with the UI Automation framework

        log::info!("Initializing Windows UI Automation");

        // Mark as initialized
        self.initialized = true;

        Ok(())
    }

    fn update_accessibility_tree(
        &mut self,
        updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError> {
        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "Windows accessibility not initialized".to_string(),
            ));
        }

        // Process each update
        for update in updates {
            match update {
                AriaUpdate::NodeCreated { node_id } => {
                    // When a node is created, we need the full node data
                    // to create the element. This requires the ARIA tree.
                    if let Some(ref tree) = self.aria_tree {
                        if let Some(node) = tree.get_node(*node_id) {
                            self.create_element_for_node(node)?;
                        }
                    }
                }
                AriaUpdate::NodeUpdated { node_id } => {
                    // Update existing element with new data
                    if let Some(ref tree) = self.aria_tree {
                        if let Some(node) = tree.get_node(*node_id) {
                            self.update_element_for_node(node)?;
                        }
                    }
                }
                AriaUpdate::NodeRemoved { node_id } => {
                    // Remove element from map
                    let node_id_u64 = node_id.as_u64();
                    self.elements.remove(&node_id_u64);
                    log::debug!("Removed UI Automation element: {}", node_id_u64);
                }
                AriaUpdate::FocusChanged { node_id } => {
                    // Raise focus changed event
                    self.raise_focus_event(node_id.as_u64())?;
                }
                AriaUpdate::LiveRegion {
                    id,
                    content,
                    urgency,
                } => {
                    // Convert ARIA urgency to announcement priority
                    let priority = match urgency {
                        crate::accessibility::aria::AriaLive::Assertive => {
                            AnnouncementPriority::Assertive
                        }
                        crate::accessibility::aria::AriaLive::Polite => {
                            AnnouncementPriority::Polite
                        }
                        crate::accessibility::aria::AriaLive::Off => AnnouncementPriority::Off,
                    };

                    // Raise notification for live region update
                    self.raise_notification(content, priority)?;
                    log::debug!("Live region '{}' updated: {}", id, content);
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
                "Windows accessibility not initialized".to_string(),
            ));
        }

        self.raise_notification(message, priority)
    }

    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError> {
        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "Windows accessibility not initialized".to_string(),
            ));
        }

        // Parse element ID (format: "gup-{node_id}")
        let node_id: u64 = element_id
            .strip_prefix("gup-")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                AccessibilityError::FocusFailed(format!("Invalid element ID: {}", element_id))
            })?;

        // Verify element exists
        if !self.elements.contains_key(&node_id) {
            return Err(AccessibilityError::FocusFailed(format!(
                "Element not found: {}",
                element_id
            )));
        }

        self.raise_focus_event(node_id)
    }

    fn platform_name(&self) -> &str {
        "Windows (UI Automation)"
    }

    fn is_available(&self) -> bool {
        // UI Automation is available on Windows Vista and later
        // We're compiling for Windows, so it's always available
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_mapping() {
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::Chart),
            UIA_ImageControlTypeId
        );
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::ChartSeries),
            UIA_ListControlTypeId
        );
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::DataPoint),
            UIA_DataItemControlTypeId
        );
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::Legend),
            UIA_GroupControlTypeId
        );
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::Axis),
            UIA_SeparatorControlTypeId
        );
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::Tooltip),
            UIA_ToolTipControlTypeId
        );
        assert_eq!(
            WindowsAccessibility::aria_role_to_control_type(AriaRole::Control),
            UIA_ButtonControlTypeId
        );
    }

    #[test]
    fn test_initialization() {
        let mut accessibility = WindowsAccessibility::new();

        // Should not be initialized yet
        assert!(!accessibility.initialized);
        assert!(accessibility.root_element.is_none());

        // Should succeed
        assert!(accessibility.initialize().is_ok());
        assert!(accessibility.initialized);
    }

    #[test]
    fn test_operations_before_init() {
        let mut accessibility = WindowsAccessibility::new();

        // Should fail before initialization
        assert!(
            accessibility
                .announce("test", AnnouncementPriority::Polite)
                .is_err()
        );
        assert!(accessibility.set_focus("gup-123").is_err());
    }

    #[test]
    fn test_element_creation() {
        let mut accessibility = WindowsAccessibility::new();
        accessibility.initialize().unwrap();

        let node = AriaNode::new(AriaRole::Chart, "Test Chart".to_string());

        // Create element
        assert!(accessibility.create_element_for_node(&node).is_ok());

        // Element should be in map
        let element = accessibility.get_element(node.id.as_u64());
        assert!(element.is_some());

        let element = element.unwrap();
        assert_eq!(element.name, "Test Chart");
        assert_eq!(element.control_type, UIA_ImageControlTypeId);
    }

    #[test]
    fn test_element_update() {
        let mut accessibility = WindowsAccessibility::new();
        accessibility.initialize().unwrap();

        let mut node = AriaNode::new(AriaRole::DataPoint, "Point 1".to_string());
        let node_id = node.id;

        // Create element
        accessibility.create_element_for_node(&node).unwrap();

        // Update node data
        node.label = "Point 1 Updated".to_string();
        node.value = Some("42".to_string());

        // Update element
        assert!(accessibility.update_element_for_node(&node).is_ok());

        // Check updated data
        let element = accessibility.get_element(node_id.as_u64()).unwrap();
        assert_eq!(element.name, "Point 1 Updated");
        assert_eq!(element.value, Some("42".to_string()));
    }

    #[test]
    fn test_platform_info() {
        let accessibility = WindowsAccessibility::new();
        assert_eq!(accessibility.platform_name(), "Windows (UI Automation)");
        assert!(accessibility.is_available());
    }
}
