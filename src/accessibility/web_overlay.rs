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
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, prelude::*};
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element, HtmlElement, KeyboardEvent, PointerEvent, TouchEvent, Window};

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
    /// Forward events to visualization
    pub forward_events: bool,
    /// Prevent duplicate events (when both overlay and canvas emit events)
    pub deduplicate_events: bool,
}

/// Event data forwarded from DOM overlay to visualization
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct DomInteractionEvent {
    /// Event type: "pointerdown", "pointermove", "pointerup", etc.
    pub event_type: String,
    /// Screen coordinates (client X/Y from the DOM event)
    pub screen_x: f32,
    pub screen_y: f32,
    /// Canvas-relative coordinates (accounting for canvas position)
    pub canvas_x: f32,
    pub canvas_y: f32,
    /// Pointer type: "mouse", "pen", "touch"
    pub pointer_type: String,
    /// Pointer ID for multi-touch tracking
    pub pointer_id: i32,
    /// Button state for pointer events
    pub button: i16,
    /// Timestamp of the event
    pub timestamp: f64,
}

/// Callback type for forwarding events to the visualization
#[cfg(target_arch = "wasm32")]
pub type EventForwardCallback = Rc<RefCell<dyn FnMut(DomInteractionEvent)>>;

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
            forward_events: true,
            deduplicate_events: true,
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
    /// Touch event handlers
    touch_handlers: Vec<Closure<dyn FnMut(TouchEvent)>>,
    /// Position manager for coordinate synchronization
    position_manager: PositionManager,
    /// Animation frame ID for position updates
    animation_frame_id: Option<i32>,
    /// Callback for forwarding events to visualization
    event_forward_callback: Option<EventForwardCallback>,
    /// Track last event timestamp for deduplication
    last_event_timestamp: f64,
    /// Track last event coordinates for deduplication
    last_event_coords: (f32, f32),
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
            touch_handlers: Vec::new(),
            position_manager: PositionManager::new(),
            animation_frame_id: None,
            event_forward_callback: None,
            last_event_timestamp: 0.0,
            last_event_coords: (0.0, 0.0),
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
            self.setup_touch_events()?;
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

        // Create a shared state for the handlers
        let self_rc = Rc::new(RefCell::new(()));

        // Add pointerdown handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_down = Closure::wrap(Box::new(move |event: PointerEvent| {
            Self::handle_pointer_event_static(
                &event,
                "pointerdown",
                &forward_cb,
                &config,
                &document,
            );
        }) as Box<dyn FnMut(PointerEvent)>);

        container
            .add_event_listener_with_callback("pointerdown", handler_down.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add pointerdown listener".to_string())
            })?;
        self.pointer_handlers.push(handler_down);

        // Add pointermove handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_move = Closure::wrap(Box::new(move |event: PointerEvent| {
            Self::handle_pointer_event_static(
                &event,
                "pointermove",
                &forward_cb,
                &config,
                &document,
            );
        }) as Box<dyn FnMut(PointerEvent)>);

        container
            .add_event_listener_with_callback("pointermove", handler_move.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add pointermove listener".to_string())
            })?;
        self.pointer_handlers.push(handler_move);

        // Add pointerup handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_up = Closure::wrap(Box::new(move |event: PointerEvent| {
            Self::handle_pointer_event_static(&event, "pointerup", &forward_cb, &config, &document);
        }) as Box<dyn FnMut(PointerEvent)>);

        container
            .add_event_listener_with_callback("pointerup", handler_up.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add pointerup listener".to_string())
            })?;
        self.pointer_handlers.push(handler_up);

        // Add pointerenter handler (for hover)
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_enter = Closure::wrap(Box::new(move |event: PointerEvent| {
            Self::handle_pointer_event_static(
                &event,
                "pointerenter",
                &forward_cb,
                &config,
                &document,
            );
        }) as Box<dyn FnMut(PointerEvent)>);

        container
            .add_event_listener_with_callback(
                "pointerenter",
                handler_enter.as_ref().unchecked_ref(),
            )
            .map_err(|_| {
                AccessibilityError::Other("Failed to add pointerenter listener".to_string())
            })?;
        self.pointer_handlers.push(handler_enter);

        // Add pointerleave handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_leave = Closure::wrap(Box::new(move |event: PointerEvent| {
            Self::handle_pointer_event_static(
                &event,
                "pointerleave",
                &forward_cb,
                &config,
                &document,
            );
        }) as Box<dyn FnMut(PointerEvent)>);

        container
            .add_event_listener_with_callback(
                "pointerleave",
                handler_leave.as_ref().unchecked_ref(),
            )
            .map_err(|_| {
                AccessibilityError::Other("Failed to add pointerleave listener".to_string())
            })?;
        self.pointer_handlers.push(handler_leave);

        Ok(())
    }

    /// Handle pointer events (static version for closures)
    fn handle_pointer_event_static(
        event: &PointerEvent,
        event_type: &str,
        forward_cb: &Option<EventForwardCallback>,
        config: &DomOverlayConfig,
        document: &Document,
    ) {
        log::debug!(
            "{}: ({}, {}), type: {}",
            event_type,
            event.client_x(),
            event.client_y(),
            event.pointer_type()
        );

        // Forward event if callback is set and forwarding is enabled
        if config.forward_events {
            if let Some(callback) = forward_cb {
                let client_x = event.client_x() as f32;
                let client_y = event.client_y() as f32;

                // Map to canvas coordinates
                let (canvas_x, canvas_y) =
                    if let Some(canvas) = document.get_element_by_id(&config.canvas_id) {
                        let rect = canvas.get_bounding_client_rect();
                        (client_x - rect.left() as f32, client_y - rect.top() as f32)
                    } else {
                        (client_x, client_y)
                    };

                let dom_event = DomInteractionEvent {
                    event_type: event_type.to_string(),
                    screen_x: client_x,
                    screen_y: client_y,
                    canvas_x,
                    canvas_y,
                    pointer_type: event.pointer_type(),
                    pointer_id: event.pointer_id(),
                    button: event.button(),
                    timestamp: event.time_stamp(),
                };

                if let Ok(mut cb) = callback.try_borrow_mut() {
                    cb(dom_event);
                }
            }
        }
    }

    /// Set up touch event handlers
    fn setup_touch_events(&mut self) -> Result<(), AccessibilityError> {
        let container = self
            .container
            .as_ref()
            .ok_or_else(|| AccessibilityError::Other("Container not initialized".to_string()))?;

        // Add touchstart handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_start = Closure::wrap(Box::new(move |event: TouchEvent| {
            Self::handle_touch_event_static(&event, "touchstart", &forward_cb, &config, &document);
        }) as Box<dyn FnMut(TouchEvent)>);

        container
            .add_event_listener_with_callback("touchstart", handler_start.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add touchstart listener".to_string())
            })?;
        self.touch_handlers.push(handler_start);

        // Add touchmove handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_move = Closure::wrap(Box::new(move |event: TouchEvent| {
            Self::handle_touch_event_static(&event, "touchmove", &forward_cb, &config, &document);
        }) as Box<dyn FnMut(TouchEvent)>);

        container
            .add_event_listener_with_callback("touchmove", handler_move.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add touchmove listener".to_string())
            })?;
        self.touch_handlers.push(handler_move);

        // Add touchend handler
        let forward_cb = self.event_forward_callback.clone();
        let config = self.config.clone();
        let document = self.document.clone();
        let handler_end = Closure::wrap(Box::new(move |event: TouchEvent| {
            Self::handle_touch_event_static(&event, "touchend", &forward_cb, &config, &document);
        }) as Box<dyn FnMut(TouchEvent)>);

        container
            .add_event_listener_with_callback("touchend", handler_end.as_ref().unchecked_ref())
            .map_err(|_| {
                AccessibilityError::Other("Failed to add touchend listener".to_string())
            })?;
        self.touch_handlers.push(handler_end);

        Ok(())
    }

    /// Handle touch events (static version for closures)
    fn handle_touch_event_static(
        event: &TouchEvent,
        event_type: &str,
        forward_cb: &Option<EventForwardCallback>,
        config: &DomOverlayConfig,
        document: &Document,
    ) {
        // Get the first touch point
        let touches = event.touches();
        if touches.length() == 0 {
            return;
        }

        let touch: web_sys::Touch = match touches.get(0) {
            Some(t) => t,
            None => return,
        };

        log::debug!(
            "{}: ({}, {}), identifier: {}",
            event_type,
            touch.client_x(),
            touch.client_y(),
            touch.identifier()
        );

        // Forward event if callback is set and forwarding is enabled
        if config.forward_events {
            if let Some(callback) = forward_cb {
                let client_x = touch.client_x() as f32;
                let client_y = touch.client_y() as f32;

                // Map to canvas coordinates
                let (canvas_x, canvas_y) =
                    if let Some(canvas) = document.get_element_by_id(&config.canvas_id) {
                        let rect = canvas.get_bounding_client_rect();
                        (client_x - rect.left() as f32, client_y - rect.top() as f32)
                    } else {
                        (client_x, client_y)
                    };

                let dom_event = DomInteractionEvent {
                    event_type: event_type.to_string(),
                    screen_x: client_x,
                    screen_y: client_y,
                    canvas_x,
                    canvas_y,
                    pointer_type: "touch".to_string(),
                    pointer_id: touch.identifier(),
                    button: 0,
                    timestamp: event.time_stamp(),
                };

                if let Ok(mut cb) = callback.try_borrow_mut() {
                    cb(dom_event);
                }
            }
        }
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
            .clone()
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

                    self.create_or_update_element(&container, *node_id, node)?;
                }
                AriaUpdate::NodeRemoved { node_id } => {
                    self.remove_element(*node_id)?;
                }
                AriaUpdate::FocusChanged { node_id } => {
                    self.set_focus(*node_id)?;
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
                AriaRole::Tooltip => "tooltip",
                AriaRole::Control => "control",
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
            AriaRole::Tooltip => "tooltip",
            AriaRole::Control => "button",
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

    /// Set the event forward callback
    ///
    /// This callback will be called for all pointer and touch events if event forwarding is enabled
    pub fn set_event_forward_callback<F>(&mut self, callback: F)
    where
        F: FnMut(DomInteractionEvent) + 'static,
    {
        self.event_forward_callback = Some(Rc::new(RefCell::new(callback)));
    }

    /// Map DOM coordinates to canvas coordinates
    ///
    /// This accounts for the canvas position within the page and any transformations
    fn map_to_canvas_coords(
        &self,
        client_x: f32,
        client_y: f32,
    ) -> Result<(f32, f32), AccessibilityError> {
        // Get canvas element
        let canvas = self
            .document
            .get_element_by_id(&self.config.canvas_id)
            .ok_or_else(|| AccessibilityError::Other("Canvas not found".to_string()))?;

        // Get canvas bounding rect
        let rect = canvas.get_bounding_client_rect();

        // Calculate canvas-relative coordinates
        let canvas_x = client_x - rect.left() as f32;
        let canvas_y = client_y - rect.top() as f32;

        Ok((canvas_x, canvas_y))
    }

    /// Check if an event should be deduplicated
    ///
    /// Returns true if this event is likely a duplicate of the last event
    fn should_deduplicate_event(&self, x: f32, y: f32, timestamp: f64) -> bool {
        if !self.config.deduplicate_events {
            return false;
        }

        // Check if event is within 50ms and at same coordinates
        let time_diff = (timestamp - self.last_event_timestamp).abs();
        let coord_diff =
            ((x - self.last_event_coords.0).abs() + (y - self.last_event_coords.1).abs());

        time_diff < 50.0 && coord_diff < 1.0
    }

    /// Update event deduplication tracking
    fn update_event_tracking(&mut self, x: f32, y: f32, timestamp: f64) {
        self.last_event_timestamp = timestamp;
        self.last_event_coords = (x, y);
    }

    /// Forward a pointer event to the visualization
    fn forward_pointer_event(&mut self, event: &PointerEvent, event_type: &str) {
        if !self.config.forward_events {
            return;
        }

        let callback = match &self.event_forward_callback {
            Some(cb) => cb.clone(),
            None => return,
        };

        let client_x = event.client_x() as f32;
        let client_y = event.client_y() as f32;
        let timestamp = event.time_stamp();

        // Check for duplicate
        if self.should_deduplicate_event(client_x, client_y, timestamp) {
            log::debug!(
                "Deduplicated event at ({}, {}) within 50ms",
                client_x,
                client_y
            );
            return;
        }

        // Map to canvas coordinates
        let (canvas_x, canvas_y) = match self.map_to_canvas_coords(client_x, client_y) {
            Ok(coords) => coords,
            Err(e) => {
                log::warn!("Failed to map coordinates: {:?}", e);
                return;
            }
        };

        // Update tracking
        self.update_event_tracking(client_x, client_y, timestamp);

        // Create event data
        let dom_event = DomInteractionEvent {
            event_type: event_type.to_string(),
            screen_x: client_x,
            screen_y: client_y,
            canvas_x,
            canvas_y,
            pointer_type: event.pointer_type(),
            pointer_id: event.pointer_id(),
            button: event.button(),
            timestamp,
        };

        // Forward to callback
        if let Ok(mut cb) = callback.try_borrow_mut() {
            cb(dom_event);
        } else {
            log::warn!("Event forward callback is already borrowed");
        }
    }

    /// Forward a touch event to the visualization
    fn forward_touch_event(&mut self, event: &TouchEvent, event_type: &str) {
        if !self.config.forward_events {
            return;
        }

        let callback = match &self.event_forward_callback {
            Some(cb) => cb.clone(),
            None => return,
        };

        // Get the first touch point
        let touches = event.touches();
        if touches.length() == 0 {
            return;
        }

        let touch: web_sys::Touch = match touches.get(0) {
            Some(t) => t,
            None => return,
        };

        let client_x = touch.client_x() as f32;
        let client_y = touch.client_y() as f32;
        let timestamp = event.time_stamp();

        // Check for duplicate
        if self.should_deduplicate_event(client_x, client_y, timestamp) {
            log::debug!("Deduplicated touch event at ({}, {})", client_x, client_y);
            return;
        }

        // Map to canvas coordinates
        let (canvas_x, canvas_y) = match self.map_to_canvas_coords(client_x, client_y) {
            Ok(coords) => coords,
            Err(e) => {
                log::warn!("Failed to map touch coordinates: {:?}", e);
                return;
            }
        };

        // Update tracking
        self.update_event_tracking(client_x, client_y, timestamp);

        // Create event data
        let dom_event = DomInteractionEvent {
            event_type: event_type.to_string(),
            screen_x: client_x,
            screen_y: client_y,
            canvas_x,
            canvas_y,
            pointer_type: "touch".to_string(),
            pointer_id: touch.identifier(),
            button: 0,
            timestamp,
        };

        // Forward to callback
        if let Ok(mut cb) = callback.try_borrow_mut() {
            cb(dom_event);
        } else {
            log::warn!("Touch event forward callback is already borrowed");
        }
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
        self.touch_handlers.clear();
        self.element_map.clear();
        self.container = None;
        self.event_forward_callback = None;
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
            forward_events: false,
            deduplicate_events: false,
        };

        assert_eq!(config.container_id, "custom-overlay");
        assert!(!config.keyboard_enabled);
        assert_eq!(config.z_index, 500);
    }
}
