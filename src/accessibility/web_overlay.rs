// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Web DOM Overlay for Accessibility
//!
//! This module provides a visible DOM overlay above the WebGL canvas that enables:
//! - Full keyboard navigation
//! - Touch and pointer event handling
//! - Visible focus indicators
//! - Interactive accessibility features
//!
//! The overlay is synchronized with the visualization state and provides
//! native web interactions for users with disabilities.

#[cfg(target_arch = "wasm32")]
use crate::accessibility::aria::{AriaNode, AriaRole, AriaTree, AriaUpdate, NodeId};
#[cfg(target_arch = "wasm32")]
use crate::accessibility::platform::AccessibilityError;
#[cfg(target_arch = "wasm32")]
use crate::accessibility::position_sync::{GpuPosition, PositionManager, ScreenPosition};
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::*, JsCast};
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element, HtmlElement, KeyboardEvent, PointerEvent, Window};

/// Configuration for the DOM overlay
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct DomOverlayConfig {
    /// Container ID for the overlay (defaults to "gup-overlay")
    pub container_id: String,
    /// Canvas ID to overlay above
    pub canvas_id: String,
    /// Enable keyboard shortcuts
    pub keyboard_enabled: bool,
    /// Enable touch/pointer events
    pub pointer_enabled: bool,
    /// Show visible focus indicators
    pub show_focus_indicators: bool,
    /// Z-index for overlay positioning
    pub z_index: i32,
}

#[cfg(target_arch = "wasm32")]
impl Default for DomOverlayConfig {
    fn default() -> Self {
        Self {
            container_id: "gup-overlay".to_string(),
            canvas_id: "gup-canvas".to_string(),
            keyboard_enabled: true,
            pointer_enabled: true,
            show_focus_indicators: true,
            z_index: 1000,
        }
    }
}

/// DOM overlay manager for web accessibility
#[cfg(target_arch = "wasm32")]
pub struct WebDomOverlay {
    config: DomOverlayConfig,
    window: Window,
    document: Document,
    container: Option<Element>,
    /// Map of ARIA node IDs to DOM elements
    element_map: HashMap<NodeId, Element>,
    /// Currently focused element
    focused_element: Option<NodeId>,
    /// Keyboard event handlers
    keyboard_handlers: Vec<Closure<dyn FnMut(KeyboardEvent)>>,
    /// Pointer event handlers
    pointer_handlers: Vec<Closure<dyn FnMut(PointerEvent)>>,
    /// Position manager for coordinate synchronization
    position_manager: PositionManager,
    /// Animation frame ID for position updates
    animation_frame_id: Option<i32>,
}

#[cfg(target_arch = "wasm32")]
impl WebDomOverlay {
    /// Create a new web DOM overlay with default config
    pub fn new() -> Result<Self, AccessibilityError> {
        Self::with_config(DomOverlayConfig::default())
    }

    /// Create a new web DOM overlay with custom config
    pub fn with_config(config: DomOverlayConfig) -> Result<Self, AccessibilityError> {
        let window = web_sys::window().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No window object".to_string())
        })?;

        let document = window.document().ok_or_else(|| {
            AccessibilityError::PlatformUnavailable("No document object".to_string())
        })?;

        Ok(Self {
            config,
            window,
            document,
            container: None,
            element_map: HashMap::new(),
            focused_element: None,
            keyboard_handlers: Vec::new(),
            pointer_handlers: Vec::new(),
            position_manager: PositionManager::new(),
            animation_frame_id: None,
        })
    }

    /// Initialize the DOM overlay structure
    pub fn initialize(&mut self) -> Result<(), AccessibilityError> {
        // Create overlay container
        let container = self.document.create_element("div").map_err(|_| {
            AccessibilityError::Other("Failed to create overlay container".to_string())
        })?;

        container.set_id(&self.config.container_id);
        container.set_class_name("gup-accessibility-overlay");

        // Position overlay absolutely above canvas
        let style = format!(
            "position: absolute; top: 0; left: 0; width: 100%; height: 100%; \
             pointer-events: none; z-index: {}; overflow: hidden;",
            self.config.z_index
        );
        container
            .set_attribute("style", &style)
            .map_err(|_| AccessibilityError::Other("Failed to set overlay style".to_string()))?;

        // Append to body or canvas parent
        if let Some(canvas) = self.document.get_element_by_id(&self.config.canvas_id) {
            if let Some(parent) = canvas.parent_element() {
                // Ensure parent has position relative/absolute
                if let Some(html_parent) = parent.dyn_ref::<HtmlElement>() {
                    let parent_style = html_parent.style();
                    if parent_style
                        .get_property_value("position")
                        .unwrap_or_default()
                        .is_empty()
                    {
                        let _ = parent_style.set_property("position", "relative");
                    }
                }
                parent.append_child(&container).map_err(|_| {
                    AccessibilityError::Other("Failed to append overlay to parent".to_string())
                })?;
            } else {
                self.document
                    .body()
                    .ok_or_else(|| {
                        AccessibilityError::PlatformUnavailable("No body element".to_string())
                    })?
                    .append_child(&container)
                    .map_err(|_| {
                        AccessibilityError::Other("Failed to append overlay to body".to_string())
                    })?;
            }
        } else {
            // Canvas not found, append to body
            self.document
                .body()
                .ok_or_else(|| {
                    AccessibilityError::PlatformUnavailable("No body element".to_string())
                })?
                .append_child(&container)
                .map_err(|_| {
                    AccessibilityError::Other("Failed to append overlay to body".to_string())
                })?;
        }

        self.container = Some(container.clone());

        // Set up keyboard navigation
        if self.config.keyboard_enabled {
            self.setup_keyboard_navigation()?;
        }

        // Set up pointer events
        if self.config.pointer_enabled {
            self.setup_pointer_events()?;
        }

        // Add CSS for focus indicators
        if self.config.show_focus_indicators {
            self.inject_focus_styles()?;
        }

        Ok(())
    }

    /// Set up keyboard navigation handlers
    fn setup_keyboard_navigation(&mut self) -> Result<(), AccessibilityError> {
        let container = self
            .container
            .as_ref()
            .ok_or_else(|| AccessibilityError::Other("Container not initialized".to_string()))?;

        // Make container focusable
        container.set_attribute("tabindex", "0").map_err(|_| {
            AccessibilityError::Other("Failed to make container focusable".to_string())
        })?;

        // Add keyboard event listener
        let handler = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            Self::handle_keyboard_event(event);
        }) as Box<dyn FnMut(KeyboardEvent)>);

        container
            .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add keyboard listener".to_string())
            })?;

        self.keyboard_handlers.push(handler);

        Ok(())
    }

    /// Handle keyboard events for navigation
    fn handle_keyboard_event(event: KeyboardEvent) {
        let key = event.key();
        log::debug!("Keyboard event: {}", key);

        match key.as_str() {
            "Tab" => {
                // Let browser handle tab navigation
                // Focus will naturally move through focusable overlay elements
            }
            "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" => {
                // Arrow key navigation within chart
                // TODO: Implement directional navigation through data points
                event.prevent_default();
            }
            "Enter" | " " => {
                // Activate/select focused element
                // TODO: Trigger selection event
                event.prevent_default();
            }
            "Escape" => {
                // Cancel or go up hierarchy
                // TODO: Implement escape behavior
            }
            _ => {
                // Other keys - may be used for shortcuts
            }
        }
    }

    /// Set up pointer event handlers
    fn setup_pointer_events(&mut self) -> Result<(), AccessibilityError> {
        let container = self
            .container
            .as_ref()
            .ok_or_else(|| AccessibilityError::Other("Container not initialized".to_string()))?;

        // Add pointer event listener
        let handler = Closure::wrap(Box::new(move |event: PointerEvent| {
            Self::handle_pointer_event(event);
        }) as Box<dyn FnMut(PointerEvent)>);

        container
            .add_event_listener_with_callback("pointerdown", handler.as_ref().unchecked_ref())
            .map_err(|_| AccessibilityError::Other("Failed to add pointer listener".to_string()))?;

        self.pointer_handlers.push(handler);

        Ok(())
    }

    /// Handle pointer events
    fn handle_pointer_event(event: PointerEvent) {
        log::debug!(
            "Pointer event: ({}, {}), type: {}",
            event.client_x(),
            event.client_y(),
            event.pointer_type()
        );

        // TODO: Forward pointer events to visualization
        // For now, just log
    }

    /// Inject CSS for focus indicators
    fn inject_focus_styles(&self) -> Result<(), AccessibilityError> {
        // Check if styles already exist
        if self
            .document
            .get_element_by_id("gup-focus-styles")
            .is_some()
        {
            return Ok(());
        }

        let style = self
            .document
            .create_element("style")
            .map_err(|_| AccessibilityError::Other("Failed to create style element".to_string()))?;

        style.set_id("gup-focus-styles");

        let css = r#"
.gup-accessibility-overlay .focusable {
    pointer-events: auto;
    cursor: pointer;
}

.gup-accessibility-overlay .focusable:focus {
    outline: 3px solid #4A90E2;
    outline-offset: 2px;
    box-shadow: 0 0 0 4px rgba(74, 144, 226, 0.2);
}

.gup-accessibility-overlay .focusable:focus-visible {
    outline: 3px solid #4A90E2;
    outline-offset: 2px;
}

.gup-accessibility-overlay .data-point {
    position: absolute;
    display: block;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: transparent;
    border: 2px solid transparent;
}

.gup-accessibility-overlay .data-point:focus {
    border-color: #4A90E2;
    background: rgba(74, 144, 226, 0.1);
}

/* High contrast mode support */
@media (prefers-contrast: high) {
    .gup-accessibility-overlay .focusable:focus {
        outline-width: 4px;
        outline-offset: 3px;
    }
}

/* Reduced motion support */
@media (prefers-reduced-motion: reduce) {
    .gup-accessibility-overlay .focusable {
        transition: none;
    }
}
"#;

        style.set_text_content(Some(css));

        self.document
            .head()
            .ok_or_else(|| AccessibilityError::PlatformUnavailable("No head element".to_string()))?
            .append_child(&style)
            .map_err(|_| AccessibilityError::Other("Failed to append styles".to_string()))?;

        Ok(())
    }

    /// Update overlay with ARIA tree changes
    pub fn update_from_aria_tree(
        &mut self,
        updates: &[AriaUpdate],
        aria_tree: &AriaTree,
    ) -> Result<(), AccessibilityError> {
        let container = self
            .container
            .as_ref()
            .ok_or_else(|| AccessibilityError::Other("Container not initialized".to_string()))?;

        for update in updates {
            match update {
                AriaUpdate::NodeCreated { node_id } | AriaUpdate::NodeUpdated { node_id } => {
                    let node = aria_tree.get_node(*node_id).ok_or_else(|| {
                        AccessibilityError::TreeUpdateFailed(format!(
                            "Node not found: {:?}",
                            node_id
                        ))
                    })?;

                    self.create_or_update_element(container, *node_id, node)?;
                }
                AriaUpdate::NodeRemoved { node_id } => {
                    self.remove_element(*node_id)?;
                }
                AriaUpdate::FocusChanged { node_id } => {
                    if let Some(node_id) = node_id {
                        self.set_focus(*node_id)?;
                    }
                }
                AriaUpdate::LiveRegion { .. } => {
                    // Live regions handled by platform layer
                }
            }
        }

        Ok(())
    }

    /// Create or update a DOM element for an ARIA node
    fn create_or_update_element(
        &mut self,
        container: &Element,
        node_id: NodeId,
        node: &AriaNode,
    ) -> Result<(), AccessibilityError> {
        let element = if let Some(existing) = self.element_map.get(&node_id) {
            existing.clone()
        } else {
            // Create new element
            let el = self.document.create_element("div").map_err(|_| {
                AccessibilityError::TreeUpdateFailed("Failed to create element".to_string())
            })?;

            let element_id = format!("gup-node-{}", node_id.as_u64());
            el.set_id(&element_id);
            el.set_class_name("focusable");

            // Add specific class based on role
            let role_class = match node.role {
                AriaRole::Chart => "chart",
                AriaRole::ChartSeries => "chart-series",
                AriaRole::DataPoint => "data-point",
                AriaRole::Legend => "legend",
                AriaRole::Axis => "axis",
            };
            el.set_class_name(&format!("focusable {}", role_class));

            // Make focusable
            el.set_attribute("tabindex", "0").map_err(|_| {
                AccessibilityError::TreeUpdateFailed("Failed to set tabindex".to_string())
            })?;

            // Append to container
            container.append_child(&el).map_err(|_| {
                AccessibilityError::TreeUpdateFailed("Failed to append element".to_string())
            })?;

            self.element_map.insert(node_id, el.clone());
            el
        };

        // Update ARIA attributes
        element
            .set_attribute("role", Self::aria_role_to_string(&node.role))
            .map_err(|_| AccessibilityError::TreeUpdateFailed("Failed to set role".to_string()))?;

        element
            .set_attribute("aria-label", &node.label)
            .map_err(|_| AccessibilityError::TreeUpdateFailed("Failed to set label".to_string()))?;

        if let Some(description) = &node.description {
            element
                .set_attribute("aria-description", description)
                .map_err(|_| {
                    AccessibilityError::TreeUpdateFailed("Failed to set description".to_string())
                })?;
        }

        // Position element if node has bounds
        // TODO: Get actual position from visualization
        // For now, use a simple layout
        self.position_element(&element, node)?;

        Ok(())
    }

    /// Position an element based on its ARIA node
    fn position_element(
        &self,
        element: &Element,
        node: &AriaNode,
    ) -> Result<(), AccessibilityError> {
        let style = match node.role {
            AriaRole::DataPoint => {
                // Try to get actual position from position manager
                if let Some(screen_pos) = self.position_manager.get_screen_position(node.id) {
                    // Position data point at its actual coordinate
                    // Center the 12px element on the coordinate
                    format!(
                        "position: absolute; left: {}px; top: {}px; transform: translate(-50%, -50%);",
                        screen_pos.x, screen_pos.y
                    )
                } else {
                    // Fallback to placeholder if position not yet set
                    "position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%);"
                        .to_string()
                }
            }
            AriaRole::Chart => {
                // Chart takes full overlay
                "position: absolute; inset: 0;".to_string()
            }
            _ => {
                // Other elements positioned relatively
                "position: relative; display: inline-block;".to_string()
            }
        };

        element.set_attribute("style", &style).map_err(|_| {
            AccessibilityError::TreeUpdateFailed("Failed to set position".to_string())
        })?;

        Ok(())
    }

    /// Remove an element from the overlay
    fn remove_element(&mut self, node_id: NodeId) -> Result<(), AccessibilityError> {
        if let Some(element) = self.element_map.remove(&node_id) {
            element.remove();
        }
        Ok(())
    }

    /// Set focus to a specific element
    fn set_focus(&mut self, node_id: NodeId) -> Result<(), AccessibilityError> {
        let element = self.element_map.get(&node_id).ok_or_else(|| {
            AccessibilityError::FocusFailed(format!("Element not found for node: {:?}", node_id))
        })?;

        let html_element = element.dyn_ref::<HtmlElement>().ok_or_else(|| {
            AccessibilityError::FocusFailed(format!("Element is not focusable: {:?}", node_id))
        })?;

        html_element.focus().map_err(|_| {
            AccessibilityError::FocusFailed(format!("Failed to focus: {:?}", node_id))
        })?;

        self.focused_element = Some(node_id);

        Ok(())
    }

    /// Convert ARIA role to HTML role string
    fn aria_role_to_string(role: &AriaRole) -> &'static str {
        match role {
            AriaRole::Chart => "figure",
            AriaRole::ChartSeries => "group",
            AriaRole::DataPoint => "listitem",
            AriaRole::Legend => "list",
            AriaRole::Axis => "group",
        }
    }

    /// Set the GPU position for a node
    ///
    /// This updates the position manager and schedules an overlay update
    pub fn set_node_position(&mut self, node_id: NodeId, gpu_position: GpuPosition) {
        self.position_manager.set_position(node_id, gpu_position);
        self.schedule_position_update();
    }

    /// Update the viewport dimensions
    ///
    /// Call this when the canvas size changes
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.position_manager.set_viewport(width, height);
        self.schedule_position_update();
    }

    /// Update pan offset
    ///
    /// Call this when the visualization is panned
    pub fn set_pan(&mut self, pan_x: f32, pan_y: f32) {
        self.position_manager.set_pan(pan_x, pan_y);
        self.schedule_position_update();
    }

    /// Update zoom level
    ///
    /// Call this when the visualization is zoomed
    pub fn set_zoom(&mut self, zoom: f32) {
        self.position_manager.set_zoom(zoom);
        self.schedule_position_update();
    }

    /// Schedule a position update on the next animation frame
    fn schedule_position_update(&mut self) {
        // Only schedule if not already scheduled
        if self.animation_frame_id.is_some() {
            return;
        }

        // Use requestAnimationFrame for smooth updates at 60 FPS
        let window = self.window.clone();
        let callback = Closure::once(Box::new(move || {
            // In a real implementation, we would store a reference to self
            // and call update_positions() here
            log::debug!("Position update scheduled");
        }) as Box<dyn FnOnce()>);

        if let Ok(id) = window.request_animation_frame(callback.as_ref().unchecked_ref()) {
            self.animation_frame_id = Some(id);
        }

        // Keep callback alive
        callback.forget();
    }

    /// Update all element positions based on current position manager state
    pub fn update_positions(&mut self, aria_tree: &AriaTree) -> Result<(), AccessibilityError> {
        if !self.position_manager.is_dirty() {
            return Ok(());
        }

        // Update positions for all elements
        for node_id in self.position_manager.node_ids() {
            if let Some(element) = self.element_map.get(node_id) {
                if let Some(node) = aria_tree.get_node(*node_id) {
                    self.position_element(element, node)?;
                }
            }
        }

        self.position_manager.clear_dirty();
        self.animation_frame_id = None;

        Ok(())
    }

    /// Get the current screen position for a node
    pub fn get_screen_position(&self, node_id: NodeId) -> Option<ScreenPosition> {
        self.position_manager.get_screen_position(node_id)
    }

    /// Clean up event handlers
    pub fn cleanup(&mut self) {
        // Remove container
        if let Some(container) = &self.container {
            container.remove();
        }

        // Clear handlers (they will be dropped automatically)
        self.keyboard_handlers.clear();
        self.pointer_handlers.clear();
        self.element_map.clear();
        self.container = None;
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for WebDomOverlay {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct WebDomOverlay;

#[cfg(not(target_arch = "wasm32"))]
impl WebDomOverlay {
    pub fn new() -> Result<Self, crate::accessibility::platform::AccessibilityError> {
        Ok(WebDomOverlay)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use super::DomOverlayConfig;

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_dom_overlay_config() {
        let config = DomOverlayConfig::default();
        assert_eq!(config.container_id, "gup-overlay");
        assert_eq!(config.canvas_id, "gup-canvas");
        assert!(config.keyboard_enabled);
        assert!(config.pointer_enabled);
        assert!(config.show_focus_indicators);
        assert_eq!(config.z_index, 1000);
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_custom_config() {
        let config = DomOverlayConfig {
            container_id: "custom-overlay".to_string(),
            canvas_id: "custom-canvas".to_string(),
            keyboard_enabled: false,
            pointer_enabled: false,
            show_focus_indicators: false,
            z_index: 500,
        };

        assert_eq!(config.container_id, "custom-overlay");
        assert!(!config.keyboard_enabled);
        assert_eq!(config.z_index, 500);
    }
}
