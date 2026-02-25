// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Selection–FocusManager integration for keyboard-accessible data points.
//!
//! This module bridges [`Selection`] with the accessibility [`FocusManager`],
//! enabling keyboard navigation of individual data points within a
//! visualization.  It provides:
//!
//! - Automatic extraction of mark positions into focusable elements
//! - ARIA node association for each focus element
//! - Data-dimension navigation (sort by X, Y, or value)
//! - Focus updates when the underlying data changes
//!
//! # Usage
//!
//! ```rust,ignore
//! use gup::accessibility::selection_focus::SelectionFocusBridge;
//! use gup::accessibility::{FocusManager, AccessibilitySystem};
//! use gup::selection::Selection;
//! use gup::mark::Circle;
//!
//! let selection: Selection<MyData, Circle> = Selection::from_data(data);
//! let mut bridge = SelectionFocusBridge::new(Default::default());
//!
//! // Register all data points as focusable elements
//! bridge.sync_focus_elements(
//!     &selection,
//!     &mut accessibility.focus_manager,
//!     |item, idx| FocusPointDescriptor {
//!         position: [item.x, item.y],
//!         label: format!("Point {}: ({:.1}, {:.1})", idx, item.x, item.y),
//!         value: Some(item.value),
//!     },
//! );
//! ```

use super::aria::{AriaNode, AriaRole, AriaTree, NodeId};
use super::focus_elements::{FocusElementConfig, MarkFocusHelper};
use super::keyboard::{FocusManager, NavigationMode};
use crate::interaction::Vec2;

/// Descriptor for a single focusable data point.
///
/// Produced by the user-supplied mapping closure to provide position,
/// labelling and an optional numeric value for dimension navigation.
#[derive(Debug, Clone)]
pub struct FocusPointDescriptor {
    /// Screen-space position (x, y) at which the focus element is placed.
    pub position: [f32; 2],

    /// Accessible label read by screen readers.
    pub label: String,

    /// Optional numeric value used for data-dimension navigation.
    pub value: Option<f64>,
}

/// Configuration for selection–focus integration.
#[derive(Debug, Clone)]
pub struct SelectionFocusConfig {
    /// Underlying focus element sizing / limits.
    pub element_config: FocusElementConfig,

    /// Whether to create ARIA nodes for each data point.
    pub create_aria_nodes: bool,

    /// Default navigation mode applied after syncing.
    pub default_navigation_mode: NavigationMode,
}

impl Default for SelectionFocusConfig {
    fn default() -> Self {
        Self {
            element_config: FocusElementConfig::default(),
            create_aria_nodes: true,
            default_navigation_mode: NavigationMode::Sequential,
        }
    }
}

/// Bridges a [`Selection`] with the [`FocusManager`] and optional [`AriaTree`].
///
/// Maintains a snapshot of the registered focusable elements so it can detect
/// when the data has changed and re-synchronize.
#[derive(Debug)]
pub struct SelectionFocusBridge {
    config: SelectionFocusConfig,
    helper: MarkFocusHelper,
    /// ARIA node IDs created for each data point (parallel to focus elements).
    aria_node_ids: Vec<NodeId>,
    /// Number of elements registered on the last sync.
    last_sync_count: usize,
    /// Cached descriptors for dimension navigation.
    descriptors: Vec<FocusPointDescriptor>,
}

impl SelectionFocusBridge {
    /// Create a new bridge with the given configuration.
    pub fn new(config: SelectionFocusConfig) -> Self {
        let helper = MarkFocusHelper::with_config(config.element_config.clone());
        Self {
            config,
            helper,
            aria_node_ids: Vec::new(),
            last_sync_count: 0,
            descriptors: Vec::new(),
        }
    }

    /// Synchronize focusable elements from a data slice.
    ///
    /// Clears any previously registered elements, then creates new focus
    /// elements from the data using the supplied `descriptor_fn` to map each
    /// data item to a [`FocusPointDescriptor`].
    ///
    /// Returns the number of elements registered (may be capped by
    /// [`FocusElementConfig::max_elements`]).
    pub fn sync_focus_elements<T, F>(
        &mut self,
        data: &[T],
        focus_manager: &mut FocusManager,
        descriptor_fn: F,
    ) -> usize
    where
        F: Fn(&T, usize) -> FocusPointDescriptor,
    {
        // Clear previous state.
        focus_manager.clear_focusable_elements();
        self.aria_node_ids.clear();
        self.descriptors.clear();

        // Build descriptors.
        let descriptors: Vec<FocusPointDescriptor> = data
            .iter()
            .enumerate()
            .map(|(i, d)| descriptor_fn(d, i))
            .collect();

        // Convert to the format expected by MarkFocusHelper.
        let positions: Vec<(Vec2, usize, String)> = descriptors
            .iter()
            .enumerate()
            .map(|(i, d)| (Vec2::new(d.position[0], d.position[1]), i, d.label.clone()))
            .collect();

        let count = self
            .helper
            .register_mark_positions(focus_manager, &positions);

        // Apply navigation mode.
        focus_manager.set_navigation_mode(self.config.default_navigation_mode.clone());

        self.last_sync_count = count;
        self.descriptors = descriptors;

        count
    }

    /// Synchronize and also register ARIA nodes for each data point.
    ///
    /// This is the full-featured variant: it calls [`sync_focus_elements`] and
    /// additionally creates an ARIA node per data point under the given
    /// `parent_node_id` in the provided [`AriaTree`].
    pub fn sync_focus_elements_with_aria<T, F>(
        &mut self,
        data: &[T],
        focus_manager: &mut FocusManager,
        aria_tree: &mut AriaTree,
        parent_node_id: NodeId,
        descriptor_fn: F,
    ) -> usize
    where
        F: Fn(&T, usize) -> FocusPointDescriptor,
    {
        let count = self.sync_focus_elements(data, focus_manager, &descriptor_fn);

        if self.config.create_aria_nodes {
            self.aria_node_ids.clear();
            for desc in &self.descriptors {
                let node = AriaNode::new(AriaRole::DataPoint, desc.label.clone());
                let node_id = aria_tree.add_child(parent_node_id, node);
                self.aria_node_ids.push(node_id);
            }
        }

        count
    }

    /// Re-sort the focus elements by a data dimension.
    ///
    /// This reorders the elements within the [`FocusManager`] so that
    /// sequential (Tab) navigation follows the chosen dimension order.
    pub fn sort_by_dimension(&self, focus_manager: &mut FocusManager, dimension: DataDimension) {
        // We can't directly reorder FocusManager's internal list, so we
        // rebuild it from our cached descriptors.
        let mut indexed: Vec<(usize, &FocusPointDescriptor)> =
            self.descriptors.iter().enumerate().collect();

        match dimension {
            DataDimension::X => indexed.sort_by(|a, b| {
                a.1.position[0]
                    .partial_cmp(&b.1.position[0])
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            DataDimension::Y => indexed.sort_by(|a, b| {
                a.1.position[1]
                    .partial_cmp(&b.1.position[1])
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            DataDimension::Value => indexed.sort_by(|a, b| {
                let va = a.1.value.unwrap_or(0.0);
                let vb = b.1.value.unwrap_or(0.0);
                va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
            }),
        }

        // Rebuild focus elements in sorted order.
        focus_manager.clear_focusable_elements();
        let elements = self.helper.create_focus_elements(
            &indexed
                .iter()
                .map(|(_, d)| {
                    (
                        Vec2::new(d.position[0], d.position[1]),
                        0usize, // data_index not important for sorted view
                        d.label.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        focus_manager.add_focusable_elements(elements);
    }

    /// Check whether the data has changed since the last sync.
    ///
    /// A simple length-based heuristic. Returns `true` if the data count
    /// differs from the last sync.
    pub fn needs_sync(&self, data_len: usize) -> bool {
        data_len != self.last_sync_count
    }

    /// Get the ARIA node IDs created during the last sync.
    pub fn aria_node_ids(&self) -> &[NodeId] {
        &self.aria_node_ids
    }

    /// Get the cached descriptors from the last sync.
    pub fn descriptors(&self) -> &[FocusPointDescriptor] {
        &self.descriptors
    }

    /// Get the number of elements registered on the last sync.
    pub fn last_sync_count(&self) -> usize {
        self.last_sync_count
    }
}

/// Data dimension used for sorted navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDimension {
    /// Sort by X position (left to right).
    X,
    /// Sort by Y position (top to bottom).
    Y,
    /// Sort by numeric value.
    Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test data type.
    #[derive(Debug, Clone)]
    struct Point {
        x: f32,
        y: f32,
        val: f64,
    }

    fn make_points() -> Vec<Point> {
        vec![
            Point {
                x: 100.0,
                y: 50.0,
                val: 3.0,
            },
            Point {
                x: 200.0,
                y: 150.0,
                val: 1.0,
            },
            Point {
                x: 50.0,
                y: 250.0,
                val: 5.0,
            },
            Point {
                x: 300.0,
                y: 100.0,
                val: 2.0,
            },
        ]
    }

    fn descriptor(p: &Point, idx: usize) -> FocusPointDescriptor {
        FocusPointDescriptor {
            position: [p.x, p.y],
            label: format!("Point {}: ({}, {})", idx, p.x, p.y),
            value: Some(p.val),
        }
    }

    #[test]
    fn sync_registers_all_elements() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        let count = bridge.sync_focus_elements(&points, &mut fm, descriptor);
        assert_eq!(count, 4);
        assert_eq!(bridge.last_sync_count(), 4);
    }

    #[test]
    fn sync_respects_max_elements() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let config = SelectionFocusConfig {
            element_config: FocusElementConfig {
                max_elements: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bridge = SelectionFocusBridge::new(config);

        let count = bridge.sync_focus_elements(&points, &mut fm, descriptor);
        assert_eq!(count, 2);
    }

    #[test]
    fn sync_clears_previous_state() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        bridge.sync_focus_elements(&points, &mut fm, descriptor);
        assert_eq!(bridge.last_sync_count(), 4);

        // Sync again with fewer items – should replace, not append.
        let fewer = vec![points[0].clone(), points[1].clone()];
        let count = bridge.sync_focus_elements(&fewer, &mut fm, descriptor);
        assert_eq!(count, 2);
        assert_eq!(bridge.last_sync_count(), 2);
    }

    #[test]
    fn needs_sync_detects_changes() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        bridge.sync_focus_elements(&points, &mut fm, descriptor);
        assert!(!bridge.needs_sync(4));
        assert!(bridge.needs_sync(3));
        assert!(bridge.needs_sync(5));
    }

    #[test]
    fn keyboard_navigation_works_after_sync() {
        use super::super::keyboard::KeyEvent;

        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        bridge.sync_focus_elements(&points, &mut fm, descriptor);

        // Tab through elements.
        fm.handle_key_input(KeyEvent::Tab);
        let desc = fm.describe_current_focus();
        assert!(desc.is_some());
        assert!(desc.unwrap().contains("Point 0"));

        fm.handle_key_input(KeyEvent::Tab);
        let desc = fm.describe_current_focus().unwrap();
        assert!(desc.contains("Point 1"));
    }

    #[test]
    fn spatial_navigation_works_after_sync() {
        use super::super::keyboard::KeyEvent;

        let points = make_points();
        let mut fm = FocusManager::new();
        let config = SelectionFocusConfig {
            default_navigation_mode: NavigationMode::Spatial,
            ..Default::default()
        };
        let mut bridge = SelectionFocusBridge::new(config);

        bridge.sync_focus_elements(&points, &mut fm, descriptor);

        // Start at first element.
        fm.set_focus(0); // Point 0 at (100, 50)

        // Arrow right should go to the nearest element to the right.
        fm.handle_key_input(KeyEvent::ArrowRight);
        let desc = fm.describe_current_focus().unwrap();
        // Point 1 (200, 150) or Point 3 (300, 100) should be reached.
        assert!(
            desc.contains("Point 1") || desc.contains("Point 3"),
            "Expected a point to the right, got: {}",
            desc
        );
    }

    #[test]
    fn sort_by_x_dimension() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        bridge.sync_focus_elements(&points, &mut fm, descriptor);

        // Sort by X.
        bridge.sort_by_dimension(&mut fm, DataDimension::X);

        // Tab through – should follow X order: 50, 100, 200, 300
        fm.handle_key_input(super::super::keyboard::KeyEvent::Tab);
        let desc = fm.describe_current_focus().unwrap();
        // First in X order is Point 2 at x=50.
        assert!(
            desc.contains("(50, 250)"),
            "First by X should be Point 2, got: {desc}"
        );
    }

    #[test]
    fn sort_by_value_dimension() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        bridge.sync_focus_elements(&points, &mut fm, descriptor);

        // Sort by value.
        bridge.sort_by_dimension(&mut fm, DataDimension::Value);

        // Tab through – should follow value order: 1.0, 2.0, 3.0, 5.0
        fm.handle_key_input(super::super::keyboard::KeyEvent::Tab);
        let desc = fm.describe_current_focus().unwrap();
        // First by value is Point 1 at val=1.0 (position 200, 150).
        assert!(
            desc.contains("(200, 150)"),
            "First by value should be Point 1, got: {desc}"
        );
    }

    #[test]
    fn aria_integration() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut aria_tree = AriaTree::new();
        let parent = aria_tree.create_chart_node("Test Chart".to_string(), None);

        let mut bridge = SelectionFocusBridge::new(Default::default());

        let count = bridge.sync_focus_elements_with_aria(
            &points,
            &mut fm,
            &mut aria_tree,
            parent,
            descriptor,
        );

        assert_eq!(count, 4);
        assert_eq!(bridge.aria_node_ids().len(), 4);

        // Verify ARIA nodes exist.
        for node_id in bridge.aria_node_ids() {
            let node = aria_tree.get_node(*node_id);
            assert!(node.is_some(), "ARIA node should exist");
            assert_eq!(node.unwrap().role, AriaRole::DataPoint);
        }
    }

    #[test]
    fn descriptors_are_cached() {
        let points = make_points();
        let mut fm = FocusManager::new();
        let mut bridge = SelectionFocusBridge::new(Default::default());

        bridge.sync_focus_elements(&points, &mut fm, descriptor);

        let descs = bridge.descriptors();
        assert_eq!(descs.len(), 4);
        assert_eq!(descs[0].position, [100.0, 50.0]);
        assert_eq!(descs[2].value, Some(5.0));
    }
}
