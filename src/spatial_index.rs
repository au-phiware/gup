// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Advanced spatial indexing algorithms for GPU-accelerated hit testing.
//!
//! This module provides multiple spatial index strategies optimized for
//! different data distributions. The interaction system can select the
//! best algorithm at runtime based on data characteristics.
//!
//! # Algorithms
//!
//! - **Uniform Grid** — the original fixed-cell-size grid (baseline).
//! - **Morton (Z-order curve)** — excellent spatial locality, simple GPU
//!   implementation, great for range queries.
//! - **Hierarchical Grid** — adaptive subdivision based on element density,
//!   better performance with clustered data.
//!
//! # Usage
//!
//! ```rust,ignore
//! use gup::spatial_index::{SpatialAlgorithm, SpatialIndex, ElementPosition};
//!
//! let elements: Vec<ElementPosition> = /* ... */;
//! let index = SpatialIndex::build(SpatialAlgorithm::Auto, &elements);
//! let hits = index.query_point([100.0, 200.0]);
//! ```

mod hierarchical;
mod morton;

pub use hierarchical::{HierarchicalCell, HierarchicalGrid};
pub use morton::{MortonEntry, MortonIndex, MortonKey, world_to_morton};

/// Position and metadata for an element to be indexed.
#[derive(Debug, Clone, Copy)]
pub struct ElementPosition {
    /// Centre position in world coordinates.
    pub position: [f32; 2],
    /// Size (width, height) for bounding-box queries.
    pub size: [f32; 2],
    /// Original element index (used to map back to ElementData).
    pub element_index: u32,
}

/// Axis-aligned bounding box for spatial queries.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    /// Minimum corner of the bounding box.
    pub min: [f32; 2],
    /// Maximum corner of the bounding box.
    pub max: [f32; 2],
}

impl Aabb {
    /// Create an AABB from minimum and maximum corners.
    pub fn new(min: [f32; 2], max: [f32; 2]) -> Self {
        Self { min, max }
    }

    /// Create an AABB from a centre and half-extents.
    pub fn from_center_size(center: [f32; 2], size: [f32; 2]) -> Self {
        Self {
            min: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            max: [center[0] + size[0] * 0.5, center[1] + size[1] * 0.5],
        }
    }

    /// Test whether `point` lies inside this bounding box (inclusive).
    pub fn contains_point(&self, point: [f32; 2]) -> bool {
        point[0] >= self.min[0]
            && point[0] <= self.max[0]
            && point[1] >= self.min[1]
            && point[1] <= self.max[1]
    }

    /// Test whether this bounding box overlaps `other`.
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min[0] <= other.max[0]
            && self.max[0] >= other.min[0]
            && self.min[1] <= other.max[1]
            && self.max[1] >= other.min[1]
    }

    /// Width of the bounding box along the x-axis.
    pub fn width(&self) -> f32 {
        self.max[0] - self.min[0]
    }

    /// Height of the bounding box along the y-axis.
    pub fn height(&self) -> f32 {
        self.max[1] - self.min[1]
    }
}

/// Which spatial algorithm to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialAlgorithm {
    /// Original uniform grid (baseline).
    UniformGrid,
    /// Z-order curve with binary search.
    Morton,
    /// Adaptive hierarchical grid.
    Hierarchical,
    /// Automatically select based on data characteristics.
    Auto,
}

/// Trait implemented by all spatial index strategies.
pub trait SpatialQuery {
    /// Find element indices whose bounding boxes contain `point`.
    fn query_point(&self, point: [f32; 2]) -> Vec<u32>;

    /// Find element indices that intersect the given region.
    fn query_region(&self, region: &Aabb) -> Vec<u32>;

    /// Total number of indexed elements.
    fn element_count(&self) -> usize;

    /// Approximate memory usage in bytes.
    fn memory_usage_bytes(&self) -> usize;
}

/// Unified spatial index that wraps different algorithm implementations.
pub enum SpatialIndex {
    /// Z-order curve index variant.
    Morton(MortonIndex),
    /// Adaptive hierarchical grid variant.
    Hierarchical(HierarchicalGrid),
}

impl SpatialIndex {
    /// Build a spatial index using the specified algorithm.
    pub fn build(algorithm: SpatialAlgorithm, elements: &[ElementPosition], bounds: Aabb) -> Self {
        match algorithm {
            SpatialAlgorithm::Morton => SpatialIndex::Morton(MortonIndex::build(elements, bounds)),
            SpatialAlgorithm::Hierarchical => {
                SpatialIndex::Hierarchical(HierarchicalGrid::build(elements, bounds))
            }
            SpatialAlgorithm::Auto | SpatialAlgorithm::UniformGrid => {
                // Auto-select: use hierarchical for clustered data, Morton for uniform
                let algorithm = select_algorithm(elements, &bounds);
                Self::build(algorithm, elements, bounds)
            }
        }
    }

    /// Get the algorithm type used.
    pub fn algorithm(&self) -> SpatialAlgorithm {
        match self {
            SpatialIndex::Morton(_) => SpatialAlgorithm::Morton,
            SpatialIndex::Hierarchical(_) => SpatialAlgorithm::Hierarchical,
        }
    }
}

impl SpatialQuery for SpatialIndex {
    fn query_point(&self, point: [f32; 2]) -> Vec<u32> {
        match self {
            SpatialIndex::Morton(idx) => idx.query_point(point),
            SpatialIndex::Hierarchical(idx) => idx.query_point(point),
        }
    }

    fn query_region(&self, region: &Aabb) -> Vec<u32> {
        match self {
            SpatialIndex::Morton(idx) => idx.query_region(region),
            SpatialIndex::Hierarchical(idx) => idx.query_region(region),
        }
    }

    fn element_count(&self) -> usize {
        match self {
            SpatialIndex::Morton(idx) => idx.element_count(),
            SpatialIndex::Hierarchical(idx) => idx.element_count(),
        }
    }

    fn memory_usage_bytes(&self) -> usize {
        match self {
            SpatialIndex::Morton(idx) => idx.memory_usage_bytes(),
            SpatialIndex::Hierarchical(idx) => idx.memory_usage_bytes(),
        }
    }
}

/// Select the best algorithm based on data characteristics.
///
/// Uses a simple heuristic: compute the coefficient of variation of
/// element density across a coarse grid. High variance ⇒ clustered
/// data ⇒ hierarchical grid. Low variance ⇒ Morton.
fn select_algorithm(elements: &[ElementPosition], bounds: &Aabb) -> SpatialAlgorithm {
    if elements.len() < 100 {
        // For small datasets either algorithm is fine; Morton is simpler
        return SpatialAlgorithm::Morton;
    }

    // Evaluate clustering using a coarse 8×8 grid
    let grid_size = 8usize;
    let total_cells = grid_size * grid_size;
    let mut counts = vec![0u32; total_cells];

    let w = bounds.width().max(f32::EPSILON);
    let h = bounds.height().max(f32::EPSILON);

    for elem in elements {
        let nx = ((elem.position[0] - bounds.min[0]) / w).clamp(0.0, 1.0 - f32::EPSILON);
        let ny = ((elem.position[1] - bounds.min[1]) / h).clamp(0.0, 1.0 - f32::EPSILON);
        let cx = (nx * grid_size as f32) as usize;
        let cy = (ny * grid_size as f32) as usize;
        counts[cy * grid_size + cx] += 1;
    }

    let mean = elements.len() as f64 / total_cells as f64;
    if mean < f64::EPSILON {
        return SpatialAlgorithm::Morton;
    }

    let variance: f64 = counts
        .iter()
        .map(|&c| {
            let diff = c as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / total_cells as f64;
    let cv = variance.sqrt() / mean;

    // High coefficient of variation → clustered → hierarchical
    if cv > 2.0 {
        SpatialAlgorithm::Hierarchical
    } else {
        SpatialAlgorithm::Morton
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uniform_elements(count: usize, spread: f32) -> Vec<ElementPosition> {
        // Distribute elements in a grid pattern for truly uniform coverage
        let side = (count as f32).sqrt().ceil() as usize;
        let step = spread / side.max(1) as f32;
        (0..count)
            .map(|i| {
                let x = (i % side) as f32 * step;
                let y = (i / side) as f32 * step;
                ElementPosition {
                    position: [x, y],
                    size: [5.0, 5.0],
                    element_index: i as u32,
                }
            })
            .collect()
    }

    fn make_clustered_elements(clusters: &[([f32; 2], usize)]) -> Vec<ElementPosition> {
        let mut elements = Vec::new();
        let mut id = 0u32;
        for &(center, count) in clusters {
            for j in 0..count {
                elements.push(ElementPosition {
                    position: [center[0] + (j as f32 * 0.1), center[1] + (j as f32 * 0.1)],
                    size: [5.0, 5.0],
                    element_index: id,
                });
                id += 1;
            }
        }
        elements
    }

    #[test]
    fn test_auto_selects_morton_for_uniform_data() {
        let elements = make_uniform_elements(1000, 1000.0);
        let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
        let algo = select_algorithm(&elements, &bounds);
        assert_eq!(algo, SpatialAlgorithm::Morton);
    }

    #[test]
    fn test_auto_selects_hierarchical_for_clustered_data() {
        let elements = make_clustered_elements(&[([100.0, 100.0], 500), ([900.0, 900.0], 500)]);
        let bounds = Aabb::new([0.0, 0.0], [1000.0, 1000.0]);
        let algo = select_algorithm(&elements, &bounds);
        assert_eq!(algo, SpatialAlgorithm::Hierarchical);
    }

    #[test]
    fn test_auto_selects_morton_for_small_datasets() {
        let elements = make_uniform_elements(50, 100.0);
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);
        let algo = select_algorithm(&elements, &bounds);
        assert_eq!(algo, SpatialAlgorithm::Morton);
    }

    #[test]
    fn test_aabb_contains_point() {
        let aabb = Aabb::new([0.0, 0.0], [10.0, 10.0]);
        assert!(aabb.contains_point([5.0, 5.0]));
        assert!(aabb.contains_point([0.0, 0.0]));
        assert!(aabb.contains_point([10.0, 10.0]));
        assert!(!aabb.contains_point([11.0, 5.0]));
        assert!(!aabb.contains_point([-1.0, 5.0]));
    }

    #[test]
    fn test_aabb_intersects() {
        let a = Aabb::new([0.0, 0.0], [10.0, 10.0]);
        let b = Aabb::new([5.0, 5.0], [15.0, 15.0]);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));

        let c = Aabb::new([20.0, 20.0], [30.0, 30.0]);
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_aabb_from_center_size() {
        let aabb = Aabb::from_center_size([5.0, 5.0], [10.0, 10.0]);
        assert_eq!(aabb.min, [0.0, 0.0]);
        assert_eq!(aabb.max, [10.0, 10.0]);
    }

    #[test]
    fn test_unified_index_point_query() {
        let elements = make_uniform_elements(100, 100.0);
        let bounds = Aabb::new([-10.0, -10.0], [110.0, 110.0]);

        // Both algorithms should return candidates for a point near elements
        let morton_idx = SpatialIndex::build(SpatialAlgorithm::Morton, &elements, bounds);
        let hier_idx = SpatialIndex::build(SpatialAlgorithm::Hierarchical, &elements, bounds);

        let morton_hits = morton_idx.query_point([50.0, 50.0]);
        let hier_hits = hier_idx.query_point([50.0, 50.0]);

        // Both should find candidates near (50,50)
        assert!(!morton_hits.is_empty(), "Morton should find candidates");
        assert!(!hier_hits.is_empty(), "Hierarchical should find candidates");
    }

    #[test]
    fn test_unified_index_memory_overhead() {
        let elements = make_uniform_elements(10_000, 1000.0);
        let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
        let element_data_bytes = elements.len() * 32; // ElementData is 32 bytes

        let morton_idx = SpatialIndex::build(SpatialAlgorithm::Morton, &elements, bounds);
        let hier_idx = SpatialIndex::build(SpatialAlgorithm::Hierarchical, &elements, bounds);

        let morton_pct = morton_idx.memory_usage_bytes() as f64 / element_data_bytes as f64 * 100.0;
        let hier_pct = hier_idx.memory_usage_bytes() as f64 / element_data_bytes as f64 * 100.0;

        println!(
            "Morton: {:.1}% of source data, Hierarchical: {:.1}% of source data",
            morton_pct, hier_pct
        );

        // Morton stores 8 bytes/element (25% of 32-byte ElementData)
        // Hierarchical stores ~20 bytes/element + nodes
        // Both should be well under 200% of source data
        assert!(
            morton_pct < 100.0,
            "Morton overhead {morton_pct:.1}% too high"
        );
        assert!(
            hier_pct < 200.0,
            "Hierarchical overhead {hier_pct:.1}% too high"
        );
    }
}
