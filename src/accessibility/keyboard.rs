// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard navigation and focus management for accessibility.
//!
//! This module provides comprehensive keyboard support for navigating
//! and interacting with visualizations.

use crate::interaction::Rect;

/// Unique identifier for focusable elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(u64);

impl ElementId {
    /// Create a new unique element ID.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the numeric ID.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for ElementId {
    fn default() -> Self {
        Self::new()
    }
}

/// Focus manager for keyboard navigation.
#[derive(Debug)]
pub struct FocusManager {
    /// All focusable elements
    focusable_elements: Vec<FocusableElement>,

    /// Index of currently focused element
    current_focus: Option<usize>,

    /// History of focused elements
    focus_history: Vec<usize>,

    /// Current navigation mode
    navigation_mode: NavigationMode,
}

impl FocusManager {
    /// Create a new focus manager.
    pub fn new() -> Self {
        Self {
            focusable_elements: Vec::new(),
            current_focus: None,
            focus_history: Vec::new(),
            navigation_mode: NavigationMode::Sequential,
        }
    }

    /// Add a focusable element.
    pub fn add_focusable_element(&mut self, element: FocusableElement) {
        self.focusable_elements.push(element);
    }

    /// Add multiple focusable elements.
    pub fn add_focusable_elements(&mut self, elements: Vec<FocusableElement>) {
        self.focusable_elements.extend(elements);
    }

    /// Clear all focusable elements.
    pub fn clear_focusable_elements(&mut self) {
        self.focusable_elements.clear();
        self.current_focus = None;
        self.focus_history.clear();
    }

    /// Get the currently focused element.
    pub fn get_focused_element(&self) -> Option<&FocusableElement> {
        self.current_focus
            .and_then(|idx| self.focusable_elements.get(idx))
    }

    /// Set focus to a specific element by index.
    pub fn set_focus(&mut self, index: usize) {
        if index < self.focusable_elements.len() {
            if let Some(current) = self.current_focus {
                self.focus_history.push(current);
            }
            self.current_focus = Some(index);
        }
    }

    /// Set the navigation mode.
    pub fn set_navigation_mode(&mut self, mode: NavigationMode) {
        self.navigation_mode = mode;
    }

    /// Get the current navigation mode.
    pub fn navigation_mode(&self) -> &NavigationMode {
        &self.navigation_mode
    }

    /// Move focus sequentially (tab/shift-tab).
    pub fn move_focus_sequential(&mut self, direction: i32) {
        if self.focusable_elements.is_empty() {
            return;
        }

        let new_focus = match self.current_focus {
            None => {
                if direction > 0 {
                    Some(0)
                } else {
                    Some(self.focusable_elements.len() - 1)
                }
            }
            Some(current) => {
                let next = if direction > 0 {
                    (current + 1) % self.focusable_elements.len()
                } else if current == 0 {
                    self.focusable_elements.len() - 1
                } else {
                    current - 1
                };
                Some(next)
            }
        };

        if let Some(idx) = new_focus {
            self.set_focus(idx);
        }
    }

    /// Handle keyboard input.
    pub fn handle_key_input(&mut self, key: KeyEvent) -> Option<AccessibilityAction> {
        match key {
            KeyEvent::Tab => {
                self.move_focus_sequential(1);
                Some(AccessibilityAction::FocusChanged)
            }
            KeyEvent::ShiftTab => {
                self.move_focus_sequential(-1);
                Some(AccessibilityAction::FocusChanged)
            }
            KeyEvent::ArrowRight => {
                match self.navigation_mode {
                    NavigationMode::Spatial => {
                        self.move_focus_spatial(Direction::Right);
                    }
                    NavigationMode::Sequential | NavigationMode::DataDimension => {
                        self.move_focus_sequential(1);
                    }
                }
                Some(AccessibilityAction::FocusChanged)
            }
            KeyEvent::ArrowLeft => {
                match self.navigation_mode {
                    NavigationMode::Spatial => {
                        self.move_focus_spatial(Direction::Left);
                    }
                    NavigationMode::Sequential | NavigationMode::DataDimension => {
                        self.move_focus_sequential(-1);
                    }
                }
                Some(AccessibilityAction::FocusChanged)
            }
            KeyEvent::ArrowUp => match self.navigation_mode {
                NavigationMode::Spatial => {
                    self.move_focus_spatial(Direction::Up);
                    Some(AccessibilityAction::FocusChanged)
                }
                NavigationMode::DataDimension => {
                    Some(AccessibilityAction::DimensionCycleRequested { forward: false })
                }
                NavigationMode::Sequential => None,
            },
            KeyEvent::ArrowDown => match self.navigation_mode {
                NavigationMode::Spatial => {
                    self.move_focus_spatial(Direction::Down);
                    Some(AccessibilityAction::FocusChanged)
                }
                NavigationMode::DataDimension => {
                    Some(AccessibilityAction::DimensionCycleRequested { forward: true })
                }
                NavigationMode::Sequential => None,
            },
            KeyEvent::Enter | KeyEvent::Space => self.activate_current_element(),
            KeyEvent::Escape => {
                self.exit_current_context();
                Some(AccessibilityAction::ContextExited)
            }
        }
    }

    /// Get a description of the currently focused element.
    pub fn describe_current_focus(&self) -> Option<String> {
        self.get_focused_element()
            .map(|element| format!("{}: {}", element.element_type.name(), element.description))
    }

    /// Move focus spatially based on direction.
    fn move_focus_spatial(&mut self, direction: Direction) {
        if self.focusable_elements.is_empty() {
            return;
        }

        let current_index = match self.current_focus {
            Some(idx) => idx,
            None => {
                // No current focus, focus first element
                self.set_focus(0);
                return;
            }
        };

        let current_element = &self.focusable_elements[current_index];
        let current_center = current_element.bounds.center();

        let mut best_candidate: Option<usize> = None;
        let mut best_distance = f32::INFINITY;

        for (i, element) in self.focusable_elements.iter().enumerate() {
            if i == current_index {
                continue;
            }

            let element_center = element.bounds.center();
            if is_in_direction(current_center.into(), element_center.into(), direction) {
                let distance = distance(current_center.into(), element_center.into());
                if distance < best_distance {
                    best_distance = distance;
                    best_candidate = Some(i);
                }
            }
        }

        if let Some(new_focus) = best_candidate {
            self.set_focus(new_focus);
        }
    }

    /// Activate the currently focused element.
    fn activate_current_element(&mut self) -> Option<AccessibilityAction> {
        self.get_focused_element()?;
        Some(AccessibilityAction::ElementActivated)
    }

    /// Exit the current context (e.g., close a modal).
    fn exit_current_context(&mut self) {
        if let Some(previous) = self.focus_history.pop() {
            self.current_focus = Some(previous);
        } else {
            self.current_focus = None;
        }
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A focusable element in the visualization.
#[derive(Debug, Clone)]
pub struct FocusableElement {
    /// Unique identifier
    pub id: ElementId,

    /// Type of element
    pub element_type: ElementType,

    /// Bounds in screen coordinates
    pub bounds: Rect,

    /// Optional data index
    pub data_index: Option<usize>,

    /// Accessible description
    pub description: String,

    /// Available actions
    pub actions: Vec<AccessibilityAction>,
}

impl FocusableElement {
    /// Create a new focusable element.
    pub fn new(element_type: ElementType, bounds: Rect, description: String) -> Self {
        Self {
            id: ElementId::new(),
            element_type,
            bounds,
            data_index: None,
            description,
            actions: Vec::new(),
        }
    }

    /// Set the data index.
    pub fn with_data_index(mut self, index: usize) -> Self {
        self.data_index = Some(index);
        self
    }

    /// Add an action.
    pub fn with_action(mut self, action: AccessibilityAction) -> Self {
        self.actions.push(action);
        self
    }
}

/// Type of focusable element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// A data point
    DataPoint,

    /// An axis
    Axis,

    /// A legend item
    LegendItem,

    /// A control button
    Control,

    /// A tooltip
    Tooltip,
}

impl ElementType {
    /// Get the name of the element type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DataPoint => "Data point",
            Self::Axis => "Axis",
            Self::LegendItem => "Legend item",
            Self::Control => "Control",
            Self::Tooltip => "Tooltip",
        }
    }
}

/// Navigation mode for keyboard interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationMode {
    /// Sequential navigation (Tab/Shift-Tab)
    Sequential,

    /// Spatial navigation (Arrow keys)
    Spatial,

    /// Data-dimension navigation.
    ///
    /// In this mode Arrow Left/Right navigate sequentially through
    /// data points (which are expected to be sorted by the active dimension),
    /// while Arrow Up/Down have no spatial meaning and are reported as
    /// [`AccessibilityAction::DimensionCycleRequested`] so the caller can
    /// switch the active sort dimension.
    DataDimension,
}

/// Keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Tab key
    Tab,

    /// Shift+Tab
    ShiftTab,

    /// Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    /// Action keys
    Enter,
    Space,
    Escape,
}

/// Direction for spatial navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Actions that can be performed for accessibility.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessibilityAction {
    /// Focus changed to a new element
    FocusChanged,

    /// An element was activated
    ElementActivated,

    /// Exited the current context
    ContextExited,

    /// Arrow Up/Down in DataDimension mode requests a dimension cycle.
    ///
    /// `forward: true` means the user pressed Down (advance to next dimension),
    /// `forward: false` means the user pressed Up (return to previous dimension).
    DimensionCycleRequested { forward: bool },
}

/// Helper function to check if a point is in a direction from another point.
fn is_in_direction(from: [f32; 2], to: [f32; 2], direction: Direction) -> bool {
    match direction {
        Direction::Right => to[0] > from[0],
        Direction::Left => to[0] < from[0],
        Direction::Down => to[1] > from[1],
        Direction::Up => to[1] < from[1],
    }
}

/// Helper function to calculate distance between two points.
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::Vec2;

    fn create_test_element(x: f32, y: f32, desc: &str) -> FocusableElement {
        let bounds = Rect::from_center_size(Vec2::new(x, y), Vec2::new(20.0, 20.0));
        FocusableElement::new(ElementType::DataPoint, bounds, desc.to_string())
    }

    #[test]
    fn test_element_id_generation() {
        let id1 = ElementId::new();
        let id2 = ElementId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_focus_manager_creation() {
        let manager = FocusManager::new();
        assert!(manager.get_focused_element().is_none());
        assert_eq!(manager.focusable_elements.len(), 0);
    }

    #[test]
    fn test_sequential_navigation() {
        let mut manager = FocusManager::new();
        manager.add_focusable_element(create_test_element(0.0, 0.0, "A"));
        manager.add_focusable_element(create_test_element(100.0, 0.0, "B"));
        manager.add_focusable_element(create_test_element(200.0, 0.0, "C"));

        // Initially no focus
        assert!(manager.get_focused_element().is_none());

        // Tab to first element
        manager.handle_key_input(KeyEvent::Tab);
        assert_eq!(manager.current_focus, Some(0));

        // Tab to second element
        manager.handle_key_input(KeyEvent::Tab);
        assert_eq!(manager.current_focus, Some(1));

        // Tab to third element
        manager.handle_key_input(KeyEvent::Tab);
        assert_eq!(manager.current_focus, Some(2));

        // Tab wraps around
        manager.handle_key_input(KeyEvent::Tab);
        assert_eq!(manager.current_focus, Some(0));

        // Shift-Tab goes backward
        manager.handle_key_input(KeyEvent::ShiftTab);
        assert_eq!(manager.current_focus, Some(2));
    }

    #[test]
    fn test_spatial_navigation() {
        let mut manager = FocusManager::new();
        manager.set_navigation_mode(NavigationMode::Spatial);

        // Create a grid of elements
        //   A   B   C
        //   D   E   F
        manager.add_focusable_element(create_test_element(0.0, 0.0, "A"));
        manager.add_focusable_element(create_test_element(100.0, 0.0, "B"));
        manager.add_focusable_element(create_test_element(200.0, 0.0, "C"));
        manager.add_focusable_element(create_test_element(0.0, 100.0, "D"));
        manager.add_focusable_element(create_test_element(100.0, 100.0, "E"));
        manager.add_focusable_element(create_test_element(200.0, 100.0, "F"));

        // Start at center (E)
        manager.set_focus(4);
        assert_eq!(manager.current_focus, Some(4));

        // Navigate right to F
        manager.handle_key_input(KeyEvent::ArrowRight);
        assert_eq!(manager.current_focus, Some(5));

        // Navigate up to C
        manager.handle_key_input(KeyEvent::ArrowUp);
        assert_eq!(manager.current_focus, Some(2));

        // Navigate left to B
        manager.handle_key_input(KeyEvent::ArrowLeft);
        assert_eq!(manager.current_focus, Some(1));

        // Navigate down to E
        manager.handle_key_input(KeyEvent::ArrowDown);
        assert_eq!(manager.current_focus, Some(4));
    }

    #[test]
    fn test_focus_description() {
        let mut manager = FocusManager::new();
        manager.add_focusable_element(create_test_element(0.0, 0.0, "Point A"));
        manager.set_focus(0);

        let desc = manager.describe_current_focus();
        assert!(desc.is_some());
        assert!(desc.unwrap().contains("Point A"));
    }

    #[test]
    fn test_element_activation() {
        let mut manager = FocusManager::new();
        manager.add_focusable_element(create_test_element(0.0, 0.0, "Button"));
        manager.set_focus(0);

        let action = manager.handle_key_input(KeyEvent::Enter);
        assert_eq!(action, Some(AccessibilityAction::ElementActivated));

        let action = manager.handle_key_input(KeyEvent::Space);
        assert_eq!(action, Some(AccessibilityAction::ElementActivated));
    }

    #[test]
    fn test_context_exit() {
        let mut manager = FocusManager::new();
        manager.add_focusable_element(create_test_element(0.0, 0.0, "A"));
        manager.add_focusable_element(create_test_element(100.0, 0.0, "B"));

        manager.set_focus(0);
        manager.set_focus(1);

        // Escape should go back to previous focus
        let action = manager.handle_key_input(KeyEvent::Escape);
        assert_eq!(action, Some(AccessibilityAction::ContextExited));
        assert_eq!(manager.current_focus, Some(0));
    }

    #[test]
    fn test_clear_elements() {
        let mut manager = FocusManager::new();
        manager.add_focusable_element(create_test_element(0.0, 0.0, "A"));
        manager.set_focus(0);

        manager.clear_focusable_elements();
        assert_eq!(manager.focusable_elements.len(), 0);
        assert!(manager.get_focused_element().is_none());
    }

    #[test]
    fn test_direction_check() {
        let from = [0.0, 0.0];
        let right = [10.0, 0.0];
        let left = [-10.0, 0.0];
        let up = [0.0, -10.0];
        let down = [0.0, 10.0];

        assert!(is_in_direction(from, right, Direction::Right));
        assert!(is_in_direction(from, left, Direction::Left));
        assert!(is_in_direction(from, up, Direction::Up));
        assert!(is_in_direction(from, down, Direction::Down));

        assert!(!is_in_direction(from, left, Direction::Right));
        assert!(!is_in_direction(from, right, Direction::Left));
    }

    #[test]
    fn test_distance_calculation() {
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        assert_eq!(distance(a, b), 5.0);
    }

    #[test]
    fn test_data_dimension_arrow_left_right_navigates_sequentially() {
        let mut manager = FocusManager::new();
        manager.set_navigation_mode(NavigationMode::DataDimension);

        manager.add_focusable_element(create_test_element(0.0, 0.0, "A"));
        manager.add_focusable_element(create_test_element(100.0, 0.0, "B"));
        manager.add_focusable_element(create_test_element(200.0, 0.0, "C"));

        manager.set_focus(0);

        let action = manager.handle_key_input(KeyEvent::ArrowRight);
        assert_eq!(action, Some(AccessibilityAction::FocusChanged));
        assert_eq!(manager.current_focus, Some(1));

        let action = manager.handle_key_input(KeyEvent::ArrowLeft);
        assert_eq!(action, Some(AccessibilityAction::FocusChanged));
        assert_eq!(manager.current_focus, Some(0));
    }

    #[test]
    fn test_data_dimension_arrow_up_down_requests_cycle() {
        let mut manager = FocusManager::new();
        manager.set_navigation_mode(NavigationMode::DataDimension);

        manager.add_focusable_element(create_test_element(0.0, 0.0, "A"));
        manager.set_focus(0);

        let action = manager.handle_key_input(KeyEvent::ArrowDown);
        assert_eq!(
            action,
            Some(AccessibilityAction::DimensionCycleRequested { forward: true })
        );
        // Focus should NOT change.
        assert_eq!(manager.current_focus, Some(0));

        let action = manager.handle_key_input(KeyEvent::ArrowUp);
        assert_eq!(
            action,
            Some(AccessibilityAction::DimensionCycleRequested { forward: false })
        );
        assert_eq!(manager.current_focus, Some(0));
    }
}
