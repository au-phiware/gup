// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform-specific accessibility integration.
//!
//! This module provides platform-specific bridges that translate Gup's
//! platform-agnostic accessibility system into native platform APIs.
//!
//! Supported platforms:
//! - macOS: NSAccessibility
//! - Windows: UI Automation API
//! - Linux: ATK (Accessibility Toolkit) / AT-SPI2
//! - Web: ARIA attributes in DOM

use crate::accessibility::aria::AriaUpdate;
#[cfg(target_arch = "wasm32")]
use crate::accessibility::aria::{AriaTree, NodeId};
use std::fmt;

// Import platform-specific implementations
#[cfg(target_os = "macos")]
pub use crate::accessibility::macos::MacOSAccessibility;

#[cfg(target_os = "windows")]
pub use crate::accessibility::windows::WindowsAccessibility;

/// Platform abstraction for accessibility integration.
///
/// Different platforms have different accessibility frameworks. This trait
/// provides a unified interface for translating Gup's accessibility system
/// into platform-native APIs.
///
/// On native platforms, implementations must be `Send + Sync` for multi-threaded
/// access. On WASM (single-threaded), these bounds are relaxed because DOM types
/// (e.g. `Rc`, JS objects) are inherently non-Send.
#[cfg(not(target_arch = "wasm32"))]
pub trait PlatformAccessibility: Send + Sync {
    /// Initialize platform-specific accessibility.
    fn initialize(&mut self) -> Result<(), AccessibilityError>;

    /// Update the platform accessibility tree with ARIA node information.
    fn update_accessibility_tree(
        &mut self,
        updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError>;

    /// Announce a message to the screen reader.
    fn announce(
        &mut self,
        message: &str,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError>;

    /// Set the currently focused element.
    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError>;

    /// Get the platform name for debugging.
    fn platform_name(&self) -> &str;

    /// Check if the platform accessibility API is available.
    fn is_available(&self) -> bool;
}

/// Platform abstraction for accessibility integration (WASM variant).
///
/// On WASM, `Send + Sync` bounds are relaxed because JavaScript/DOM types
/// cannot be shared across threads (WASM is single-threaded).
#[cfg(target_arch = "wasm32")]
pub trait PlatformAccessibility {
    /// Initialize platform-specific accessibility.
    fn initialize(&mut self) -> Result<(), AccessibilityError>;

    /// Update the platform accessibility tree with ARIA node information.
    fn update_accessibility_tree(
        &mut self,
        updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError>;

    /// Announce a message to the screen reader.
    fn announce(
        &mut self,
        message: &str,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError>;

    /// Set the currently focused element.
    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError>;

    /// Get the platform name for debugging.
    fn platform_name(&self) -> &str;

    /// Check if the platform accessibility API is available.
    fn is_available(&self) -> bool;
}

/// Priority level for screen reader announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementPriority {
    /// Polite announcements wait for screen reader to be idle.
    Polite,
    /// Assertive announcements interrupt current speech.
    Assertive,
    /// Off announcements are not spoken (only updated in tree).
    Off,
}

/// Errors that can occur during platform accessibility operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityError {
    /// Platform API is not available or not initialized.
    PlatformUnavailable(String),
    /// Failed to update accessibility tree.
    TreeUpdateFailed(String),
    /// Failed to announce message.
    AnnouncementFailed(String),
    /// Failed to set focus.
    FocusFailed(String),
    /// Other platform-specific error.
    Other(String),
}

impl fmt::Display for AccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlatformUnavailable(msg) => write!(f, "Platform unavailable: {}", msg),
            Self::TreeUpdateFailed(msg) => write!(f, "Tree update failed: {}", msg),
            Self::AnnouncementFailed(msg) => write!(f, "Announcement failed: {}", msg),
            Self::FocusFailed(msg) => write!(f, "Focus failed: {}", msg),
            Self::Other(msg) => write!(f, "Accessibility error: {}", msg),
        }
    }
}

impl std::error::Error for AccessibilityError {}

/// Create a platform-specific accessibility implementation for the current platform.
pub fn create_platform_accessibility() -> Box<dyn PlatformAccessibility> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSAccessibility::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsAccessibility::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxAccessibility::new())
    }

    #[cfg(target_arch = "wasm32")]
    {
        Box::new(WebAccessibility::new())
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_arch = "wasm32"
    )))]
    {
        Box::new(NullAccessibility::new())
    }
}

// ============================================================================
// Linux ATK/AT-SPI2 Implementation
// ============================================================================

/// Linux accessibility implementation using AT-SPI2.
#[cfg(target_os = "linux")]
pub struct LinuxAccessibility {
    atspi_manager: crate::accessibility::atspi::AtSpiManager,
    runtime: Option<tokio::runtime::Runtime>,
}

#[cfg(target_os = "linux")]
impl Default for LinuxAccessibility {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl LinuxAccessibility {
    pub fn new() -> Self {
        Self {
            atspi_manager: crate::accessibility::atspi::AtSpiManager::new(
                "Gup Visualization".to_string(),
            ),
            runtime: None,
        }
    }
}

#[cfg(target_os = "linux")]
impl PlatformAccessibility for LinuxAccessibility {
    fn initialize(&mut self) -> Result<(), AccessibilityError> {
        // Create a runtime for async D-Bus operations
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                AccessibilityError::Other(format!("Failed to create tokio runtime: {}", e))
            })?;

        // Connect to AT-SPI2 accessibility bus
        runtime.block_on(async {
            self.atspi_manager.connect().await.map_err(|e| {
                AccessibilityError::Other(format!("Failed to connect to AT-SPI2: {}", e))
            })
        })?;

        self.runtime = Some(runtime);
        log::info!("Linux AT-SPI2 accessibility initialized");
        Ok(())
    }

    fn update_accessibility_tree(
        &mut self,
        updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError> {
        if self.runtime.is_none() {
            return Err(AccessibilityError::PlatformUnavailable(
                "Linux accessibility not initialized".to_string(),
            ));
        }

        // We need access to the ARIA nodes to translate updates
        // For now, process the updates without full node data
        // In production, we would need to pass the full node map

        if let Some(runtime) = &self.runtime {
            runtime.block_on(async {
                // Process updates through AT-SPI2 manager
                // Note: This simplified version doesn't have access to full aria_nodes
                // A full implementation would need to pass or maintain the node map
                for update in updates {
                    match update {
                        AriaUpdate::NodeCreated { node_id } => {
                            log::debug!("AT-SPI2: Node created {:?}", node_id);
                        }
                        AriaUpdate::NodeUpdated { node_id } => {
                            log::debug!("AT-SPI2: Node updated {:?}", node_id);
                        }
                        AriaUpdate::NodeRemoved { node_id } => {
                            log::debug!("AT-SPI2: Node removed {:?}", node_id);
                        }
                        AriaUpdate::FocusChanged { node_id } => {
                            log::debug!("AT-SPI2: Focus changed to {:?}", node_id);
                            let _ = self.atspi_manager.set_focus(node_id).await;
                        }
                        AriaUpdate::LiveRegion {
                            content, urgency, ..
                        } => {
                            log::debug!("AT-SPI2: Live region update ({:?}): {}", urgency, content);
                            let _ = self.atspi_manager.announce(content).await;
                        }
                    }
                }
                Ok::<(), AccessibilityError>(())
            })?;
        }

        Ok(())
    }

    fn announce(
        &mut self,
        message: &str,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        if self.runtime.is_none() {
            return Err(AccessibilityError::PlatformUnavailable(
                "Linux accessibility not initialized".to_string(),
            ));
        }

        log::debug!("Linux announce ({:?}): {}", priority, message);

        if let Some(runtime) = &self.runtime {
            runtime.block_on(async {
                self.atspi_manager.announce(message).await.map_err(|e| {
                    AccessibilityError::AnnouncementFailed(format!(
                        "Failed to announce via AT-SPI2: {}",
                        e
                    ))
                })
            })?;
        }

        Ok(())
    }

    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError> {
        if self.runtime.is_none() {
            return Err(AccessibilityError::PlatformUnavailable(
                "Linux accessibility not initialized".to_string(),
            ));
        }

        log::debug!("Linux set focus: {}", element_id);

        // In a full implementation, we would parse element_id to get NodeId
        // For now, just log the focus change

        Ok(())
    }

    fn platform_name(&self) -> &str {
        "Linux (AT-SPI2)"
    }

    fn is_available(&self) -> bool {
        // Check if AT-SPI2 is available (D-Bus connection established)
        self.atspi_manager.is_connected()
    }
}

// ============================================================================
// Web ARIA Implementation
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub struct WebAccessibility {
    initialized: bool,
    aria_tree: Option<AriaTree>,
    dom_overlay: Option<crate::accessibility::web_overlay::WebDomOverlay>,
}

#[cfg(target_arch = "wasm32")]
impl WebAccessibility {
    pub fn new() -> Self {
        Self {
            initialized: false,
            aria_tree: None,
            dom_overlay: None,
        }
    }

    fn aria_role_to_string(role: &crate::accessibility::aria::AriaRole) -> &'static str {
        use crate::accessibility::aria::AriaRole;
        match role {
            AriaRole::Chart => "figure",
            AriaRole::ChartSeries => "group",
            AriaRole::DataPoint => "listitem",
            AriaRole::Legend => "list",
            AriaRole::Axis => "group",
            AriaRole::Tooltip => "tooltip",
            AriaRole::Control => "button",
        }
    }

    /// Store a reference to the AriaTree for node lookups.
    pub fn set_aria_tree(&mut self, tree: AriaTree) {
        self.aria_tree = Some(tree);
    }
}

#[cfg(target_arch = "wasm32")]
impl PlatformAccessibility for WebAccessibility {
    fn initialize(&mut self) -> Result<(), AccessibilityError> {
        // Web accessibility is always available
        self.initialized = true;

        // Initialize DOM overlay
        let mut overlay = crate::accessibility::web_overlay::WebDomOverlay::new()?;
        overlay.initialize()?;
        self.dom_overlay = Some(overlay);

        Ok(())
    }

    fn update_accessibility_tree(
        &mut self,
        updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError> {
        use wasm_bindgen::JsCast;

        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "Web accessibility not initialized".to_string(),
            ));
        }

        // Update DOM overlay if available
        if let Some(overlay) = &mut self.dom_overlay {
            if let Some(aria_tree) = &self.aria_tree {
                overlay.update_from_aria_tree(updates, aria_tree)?;
            }
        }

        let window = web_sys::window().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No window object".to_string())
        })?;

        let document = window.document().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No document object".to_string())
        })?;

        // Process ARIA updates for hidden accessibility layer
        for update in updates {
            match update {
                AriaUpdate::NodeCreated { node_id } | AriaUpdate::NodeUpdated { node_id } => {
                    // Get node from stored tree
                    let node = self
                        .aria_tree
                        .as_ref()
                        .and_then(|tree| tree.get_node(*node_id))
                        .ok_or_else(|| {
                            AccessibilityError::TreeUpdateFailed(format!(
                                "Node not found: {:?}",
                                node_id
                            ))
                        })?;

                    // Create element ID from node ID
                    let element_id = format!("aria-node-{}", node_id.as_u64());

                    // Get or create DOM element for this ARIA node
                    let element = document
                        .get_element_by_id(&element_id)
                        .or_else(|| {
                            // Create new element
                            let el = document.create_element("div").ok()?;
                            el.set_id(&element_id);
                            Some(el)
                        })
                        .ok_or_else(|| {
                            AccessibilityError::TreeUpdateFailed(format!(
                                "Failed to create element for {}",
                                element_id
                            ))
                        })?;

                    // Set ARIA role
                    element
                        .set_attribute("role", Self::aria_role_to_string(&node.role))
                        .map_err(|_| {
                            AccessibilityError::TreeUpdateFailed(format!(
                                "Failed to set role for {}",
                                element_id
                            ))
                        })?;

                    // Set ARIA label
                    element
                        .set_attribute("aria-label", &node.label)
                        .map_err(|_| {
                            AccessibilityError::TreeUpdateFailed(format!(
                                "Failed to set label for {}",
                                element_id
                            ))
                        })?;

                    // Set ARIA description
                    if let Some(description) = &node.description {
                        element
                            .set_attribute("aria-description", description)
                            .map_err(|_| {
                                AccessibilityError::TreeUpdateFailed(format!(
                                    "Failed to set description for {}",
                                    element_id
                                ))
                            })?;
                    }
                }
                AriaUpdate::NodeRemoved { node_id } => {
                    // Remove DOM element
                    let element_id = format!("aria-node-{}", node_id.as_u64());
                    if let Some(element) = document.get_element_by_id(&element_id) {
                        element.remove();
                    }
                }
                AriaUpdate::FocusChanged { .. } | AriaUpdate::LiveRegion { .. } => {
                    // These are handled by other methods
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
        use wasm_bindgen::JsCast;

        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "Web accessibility not initialized".to_string(),
            ));
        }

        let window = web_sys::window().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No window object".to_string())
        })?;

        let document = window.document().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No document object".to_string())
        })?;

        // Get or create live region
        let live_region = document
            .get_element_by_id("gup-live-region")
            .or_else(|| {
                let el = document.create_element("div").ok()?;
                el.set_id("gup-live-region");
                el.set_attribute("role", "status").ok()?;

                // Set aria-live based on priority
                let aria_live = match priority {
                    AnnouncementPriority::Polite => "polite",
                    AnnouncementPriority::Assertive => "assertive",
                    AnnouncementPriority::Off => "off",
                };
                el.set_attribute("aria-live", aria_live).ok()?;

                // Hide visually but keep for screen readers
                el.set_attribute(
                    "style",
                    "position: absolute; left: -10000px; width: 1px; height: 1px; overflow: hidden;",
                )
                .ok()?;

                document.body()?.append_child(&el).ok()?;
                Some(el)
            })
            .ok_or_else(|| {
                AccessibilityError::AnnouncementFailed("Failed to create live region".to_string())
            })?;

        // Update live region text
        live_region.set_text_content(Some(message));

        Ok(())
    }

    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError> {
        use wasm_bindgen::JsCast;

        if !self.initialized {
            return Err(AccessibilityError::PlatformUnavailable(
                "Web accessibility not initialized".to_string(),
            ));
        }

        let window = web_sys::window().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No window object".to_string())
        })?;

        let document = window.document().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No document object".to_string())
        })?;

        let element = document.get_element_by_id(element_id).ok_or_else(|| {
            AccessibilityError::FocusFailed(format!("Element not found: {}", element_id))
        })?;

        // Cast to HTMLElement to access focus method
        let html_element = element.dyn_ref::<web_sys::HtmlElement>().ok_or_else(|| {
            AccessibilityError::FocusFailed(format!("Element is not focusable: {}", element_id))
        })?;

        html_element.focus().map_err(|_| {
            AccessibilityError::FocusFailed(format!("Failed to focus element: {}", element_id))
        })?;

        Ok(())
    }

    fn platform_name(&self) -> &str {
        "Web (ARIA)"
    }

    fn is_available(&self) -> bool {
        // Web ARIA is always available in browsers
        web_sys::window().is_some()
    }
}

// ============================================================================
// Null Implementation (for unsupported platforms)
// ============================================================================

/// Null implementation that does nothing.
/// Used for platforms without native accessibility support.
pub struct NullAccessibility;

impl NullAccessibility {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAccessibility for NullAccessibility {
    fn initialize(&mut self) -> Result<(), AccessibilityError> {
        Ok(())
    }

    fn update_accessibility_tree(
        &mut self,
        _updates: &[AriaUpdate],
    ) -> Result<(), AccessibilityError> {
        Ok(())
    }

    fn announce(
        &mut self,
        message: &str,
        _priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        log::debug!("Null platform announce: {}", message);
        Ok(())
    }

    fn set_focus(&mut self, element_id: &str) -> Result<(), AccessibilityError> {
        log::debug!("Null platform set focus: {}", element_id);
        Ok(())
    }

    fn platform_name(&self) -> &str {
        "Null (No platform support)"
    }

    fn is_available(&self) -> bool {
        false
    }
}

impl Default for NullAccessibility {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_platform_accessibility() {
        let platform = create_platform_accessibility();
        assert!(!platform.platform_name().is_empty());
    }

    #[test]
    fn test_announcement_priority() {
        assert_eq!(AnnouncementPriority::Polite, AnnouncementPriority::Polite);
        assert_ne!(
            AnnouncementPriority::Polite,
            AnnouncementPriority::Assertive
        );
    }

    #[test]
    fn test_accessibility_error_display() {
        let err = AccessibilityError::PlatformUnavailable("test".to_string());
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_null_accessibility() {
        let mut null = NullAccessibility::new();
        assert!(null.initialize().is_ok());
        assert!(!null.is_available());
        assert!(null.announce("test", AnnouncementPriority::Polite).is_ok());
        assert!(null.set_focus("test-id").is_ok());
    }
}
