// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! ARIA (Accessible Rich Internet Applications) integration for screen readers.
//!
//! This module provides semantic descriptions of visualizations and their data
//! for screen reader accessibility.

use std::collections::HashMap;

/// Unique identifier for ARIA nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    /// Create a new unique node ID.
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

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// ARIA tree structure representing the semantic hierarchy of a visualization.
#[derive(Debug)]
pub struct AriaTree {
    /// Root node of the tree
    root: Option<NodeId>,

    /// All nodes in the tree
    nodes: HashMap<NodeId, AriaNode>,

    /// Current focus within the tree
    current_focus: Option<NodeId>,

    /// Queue of updates to be sent to screen reader
    update_queue: Vec<AriaUpdate>,
}

impl AriaTree {
    /// Create a new empty ARIA tree.
    pub fn new() -> Self {
        Self {
            root: None,
            nodes: HashMap::new(),
            current_focus: None,
            update_queue: Vec::new(),
        }
    }

    /// Create a chart description node.
    pub fn create_chart_node(&mut self, label: String, description: Option<String>) -> NodeId {
        let node = AriaNode {
            id: NodeId::new(),
            role: AriaRole::Chart,
            label,
            description,
            value: None,
            children: Vec::new(),
            properties: AriaProperties::default(),
        };

        let id = node.id;
        self.nodes.insert(id, node);

        if self.root.is_none() {
            self.root = Some(id);
        }

        self.queue_update(AriaUpdate::NodeCreated { node_id: id });
        id
    }

    /// Add a child node to a parent.
    pub fn add_child(&mut self, parent_id: NodeId, child: AriaNode) -> NodeId {
        let child_id = child.id;
        self.nodes.insert(child_id, child);

        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }

        self.queue_update(AriaUpdate::NodeCreated { node_id: child_id });
        child_id
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: NodeId) -> Option<&AriaNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a node by ID.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut AriaNode> {
        self.nodes.get_mut(&id)
    }

    /// Set the current focus.
    pub fn set_focus(&mut self, node_id: Option<NodeId>) {
        if self.current_focus != node_id {
            self.current_focus = node_id;
            if let Some(id) = node_id {
                self.queue_update(AriaUpdate::FocusChanged { node_id: id });
            }
        }
    }

    /// Get the current focus.
    pub fn get_focus(&self) -> Option<NodeId> {
        self.current_focus
    }

    /// Update a live region with new content.
    pub fn update_live_region(&mut self, region_id: &str, content: &str) {
        self.queue_update(AriaUpdate::LiveRegion {
            id: region_id.to_string(),
            content: content.to_string(),
            urgency: AriaLive::Polite,
        });
    }

    /// Queue an update for the screen reader.
    fn queue_update(&mut self, update: AriaUpdate) {
        self.update_queue.push(update);
    }

    /// Get and clear pending updates.
    pub fn drain_update_queue(&mut self) -> Vec<AriaUpdate> {
        std::mem::take(&mut self.update_queue)
    }

    /// Get the root node ID of this tree.
    pub fn get_root_node(&self) -> Option<NodeId> {
        self.root
    }

    /// Analyze data patterns for description generation.
    pub fn analyze_data_patterns<T>(&self, data: &[T]) -> String {
        let count = data.len();

        if count == 0 {
            "Empty dataset".to_string()
        } else if count == 1 {
            "Single data point".to_string()
        } else if count < 100 {
            format!("Small dataset with {} points", count)
        } else if count < 10000 {
            format!("Medium dataset with {} points", count)
        } else {
            format!("Large dataset with {} points", count)
        }
    }

    /// Remove a node and all its descendants from the tree.
    ///
    /// If the removed node is the tree root, the root is cleared.
    /// Emits [`AriaUpdate::NodeRemoved`] for each removed node.
    pub fn remove_subtree(&mut self, node_id: NodeId) {
        // Collect all IDs to remove via breadth-first traversal.
        let mut to_remove = vec![node_id];
        let mut i = 0;
        while i < to_remove.len() {
            if let Some(node) = self.nodes.get(&to_remove[i]) {
                to_remove.extend_from_slice(&node.children);
            }
            i += 1;
        }

        // Remove from parent's children list.
        // We search all nodes since we don't store parent pointers.
        for node in self.nodes.values_mut() {
            node.children.retain(|c| *c != node_id);
        }

        // Remove all collected nodes.
        for id in &to_remove {
            self.nodes.remove(id);
            self.queue_update(AriaUpdate::NodeRemoved { node_id: *id });
        }

        // Clear root if it was removed.
        if self.root == Some(node_id) {
            self.root = None;
        }

        // Clear focus if it pointed to a removed node.
        if let Some(focus) = self.current_focus
            && to_remove.contains(&focus)
        {
            self.current_focus = None;
        }
    }

    /// Returns the total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for AriaTree {
    fn default() -> Self {
        Self::new()
    }
}

/// A node in the ARIA tree representing a semantic element.
#[derive(Debug, Clone)]
pub struct AriaNode {
    /// Unique identifier for this node
    pub id: NodeId,

    /// ARIA role describing the element type
    pub role: AriaRole,

    /// Accessible label
    pub label: String,

    /// Optional detailed description
    pub description: Option<String>,

    /// Optional value for interactive elements
    pub value: Option<String>,

    /// Child nodes
    pub children: Vec<NodeId>,

    /// Additional ARIA properties
    pub properties: AriaProperties,
}

impl AriaNode {
    /// Create a new ARIA node.
    pub fn new(role: AriaRole, label: String) -> Self {
        Self {
            id: NodeId::new(),
            role,
            label,
            description: None,
            value: None,
            children: Vec::new(),
            properties: AriaProperties::default(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the value.
    pub fn with_value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    /// Set live region properties.
    pub fn with_live(mut self, live: AriaLive) -> Self {
        self.properties.live = Some(live);
        self
    }
}

/// ARIA roles for different element types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaRole {
    /// A data visualization chart
    Chart,

    /// A series within a chart
    ChartSeries,

    /// An individual data point
    DataPoint,

    /// A legend explaining visual encodings
    Legend,

    /// An axis showing scale
    Axis,

    /// A tooltip with contextual information
    Tooltip,

    /// An interactive control
    Control,
}

impl AriaRole {
    /// Get the ARIA role name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chart => "img",
            Self::ChartSeries => "list",
            Self::DataPoint => "listitem",
            Self::Legend => "region",
            Self::Axis => "separator",
            Self::Tooltip => "tooltip",
            Self::Control => "button",
        }
    }
}

/// ARIA live region properties.
#[derive(Debug, Clone)]
pub struct AriaProperties {
    /// Live region update urgency
    pub live: Option<AriaLive>,

    /// Whether updates should be atomic
    pub atomic: bool,

    /// What types of changes are relevant
    pub relevant: AriaRelevant,
}

impl Default for AriaProperties {
    fn default() -> Self {
        Self {
            live: None,
            atomic: false,
            relevant: AriaRelevant::ADDITIONS | AriaRelevant::TEXT,
        }
    }
}

/// ARIA live region urgency levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaLive {
    /// Updates announced immediately
    Assertive,

    /// Updates announced when convenient
    Polite,

    /// No announcements
    Off,
}

/// ARIA relevant update types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AriaRelevant(u8);

impl AriaRelevant {
    pub const ADDITIONS: Self = Self(0b0001);
    pub const REMOVALS: Self = Self(0b0010);
    pub const TEXT: Self = Self(0b0100);
    pub const ALL: Self = Self(0b1111);
}

impl std::ops::BitOr for AriaRelevant {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Updates to be communicated to screen readers.
#[derive(Debug, Clone)]
pub enum AriaUpdate {
    /// A new node was created
    NodeCreated { node_id: NodeId },

    /// A node was updated
    NodeUpdated { node_id: NodeId },

    /// A node was removed
    NodeRemoved { node_id: NodeId },

    /// Focus changed to a new node
    FocusChanged { node_id: NodeId },

    /// Live region was updated
    LiveRegion {
        id: String,
        content: String,
        urgency: AriaLive,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_generation() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_aria_tree_creation() {
        let mut tree = AriaTree::new();
        let chart_id = tree.create_chart_node(
            "Scatter plot".to_string(),
            Some("Sales data over time".to_string()),
        );

        assert!(tree.get_node(chart_id).is_some());
        assert_eq!(tree.root, Some(chart_id));
    }

    #[test]
    fn test_aria_tree_hierarchy() {
        let mut tree = AriaTree::new();
        let chart_id = tree.create_chart_node("Chart".to_string(), None);

        let series = AriaNode::new(AriaRole::ChartSeries, "Series 1".to_string());
        let series_id = tree.add_child(chart_id, series);

        let chart = tree.get_node(chart_id).unwrap();
        assert_eq!(chart.children.len(), 1);
        assert_eq!(chart.children[0], series_id);
    }

    #[test]
    fn test_aria_focus_management() {
        let mut tree = AriaTree::new();
        let node_id = tree.create_chart_node("Chart".to_string(), None);

        assert_eq!(tree.get_focus(), None);

        tree.set_focus(Some(node_id));
        assert_eq!(tree.get_focus(), Some(node_id));

        tree.set_focus(None);
        assert_eq!(tree.get_focus(), None);
    }

    #[test]
    fn test_aria_update_queue() {
        let mut tree = AriaTree::new();
        let _node_id = tree.create_chart_node("Chart".to_string(), None);

        let updates = tree.drain_update_queue();
        assert_eq!(updates.len(), 1);

        // Queue should be empty after draining
        let updates2 = tree.drain_update_queue();
        assert_eq!(updates2.len(), 0);
    }

    #[test]
    fn test_aria_live_region() {
        let mut tree = AriaTree::new();
        tree.update_live_region("status", "Data updated");

        let updates = tree.drain_update_queue();
        assert_eq!(updates.len(), 1);

        match &updates[0] {
            AriaUpdate::LiveRegion { id, content, .. } => {
                assert_eq!(id, "status");
                assert_eq!(content, "Data updated");
            }
            _ => panic!("Expected LiveRegion update"),
        }
    }

    #[test]
    fn test_data_pattern_analysis() {
        let tree = AriaTree::new();

        let empty: Vec<i32> = vec![];
        assert_eq!(tree.analyze_data_patterns(&empty), "Empty dataset");

        let single = vec![1];
        assert_eq!(tree.analyze_data_patterns(&single), "Single data point");

        let small = vec![1; 50];
        assert!(tree.analyze_data_patterns(&small).contains("Small dataset"));

        let medium = vec![1; 500];
        assert!(
            tree.analyze_data_patterns(&medium)
                .contains("Medium dataset")
        );

        let large = vec![1; 50000];
        assert!(tree.analyze_data_patterns(&large).contains("Large dataset"));
    }

    #[test]
    fn test_aria_role_strings() {
        assert_eq!(AriaRole::Chart.as_str(), "img");
        assert_eq!(AriaRole::DataPoint.as_str(), "listitem");
        assert_eq!(AriaRole::Control.as_str(), "button");
    }

    #[test]
    fn test_remove_subtree() {
        let mut tree = AriaTree::new();
        let root = tree.create_chart_node("Root".to_string(), None);
        let child = tree.add_child(root, AriaNode::new(AriaRole::ChartSeries, "S1".into()));
        let grandchild = tree.add_child(child, AriaNode::new(AriaRole::DataPoint, "P1".into()));

        assert_eq!(tree.node_count(), 3);

        tree.remove_subtree(child);

        // child and grandchild should be gone
        assert!(tree.get_node(child).is_none());
        assert!(tree.get_node(grandchild).is_none());
        // root should remain but without the child reference
        assert!(tree.get_node(root).is_some());
        assert!(tree.get_node(root).unwrap().children.is_empty());
        assert_eq!(tree.node_count(), 1);
    }

    #[test]
    fn test_remove_subtree_root() {
        let mut tree = AriaTree::new();
        let root = tree.create_chart_node("Root".to_string(), None);
        tree.add_child(root, AriaNode::new(AriaRole::DataPoint, "P1".into()));

        tree.remove_subtree(root);
        assert_eq!(tree.node_count(), 0);
        assert!(tree.get_root_node().is_none());
    }

    #[test]
    fn test_remove_subtree_clears_focus() {
        let mut tree = AriaTree::new();
        let root = tree.create_chart_node("Root".to_string(), None);
        let child = tree.add_child(root, AriaNode::new(AriaRole::DataPoint, "P1".into()));

        tree.set_focus(Some(child));
        assert_eq!(tree.get_focus(), Some(child));

        tree.remove_subtree(child);
        assert_eq!(tree.get_focus(), None);
    }
}
