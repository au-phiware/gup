// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hierarchical (quadtree-like) grid spatial index.
//!
//! This implementation uses adaptive subdivision: cells that contain more
//! elements than a threshold are subdivided into 4 children. The maximum
//! depth is capped to prevent excessive subdivision. The tree is stored in
//! a flat array for GPU-friendly access patterns (no pointer chasing).
//!
//! The index returns *candidate* element indices based on cell overlap.
//! The caller is responsible for precise hit testing. This keeps per-element
//! overhead to just 4 bytes (one `u32` index in the sorted element list).

use super::{Aabb, ElementPosition, SpatialQuery};

/// Maximum subdivision depth. 8 levels gives cells as small as
/// 1/256 of the world bounds in each axis.
const MAX_DEPTH: u32 = 8;

/// A cell that contains more elements than this threshold will be subdivided
/// (unless it is already at `MAX_DEPTH`).
const SUBDIVISION_THRESHOLD: usize = 32;

/// A node in the hierarchical grid, stored in a flat array.
///
/// The layout is designed to be GPU-friendly: fixed-size struct, no pointers,
/// children referenced by array offset.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HierarchicalCell {
    /// World-space bounds of this cell.
    pub bounds: Aabb,
    /// Start index into the element list for this cell's elements.
    /// Only meaningful for leaf nodes.
    pub element_offset: u32,
    /// Number of elements directly in this cell (leaf) or total in subtree.
    pub element_count: u32,
    /// Index of the first child in the node array, or 0 if leaf.
    pub child_offset: u32,
    /// Depth in the hierarchy (0 = root).
    pub depth: u32,
}

impl HierarchicalCell {
    fn is_leaf(&self) -> bool {
        self.child_offset == 0
    }
}

/// A hierarchical grid spatial index with adaptive subdivision.
///
/// Element positions are stored alongside their indices for cell-overlap
/// testing during queries. This avoids referencing back to the original
/// element data while keeping the per-element cost low (12 bytes:
/// position `[f32; 2]` + index `u32`).
pub struct HierarchicalGrid {
    /// Flat array of nodes (root at index 0).
    nodes: Vec<HierarchicalCell>,
    /// Element indices sorted by cell assignment. Leaf cells reference
    /// contiguous ranges in this array.
    element_indices: Vec<u32>,
    /// Element centres indexed by element_index for cell-overlap testing.
    element_positions: Vec<[f32; 2]>,
    /// Element sizes indexed by element_index.
    element_sizes: Vec<[f32; 2]>,
    /// Total number of elements.
    count: usize,
}

impl HierarchicalGrid {
    /// Build a hierarchical grid from elements.
    pub fn build(elements: &[ElementPosition], bounds: Aabb) -> Self {
        if elements.is_empty() {
            return Self {
                nodes: Vec::new(),
                element_indices: Vec::new(),
                element_positions: Vec::new(),
                element_sizes: Vec::new(),
                count: 0,
            };
        }

        // Store element positions and sizes for queries
        let max_idx = elements.iter().map(|e| e.element_index).max().unwrap_or(0) as usize;
        let mut element_positions = vec![[0.0f32; 2]; max_idx + 1];
        let mut element_sizes = vec![[0.0f32; 2]; max_idx + 1];
        for e in elements {
            element_positions[e.element_index as usize] = e.position;
            element_sizes[e.element_index as usize] = e.size;
        }

        // Start with all element indices
        let all_indices: Vec<u32> = elements.iter().map(|e| e.element_index).collect();

        let mut nodes = Vec::with_capacity(elements.len().min(4096));
        let mut sorted_indices = Vec::with_capacity(elements.len());

        struct WorkItem {
            bounds: Aabb,
            indices: Vec<u32>,
            depth: u32,
            node_index: usize,
        }

        // Create root node (placeholder)
        nodes.push(HierarchicalCell {
            bounds,
            element_offset: 0,
            element_count: all_indices.len() as u32,
            child_offset: 0,
            depth: 0,
        });

        let mut stack = vec![WorkItem {
            bounds,
            indices: all_indices,
            depth: 0,
            node_index: 0,
        }];

        while let Some(item) = stack.pop() {
            // Should this cell be subdivided?
            if item.indices.len() <= SUBDIVISION_THRESHOLD || item.depth >= MAX_DEPTH {
                // Leaf node: store elements
                let offset = sorted_indices.len() as u32;
                sorted_indices.extend_from_slice(&item.indices);
                nodes[item.node_index].element_offset = offset;
                nodes[item.node_index].element_count = item.indices.len() as u32;
                nodes[item.node_index].child_offset = 0;
                continue;
            }

            // Subdivide into 4 quadrants
            let mid_x = (item.bounds.min[0] + item.bounds.max[0]) * 0.5;
            let mid_y = (item.bounds.min[1] + item.bounds.max[1]) * 0.5;

            let child_bounds = [
                Aabb::new(item.bounds.min, [mid_x, mid_y]), // Bottom-left
                Aabb::new([mid_x, item.bounds.min[1]], [item.bounds.max[0], mid_y]), // Bottom-right
                Aabb::new([item.bounds.min[0], mid_y], [mid_x, item.bounds.max[1]]), // Top-left
                Aabb::new([mid_x, mid_y], item.bounds.max), // Top-right
            ];

            // Distribute elements to children based on their centre position
            let mut child_indices = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            for &idx in &item.indices {
                let pos = element_positions[idx as usize];
                let quadrant = if pos[0] < mid_x {
                    if pos[1] < mid_y { 0 } else { 2 }
                } else if pos[1] < mid_y {
                    1
                } else {
                    3
                };
                child_indices[quadrant].push(idx);
            }

            // Check if subdivision is useful: if one child has almost all
            // elements, don't subdivide (prevents infinite descent with
            // coincident points).
            let max_child_count = child_indices.iter().map(|c| c.len()).max().unwrap_or(0);
            if max_child_count as f32 > item.indices.len() as f32 * 0.95 {
                // Subdivision not useful, make this a leaf
                let offset = sorted_indices.len() as u32;
                sorted_indices.extend_from_slice(&item.indices);
                nodes[item.node_index].element_offset = offset;
                nodes[item.node_index].element_count = item.indices.len() as u32;
                nodes[item.node_index].child_offset = 0;
                continue;
            }

            // Allocate child nodes
            let first_child = nodes.len();
            nodes[item.node_index].child_offset = first_child as u32;
            nodes[item.node_index].element_count = item.indices.len() as u32;

            for i in 0..4 {
                nodes.push(HierarchicalCell {
                    bounds: child_bounds[i],
                    element_offset: 0,
                    element_count: child_indices[i].len() as u32,
                    child_offset: 0,
                    depth: item.depth + 1,
                });
            }

            // Push children onto work stack (reverse order for DFS)
            for i in (0..4).rev() {
                if !child_indices[i].is_empty() {
                    stack.push(WorkItem {
                        bounds: child_bounds[i],
                        indices: std::mem::take(&mut child_indices[i]),
                        depth: item.depth + 1,
                        node_index: first_child + i,
                    });
                } else {
                    // Empty child is already a leaf with count=0
                    nodes[first_child + i].element_offset = sorted_indices.len() as u32;
                }
            }
        }

        Self {
            nodes,
            element_indices: sorted_indices,
            element_positions,
            element_sizes,
            count: elements.len(),
        }
    }

    /// Get the nodes array (for debugging / benchmarking).
    pub fn nodes(&self) -> &[HierarchicalCell] {
        &self.nodes
    }

    /// Get the depth of the tree.
    pub fn max_depth(&self) -> u32 {
        self.nodes.iter().map(|n| n.depth).max().unwrap_or(0)
    }
}

impl SpatialQuery for HierarchicalGrid {
    fn query_point(&self, point: [f32; 2]) -> Vec<u32> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut stack = vec![0usize];

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            if node.is_leaf() {
                let start = node.element_offset as usize;
                let end = start + node.element_count as usize;
                for &elem_idx in &self.element_indices[start..end] {
                    let i = elem_idx as usize;
                    if i < self.element_positions.len() {
                        let pos = self.element_positions[i];
                        let size = self.element_sizes[i];
                        let elem_bounds = Aabb::from_center_size(pos, size);
                        if elem_bounds.contains_point(point) {
                            results.push(elem_idx);
                        }
                    }
                }
            } else {
                // Descend into children whose bounds contain the point
                let first_child = node.child_offset as usize;
                for i in 0..4 {
                    let child_idx = first_child + i;
                    if child_idx < self.nodes.len() {
                        let child = &self.nodes[child_idx];
                        if child.element_count > 0 && child.bounds.contains_point(point) {
                            stack.push(child_idx);
                        }
                    }
                }
            }
        }

        results
    }

    fn query_region(&self, region: &Aabb) -> Vec<u32> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut stack = vec![0usize];

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            if node.is_leaf() {
                let start = node.element_offset as usize;
                let end = start + node.element_count as usize;
                for &elem_idx in &self.element_indices[start..end] {
                    let i = elem_idx as usize;
                    if i < self.element_positions.len() {
                        let pos = self.element_positions[i];
                        let size = self.element_sizes[i];
                        let elem_bounds = Aabb::from_center_size(pos, size);
                        if elem_bounds.intersects(region) {
                            results.push(elem_idx);
                        }
                    }
                }
            } else {
                let first_child = node.child_offset as usize;
                for i in 0..4 {
                    let child_idx = first_child + i;
                    if child_idx < self.nodes.len() {
                        let child = &self.nodes[child_idx];
                        if child.element_count > 0 && child.bounds.intersects(region) {
                            stack.push(child_idx);
                        }
                    }
                }
            }
        }

        results
    }

    fn element_count(&self) -> usize {
        self.count
    }

    fn memory_usage_bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<HierarchicalCell>()
            + self.element_indices.capacity() * std::mem::size_of::<u32>()
            + self.element_positions.capacity() * std::mem::size_of::<[f32; 2]>()
            + self.element_sizes.capacity() * std::mem::size_of::<[f32; 2]>()
            + std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid_elements(nx: usize, ny: usize, spacing: f32) -> Vec<ElementPosition> {
        let mut elements = Vec::with_capacity(nx * ny);
        let mut id = 0;
        for y in 0..ny {
            for x in 0..nx {
                elements.push(ElementPosition {
                    position: [x as f32 * spacing, y as f32 * spacing],
                    size: [5.0, 5.0],
                    element_index: id,
                });
                id += 1;
            }
        }
        elements
    }

    #[test]
    fn test_hierarchical_build_empty() {
        let bounds = Aabb::new([0.0, 0.0], [100.0, 100.0]);
        let grid = HierarchicalGrid::build(&[], bounds);
        assert_eq!(grid.element_count(), 0);
        assert!(grid.nodes().is_empty());
    }

    #[test]
    fn test_hierarchical_build_small() {
        let elements: Vec<ElementPosition> = (0..10)
            .map(|i| ElementPosition {
                position: [i as f32 * 10.0, i as f32 * 10.0],
                size: [5.0, 5.0],
                element_index: i as u32,
            })
            .collect();

        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        assert_eq!(grid.element_count(), 10);
        // Small dataset should be a single leaf (no subdivision)
        assert_eq!(grid.nodes().len(), 1);
        assert!(grid.nodes()[0].is_leaf());
    }

    #[test]
    fn test_hierarchical_build_subdivides() {
        // Create enough elements spread across the space to trigger subdivision
        let elements = make_grid_elements(20, 20, 5.0); // 400 elements
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        assert_eq!(grid.element_count(), 400);
        assert!(
            grid.nodes().len() > 1,
            "Should have subdivided: {} nodes",
            grid.nodes().len()
        );
        assert!(grid.max_depth() > 0, "Should have depth > 0");
    }

    #[test]
    fn test_hierarchical_point_query() {
        let elements = make_grid_elements(10, 10, 10.0);
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        // Query at an element position
        let hits = grid.query_point([0.0, 0.0]);
        assert!(!hits.is_empty(), "Should find element at origin");
        assert!(hits.contains(&0), "Should find element 0 at origin");

        // Query far from any element
        let far_hits = grid.query_point([500.0, 500.0]);
        assert!(far_hits.is_empty(), "Should find nothing far from data");
    }

    #[test]
    fn test_hierarchical_region_query() {
        let elements = make_grid_elements(10, 10, 10.0);
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        let region = Aabb::new([15.0, 15.0], [45.0, 45.0]);
        let hits = grid.query_region(&region);
        assert!(!hits.is_empty(), "Should find elements in region");

        // Verify all hits actually intersect the region
        for &idx in &hits {
            let elem = &elements[idx as usize];
            let elem_bounds = Aabb::from_center_size(elem.position, elem.size);
            assert!(
                elem_bounds.intersects(&region),
                "Element {idx} at ({}, {}) should intersect region",
                elem.position[0],
                elem.position[1]
            );
        }
    }

    #[test]
    fn test_hierarchical_clustered_data() {
        // Create four well-separated clusters so that subdivision is effective
        let mut elements = Vec::new();
        let mut id = 0u32;

        // Four clusters of 50 elements each in different quadrants
        for &center in &[[25.0, 25.0], [75.0, 25.0], [25.0, 75.0], [75.0, 75.0]] {
            for i in 0..50 {
                elements.push(ElementPosition {
                    position: [
                        center[0] + (i % 7) as f32 * 0.5,
                        center[1] + (i / 7) as f32 * 0.5,
                    ],
                    size: [2.0, 2.0],
                    element_index: id,
                });
                id += 1;
            }
        }

        let bounds = Aabb::new([0.0, 0.0], [100.0, 100.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        // 200 elements > threshold, and they're well-distributed across
        // quadrants, so subdivision should happen
        assert!(
            grid.max_depth() >= 1,
            "Clustered data should cause subdivision, got depth {}",
            grid.max_depth()
        );

        // Query in each cluster should find elements
        for &center in &[[25.0, 25.0], [75.0, 25.0], [25.0, 75.0], [75.0, 75.0]] {
            let hits = grid.query_point(center);
            assert!(!hits.is_empty(), "Should find elements at {:?}", center);
        }
    }

    #[test]
    fn test_hierarchical_all_elements_recovered() {
        let elements = make_grid_elements(10, 10, 10.0);
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        // Query the entire bounds should return all elements
        let all_hits = grid.query_region(&bounds);
        assert_eq!(
            all_hits.len(),
            elements.len(),
            "Full region query should return all elements"
        );
    }

    #[test]
    fn test_hierarchical_memory_overhead() {
        let elements = make_grid_elements(100, 100, 1.0); // 10,000 elements
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        let source_data_bytes = elements.len() * 32; // ElementData is 32 bytes
        let overhead_pct = grid.memory_usage_bytes() as f64 / source_data_bytes as f64 * 100.0;
        println!(
            "Hierarchical: {:.0} KB for {} elements, {:.1}% of source data",
            grid.memory_usage_bytes() as f64 / 1024.0,
            elements.len(),
            overhead_pct,
        );
        // Node overhead is small; per-element data (indices + positions + sizes)
        // is ~20 bytes/element = ~62.5% of 32-byte ElementData.
        assert!(
            overhead_pct < 200.0,
            "Hierarchical overhead should be manageable"
        );
    }

    #[test]
    fn test_hierarchical_no_infinite_subdivision() {
        // All elements at the same position: the 95% threshold should stop subdivision
        let elements: Vec<ElementPosition> = (0..200)
            .map(|i| ElementPosition {
                position: [50.0, 50.0],
                size: [5.0, 5.0],
                element_index: i as u32,
            })
            .collect();

        let bounds = Aabb::new([0.0, 0.0], [100.0, 100.0]);
        let grid = HierarchicalGrid::build(&elements, bounds);

        // Should not subdivide much because all elements go to same child
        assert!(
            grid.max_depth() <= 2,
            "Coincident elements should not cause deep subdivision, got depth {}",
            grid.max_depth()
        );
    }
}
