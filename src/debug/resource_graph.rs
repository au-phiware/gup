// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU resource dependency graph visualization and analysis.
//!
//! This module provides tools to visualize relationships between GPU resources
//! (buffers, pipelines, bind groups, textures) and analyze resource usage patterns.

use crate::error::{GupError, GupResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Unique identifier for a GPU resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(u64);

impl ResourceId {
    /// Create a new resource ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the underlying ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "R{}", self.0)
    }
}

/// Type of GPU resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// GPU buffer resource.
    Buffer,
    /// Compute or render pipeline.
    Pipeline,
    /// Bind group linking resources to a pipeline.
    BindGroup,
    /// GPU texture resource.
    Texture,
    /// Texture sampler.
    Sampler,
}

impl ResourceType {
    /// Get a color code for Graphviz visualization
    fn dot_color(&self) -> &'static str {
        match self {
            ResourceType::Buffer => "#4285F4",    // Blue
            ResourceType::Pipeline => "#EA4335",  // Red
            ResourceType::BindGroup => "#FBBC04", // Yellow
            ResourceType::Texture => "#34A853",   // Green
            ResourceType::Sampler => "#9C27B0",   // Purple
        }
    }

    /// Get a shape for Graphviz visualization
    fn dot_shape(&self) -> &'static str {
        match self {
            ResourceType::Buffer => "box",
            ResourceType::Pipeline => "ellipse",
            ResourceType::BindGroup => "diamond",
            ResourceType::Texture => "hexagon",
            ResourceType::Sampler => "octagon",
        }
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Buffer => write!(f, "Buffer"),
            ResourceType::Pipeline => write!(f, "Pipeline"),
            ResourceType::BindGroup => write!(f, "BindGroup"),
            ResourceType::Texture => write!(f, "Texture"),
            ResourceType::Sampler => write!(f, "Sampler"),
        }
    }
}

/// State of a GPU resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceState {
    /// Resource is actively in use.
    Active,
    /// Resource exists but has not been recently used.
    Inactive,
    /// Resource has been destroyed.
    Destroyed,
}

impl ResourceState {
    /// Get a color code for Graphviz visualization
    fn dot_color(&self) -> &'static str {
        match self {
            ResourceState::Active => "green",
            ResourceState::Inactive => "orange",
            ResourceState::Destroyed => "red",
        }
    }
}

/// GPU resource node in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceNode {
    /// Unique identifier for this resource.
    pub id: ResourceId,
    /// Type of GPU resource.
    pub resource_type: ResourceType,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// Size of the resource in bytes.
    pub size: u64,
    /// Current state of the resource.
    pub state: ResourceState,
    /// Optional usage flags description.
    pub usage_flags: Option<String>,
    /// Resources that this resource depends on
    pub dependencies: Vec<ResourceId>,
}

/// GPU resource dependency graph
#[derive(Debug, Clone, Default)]
pub struct ResourceGraph {
    /// All resource nodes indexed by ID
    nodes: HashMap<ResourceId, ResourceNode>,
    /// Reverse dependency map: resource -> resources that depend on it
    dependents: HashMap<ResourceId, HashSet<ResourceId>>,
    /// Next resource ID to allocate
    next_id: u64,
}

impl ResourceGraph {
    /// Create a new empty resource graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a resource node to the graph
    pub fn add_resource(
        &mut self,
        resource_type: ResourceType,
        label: Option<String>,
        size: u64,
        usage_flags: Option<String>,
        dependencies: Vec<ResourceId>,
    ) -> ResourceId {
        let id = ResourceId::new(self.next_id);
        self.next_id += 1;

        // Validate that all dependencies exist
        for dep_id in &dependencies {
            if !self.nodes.contains_key(dep_id) {
                log::warn!(
                    "Resource {:?} references non-existent dependency {:?}",
                    id,
                    dep_id
                );
            }
        }

        let node = ResourceNode {
            id,
            resource_type,
            label,
            size,
            state: ResourceState::Active,
            usage_flags,
            dependencies: dependencies.clone(),
        };

        self.nodes.insert(id, node);

        // Update reverse dependencies
        for dep_id in dependencies {
            self.dependents.entry(dep_id).or_default().insert(id);
        }

        id
    }

    /// Mark a resource as destroyed
    pub fn destroy_resource(&mut self, id: ResourceId) -> GupResult<()> {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.state = ResourceState::Destroyed;
            Ok(())
        } else {
            Err(GupError::resource_error(format!(
                "Resource {:?} not found",
                id
            )))
        }
    }

    /// Mark a resource as inactive (not recently used)
    pub fn mark_inactive(&mut self, id: ResourceId) -> GupResult<()> {
        if let Some(node) = self.nodes.get_mut(&id) {
            if node.state == ResourceState::Active {
                node.state = ResourceState::Inactive;
            }
            Ok(())
        } else {
            Err(GupError::resource_error(format!(
                "Resource {:?} not found",
                id
            )))
        }
    }

    /// Get a resource node by ID
    pub fn get_resource(&self, id: ResourceId) -> Option<&ResourceNode> {
        self.nodes.get(&id)
    }

    /// Get all resources
    pub fn resources(&self) -> impl Iterator<Item = &ResourceNode> {
        self.nodes.values()
    }

    /// Get resources that depend on the given resource
    pub fn get_dependents(&self, id: ResourceId) -> Vec<ResourceId> {
        self.dependents
            .get(&id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Detect circular dependencies
    pub fn detect_circular_dependencies(&self) -> Vec<Vec<ResourceId>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for id in self.nodes.keys() {
            if !visited.contains(id) {
                self.dfs_cycle_detection(*id, &mut visited, &mut rec_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    /// DFS helper for cycle detection
    fn dfs_cycle_detection(
        &self,
        id: ResourceId,
        visited: &mut HashSet<ResourceId>,
        rec_stack: &mut HashSet<ResourceId>,
        path: &mut Vec<ResourceId>,
        cycles: &mut Vec<Vec<ResourceId>>,
    ) {
        visited.insert(id);
        rec_stack.insert(id);
        path.push(id);

        if let Some(node) = self.nodes.get(&id) {
            for &dep_id in &node.dependencies {
                if !visited.contains(&dep_id) {
                    self.dfs_cycle_detection(dep_id, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(&dep_id) {
                    // Found a cycle
                    if let Some(cycle_start) = path.iter().position(|&x| x == dep_id) {
                        let cycle = path[cycle_start..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(&id);
    }

    /// Find unused resources (no dependents and not active)
    pub fn find_unused_resources(&self) -> Vec<ResourceId> {
        self.nodes
            .iter()
            .filter(|(id, node)| {
                node.state != ResourceState::Active
                    && self.dependents.get(id).is_none_or(|deps| deps.is_empty())
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Calculate total resource footprint for a dependency chain
    pub fn calculate_dependency_footprint(&self, id: ResourceId) -> u64 {
        let mut visited = HashSet::new();
        let mut total_size = 0u64;
        let mut queue = VecDeque::new();

        queue.push_back(id);
        visited.insert(id);

        while let Some(current_id) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current_id) {
                total_size += node.size;

                for &dep_id in &node.dependencies {
                    if !visited.contains(&dep_id) {
                        visited.insert(dep_id);
                        queue.push_back(dep_id);
                    }
                }
            }
        }

        total_size
    }

    /// Find resource sharing opportunities (resources with multiple dependents)
    pub fn find_sharing_opportunities(&self) -> Vec<(ResourceId, usize)> {
        let mut opportunities: Vec<_> = self
            .dependents
            .iter()
            .filter(|(_, deps)| deps.len() > 1)
            .map(|(id, deps)| (*id, deps.len()))
            .collect();

        opportunities.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        opportunities
    }

    /// Generate DOT format for Graphviz
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph ResourceGraph {\n");
        dot.push_str("  rankdir=LR;\n");
        dot.push_str("  node [style=filled];\n\n");

        // Add nodes
        for node in self.nodes.values() {
            let default_label = format!("{}", node.resource_type);
            let label = node.label.as_deref().unwrap_or(&default_label);
            let size_mb = node.size as f64 / (1024.0 * 1024.0);
            dot.push_str(&format!(
                "  {} [label=\"{}\\n{:.2} MB\", shape={}, fillcolor=\"{}\", color=\"{}\"];\n",
                node.id,
                label,
                size_mb,
                node.resource_type.dot_shape(),
                node.resource_type.dot_color(),
                node.state.dot_color()
            ));
        }

        dot.push('\n');

        // Add edges
        for node in self.nodes.values() {
            for &dep_id in &node.dependencies {
                dot.push_str(&format!("  {} -> {};\n", node.id, dep_id));
            }
        }

        dot.push_str("}\n");
        dot
    }

    /// Generate text-based tree visualization
    pub fn to_tree_text(&self, root_id: Option<ResourceId>) -> String {
        let mut output = String::new();

        if let Some(root) = root_id {
            self.render_tree_node(root, "", true, &mut output, &mut HashSet::new());
        } else {
            // Find root nodes (nodes with no dependencies)
            let roots: Vec<_> = self
                .nodes
                .values()
                .filter(|n| n.dependencies.is_empty())
                .map(|n| n.id)
                .collect();

            for (i, root) in roots.iter().enumerate() {
                let is_last = i == roots.len() - 1;
                self.render_tree_node(*root, "", is_last, &mut output, &mut HashSet::new());
            }
        }

        output
    }

    /// Helper to render a tree node
    fn render_tree_node(
        &self,
        id: ResourceId,
        prefix: &str,
        is_last: bool,
        output: &mut String,
        visited: &mut HashSet<ResourceId>,
    ) {
        if visited.contains(&id) {
            output.push_str(&format!(
                "{}{}─ {} (circular reference)\n",
                prefix,
                if is_last { "└" } else { "├" },
                id
            ));
            return;
        }

        visited.insert(id);

        if let Some(node) = self.nodes.get(&id) {
            let default_label = format!("{}", node.resource_type);
            let label = node.label.as_deref().unwrap_or(&default_label);
            let size_mb = node.size as f64 / (1024.0 * 1024.0);
            let state_icon = match node.state {
                ResourceState::Active => "✓",
                ResourceState::Inactive => "○",
                ResourceState::Destroyed => "✗",
            };

            output.push_str(&format!(
                "{}{}─ {} {} [{}, {:.2} MB]\n",
                prefix,
                if is_last { "└" } else { "├" },
                state_icon,
                label,
                node.resource_type,
                size_mb
            ));

            // Render dependencies
            let new_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });
            let dependents = self.get_dependents(id);

            for (i, dep_id) in dependents.iter().enumerate() {
                let is_last_dep = i == dependents.len() - 1;
                self.render_tree_node(*dep_id, &new_prefix, is_last_dep, output, visited);
            }
        }
    }

    /// Export graph data to JSON
    pub fn to_json(&self) -> GupResult<String> {
        let export = GraphExport {
            nodes: self.nodes.values().cloned().collect(),
            edges: self.collect_edges(),
        };

        serde_json::to_string_pretty(&export)
            .map_err(|e| GupError::invalid_operation(format!("Failed to serialize graph: {}", e)))
    }

    /// Collect all edges for export
    fn collect_edges(&self) -> Vec<GraphEdge> {
        let mut edges = Vec::new();

        for node in self.nodes.values() {
            for &dep_id in &node.dependencies {
                edges.push(GraphEdge {
                    from: node.id,
                    to: dep_id,
                });
            }
        }

        edges
    }

    /// Generate analysis report
    pub fn generate_report(&self) -> ResourceGraphReport {
        let total_resources = self.nodes.len();
        let active_resources = self
            .nodes
            .values()
            .filter(|n| n.state == ResourceState::Active)
            .count();
        let total_memory: u64 = self.nodes.values().map(|n| n.size).sum();
        let circular_deps = self.detect_circular_dependencies();
        let unused = self.find_unused_resources();
        let sharing_ops = self.find_sharing_opportunities();

        // Group by resource type
        let mut by_type: HashMap<ResourceType, usize> = HashMap::new();
        let mut memory_by_type: HashMap<ResourceType, u64> = HashMap::new();

        for node in self.nodes.values() {
            *by_type.entry(node.resource_type).or_insert(0) += 1;
            *memory_by_type.entry(node.resource_type).or_insert(0) += node.size;
        }

        ResourceGraphReport {
            total_resources,
            active_resources,
            inactive_resources: total_resources - active_resources,
            total_memory,
            circular_dependencies: circular_deps.len(),
            unused_resources: unused.len(),
            sharing_opportunities: sharing_ops.len(),
            resources_by_type: by_type,
            memory_by_type,
            circular_dependency_details: circular_deps,
            unused_resource_ids: unused,
            sharing_opportunity_details: sharing_ops,
        }
    }
}

/// Graph export format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphExport {
    nodes: Vec<ResourceNode>,
    edges: Vec<GraphEdge>,
}

/// Graph edge for export
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphEdge {
    from: ResourceId,
    to: ResourceId,
}

/// Resource graph analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGraphReport {
    /// Total number of resources in the graph.
    pub total_resources: usize,
    /// Number of active resources.
    pub active_resources: usize,
    /// Number of inactive or destroyed resources.
    pub inactive_resources: usize,
    /// Total memory consumed by all resources.
    pub total_memory: u64,
    /// Number of circular dependency cycles detected.
    pub circular_dependencies: usize,
    /// Number of unused resources detected.
    pub unused_resources: usize,
    /// Number of resource sharing opportunities found.
    pub sharing_opportunities: usize,
    /// Resource counts grouped by type.
    pub resources_by_type: HashMap<ResourceType, usize>,
    /// Memory usage grouped by resource type.
    pub memory_by_type: HashMap<ResourceType, u64>,
    /// Details of circular dependency cycles.
    pub circular_dependency_details: Vec<Vec<ResourceId>>,
    /// Identifiers of unused resources.
    pub unused_resource_ids: Vec<ResourceId>,
    /// Shared resources with their dependent count.
    pub sharing_opportunity_details: Vec<(ResourceId, usize)>,
}

impl ResourceGraphReport {
    /// Generate a formatted text report
    pub fn to_text(&self) -> String {
        let mut output = String::new();

        output.push_str("═══ Resource Graph Analysis Report ═══\n\n");

        // Summary
        output.push_str("┌─ Summary ─────────────────────────────────────┐\n");
        output.push_str(&format!(
            "│ Total Resources:     {:>10}           │\n",
            self.total_resources
        ));
        output.push_str(&format!(
            "│ Active Resources:    {:>10}           │\n",
            self.active_resources
        ));
        output.push_str(&format!(
            "│ Inactive Resources:  {:>10}           │\n",
            self.inactive_resources
        ));
        output.push_str(&format!(
            "│ Total Memory:        {:>10.2} MB     │\n",
            self.total_memory as f64 / (1024.0 * 1024.0)
        ));
        output.push_str("└───────────────────────────────────────────────┘\n\n");

        // Resources by type
        output.push_str("┌─ Resources by Type ───────────────────────────┐\n");
        for (resource_type, count) in &self.resources_by_type {
            let memory = self.memory_by_type.get(resource_type).unwrap_or(&0);
            let memory_mb = *memory as f64 / (1024.0 * 1024.0);
            output.push_str(&format!(
                "│ {:<15} {:>5}  ({:>8.2} MB)     │\n",
                format!("{:?}:", resource_type),
                count,
                memory_mb
            ));
        }
        output.push_str("└───────────────────────────────────────────────┘\n\n");

        // Issues
        output.push_str("┌─ Issues ──────────────────────────────────────┐\n");
        output.push_str(&format!(
            "│ Circular Dependencies: {:>10}           │\n",
            self.circular_dependencies
        ));
        output.push_str(&format!(
            "│ Unused Resources:      {:>10}           │\n",
            self.unused_resources
        ));
        output.push_str("└───────────────────────────────────────────────┘\n\n");

        // Opportunities
        output.push_str("┌─ Optimization Opportunities ──────────────────┐\n");
        output.push_str(&format!(
            "│ Sharing Opportunities: {:>10}           │\n",
            self.sharing_opportunities
        ));
        if !self.sharing_opportunity_details.is_empty() {
            output.push_str("│                                               │\n");
            output.push_str("│ Top shared resources:                         │\n");
            for (id, count) in self.sharing_opportunity_details.iter().take(5) {
                output.push_str(&format!(
                    "│   {} shared by {} resources         │\n",
                    id, count
                ));
            }
        }
        output.push_str("└───────────────────────────────────────────────┘\n");

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_graph_creation() {
        let mut graph = ResourceGraph::new();

        let buffer_id = graph.add_resource(
            ResourceType::Buffer,
            Some("Vertex Buffer".to_string()),
            1024 * 1024,
            Some("VERTEX | COPY_DST".to_string()),
            vec![],
        );

        let pipeline_id = graph.add_resource(
            ResourceType::Pipeline,
            Some("Render Pipeline".to_string()),
            512,
            None,
            vec![buffer_id],
        );

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.get_dependents(buffer_id), vec![pipeline_id]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut graph = ResourceGraph::new();

        let r1 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![]);
        let r2 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![r1]);

        // Manually create circular dependency
        if let Some(node) = graph.nodes.get_mut(&r1) {
            node.dependencies.push(r2);
            graph.dependents.entry(r2).or_default().insert(r1);
        }

        let cycles = graph.detect_circular_dependencies();
        assert!(!cycles.is_empty(), "Should detect circular dependency");
    }

    #[test]
    fn test_unused_resource_detection() {
        let mut graph = ResourceGraph::new();

        let r1 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![]);
        let _r2 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![r1]);
        let r3 = graph.add_resource(ResourceType::Texture, None, 2048, None, vec![]);

        // Mark r3 as inactive
        graph.mark_inactive(r3).unwrap();

        let unused = graph.find_unused_resources();
        assert!(unused.contains(&r3), "r3 should be detected as unused");
    }

    #[test]
    fn test_dependency_footprint() {
        let mut graph = ResourceGraph::new();

        let r1 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![]);
        let r2 = graph.add_resource(ResourceType::Buffer, None, 2048, None, vec![r1]);
        let r3 = graph.add_resource(ResourceType::Pipeline, None, 512, None, vec![r1, r2]);

        let footprint = graph.calculate_dependency_footprint(r3);
        assert_eq!(
            footprint,
            1024 + 2048 + 512,
            "Should calculate total footprint"
        );
    }

    #[test]
    fn test_sharing_opportunities() {
        let mut graph = ResourceGraph::new();

        let shared = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![]);
        let _r2 = graph.add_resource(ResourceType::Pipeline, None, 512, None, vec![shared]);
        let _r3 = graph.add_resource(ResourceType::Pipeline, None, 512, None, vec![shared]);
        let _r4 = graph.add_resource(ResourceType::BindGroup, None, 256, None, vec![shared]);

        let opportunities = graph.find_sharing_opportunities();
        assert_eq!(opportunities.len(), 1);
        assert_eq!(opportunities[0].0, shared);
        assert_eq!(
            opportunities[0].1, 3,
            "Shared resource should have 3 dependents"
        );
    }

    #[test]
    fn test_dot_export() {
        let mut graph = ResourceGraph::new();

        let r1 = graph.add_resource(
            ResourceType::Buffer,
            Some("Data".to_string()),
            1024,
            None,
            vec![],
        );
        let _r2 = graph.add_resource(
            ResourceType::Pipeline,
            Some("Render".to_string()),
            512,
            None,
            vec![r1],
        );

        let dot = graph.to_dot();
        assert!(dot.contains("digraph ResourceGraph"));
        assert!(dot.contains("Data"));
        assert!(dot.contains("Render"));
    }

    #[test]
    fn test_json_export() {
        let mut graph = ResourceGraph::new();

        let r1 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![]);
        let _r2 = graph.add_resource(ResourceType::Pipeline, None, 512, None, vec![r1]);

        let json = graph.to_json().unwrap();
        assert!(json.contains("nodes"));
        assert!(json.contains("edges"));
    }

    #[test]
    fn test_report_generation() {
        let mut graph = ResourceGraph::new();

        let r1 = graph.add_resource(ResourceType::Buffer, None, 1024, None, vec![]);
        let _r2 = graph.add_resource(ResourceType::Pipeline, None, 512, None, vec![r1]);
        let r3 = graph.add_resource(ResourceType::Texture, None, 2048, None, vec![]);
        graph.mark_inactive(r3).unwrap();

        let report = graph.generate_report();
        assert_eq!(report.total_resources, 3);
        assert_eq!(report.active_resources, 2);
        assert_eq!(report.total_memory, 1024 + 512 + 2048);
    }
}
