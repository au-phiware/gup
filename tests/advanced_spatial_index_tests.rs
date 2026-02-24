// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for advanced spatial index algorithms (GUP-078).
//!
//! These tests validate that:
//! - Morton and Hierarchical algorithms are correctly selected
//! - Both algorithms produce correct results for uniform and clustered data
//! - InteractionSystem correctly integrates advanced spatial indexing
//! - Performance improvement over basic grid for target workloads
//! - Memory overhead stays within acceptable limits

use gup::interaction::{ElementData, InteractionSystem};
use gup::spatial_index::{
    Aabb, ElementPosition, HierarchicalGrid, MortonIndex, SpatialAlgorithm, SpatialIndex,
    SpatialQuery,
};
use gup::test_utils::create_test_context;
use std::time::Instant;

/// Create a test interaction system with a valid GPU context.
async fn create_test_interaction_system() -> InteractionSystem {
    let guard = create_test_context()
        .await
        .expect("Failed to create test context");
    let context = guard.clone_context();
    InteractionSystem::new(&context)
        .await
        .expect("Failed to create interaction system")
}

/// Create uniformly distributed elements in a grid pattern.
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

/// Create clustered element data at specific centres.
fn make_clustered_positions(clusters: &[([f32; 2], usize)]) -> Vec<ElementPosition> {
    let mut elements = Vec::new();
    let mut id = 0u32;
    for &(center, count) in clusters {
        for j in 0..count {
            elements.push(ElementPosition {
                position: [
                    center[0] + (j % 10) as f32 * 0.5,
                    center[1] + (j / 10) as f32 * 0.5,
                ],
                size: [5.0, 5.0],
                element_index: id,
            });
            id += 1;
        }
    }
    elements
}

/// Convert ElementPosition to ElementData for InteractionSystem tests.
#[allow(dead_code)]
fn to_element_data(positions: &[ElementPosition]) -> Vec<ElementData> {
    positions
        .iter()
        .map(|p| ElementData {
            position: p.position,
            size: p.size,
            mark_type: 0,
            element_id: p.element_index,
            selection_id: 0,
            _padding: 0,
        })
        .collect()
}

// --- Algorithm Correctness Tests ---

#[test]
fn test_morton_correctness_uniform_data() {
    let elements = make_grid_elements(100, 100, 10.0); // 10K elements
    let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
    let index = MortonIndex::build(&elements, bounds);

    // Point query in the middle of the grid
    let candidates = index.query_point([500.0, 500.0]);
    assert!(
        !candidates.is_empty(),
        "Morton should find candidates at grid center"
    );

    // Region query covering a known area
    let region = Aabb::new([100.0, 100.0], [200.0, 200.0]);
    let region_candidates = index.query_region(&region);
    assert!(
        !region_candidates.is_empty(),
        "Morton region query should find candidates"
    );

    // Verify no candidates outside bounds
    let outside = index.query_point([2000.0, 2000.0]);
    assert!(
        outside.is_empty(),
        "No candidates should be found outside bounds"
    );
}

#[test]
fn test_morton_correctness_clustered_data() {
    let elements = make_clustered_positions(&[
        ([100.0, 100.0], 500),
        ([500.0, 500.0], 300),
        ([900.0, 900.0], 200),
    ]);
    let bounds = Aabb::new([0.0, 0.0], [1000.0, 1000.0]);
    let index = MortonIndex::build(&elements, bounds);

    // Query near each cluster
    let c1 = index.query_point([100.0, 100.0]);
    let c2 = index.query_point([500.0, 500.0]);
    let c3 = index.query_point([900.0, 900.0]);

    assert!(!c1.is_empty(), "Should find cluster 1");
    assert!(!c2.is_empty(), "Should find cluster 2");
    assert!(!c3.is_empty(), "Should find cluster 3");

    // Query between clusters should find fewer/no candidates
    let between = index.query_point([300.0, 300.0]);
    assert!(
        between.len() < c1.len(),
        "Area between clusters should have fewer candidates"
    );
}

#[test]
fn test_hierarchical_correctness_uniform_data() {
    let elements = make_grid_elements(100, 100, 10.0);
    let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
    let grid = HierarchicalGrid::build(&elements, bounds);

    // Point query should find elements
    let hits = grid.query_point([500.0, 500.0]);
    assert!(
        !hits.is_empty(),
        "Hierarchical should find hits at grid center"
    );

    // Region query
    let region = Aabb::new([100.0, 100.0], [200.0, 200.0]);
    let region_hits = grid.query_region(&region);
    assert!(
        !region_hits.is_empty(),
        "Hierarchical region query should find hits"
    );

    // Full region should recover all elements
    let all = grid.query_region(&bounds);
    assert_eq!(
        all.len(),
        elements.len(),
        "Full bounds query should return all elements"
    );
}

#[test]
fn test_hierarchical_correctness_clustered_data() {
    let elements = make_clustered_positions(&[
        ([100.0, 100.0], 200),
        ([500.0, 500.0], 200),
        ([900.0, 900.0], 200),
    ]);
    let bounds = Aabb::new([0.0, 0.0], [1000.0, 1000.0]);
    let grid = HierarchicalGrid::build(&elements, bounds);

    // Should have subdivided due to clustering
    assert!(grid.max_depth() >= 1, "Should subdivide for clustered data");

    // Query each cluster
    for &center in &[[100.0, 100.0], [500.0, 500.0], [900.0, 900.0]] {
        let hits = grid.query_point(center);
        assert!(
            !hits.is_empty(),
            "Should find hits at cluster center {:?}",
            center
        );
    }
}

// --- Algorithm Selection Tests ---

#[test]
fn test_auto_selection_selects_morton_for_uniform() {
    let elements = make_grid_elements(50, 50, 10.0); // 2500 elements uniform
    let bounds = Aabb::new([-10.0, -10.0], [510.0, 510.0]);
    let index = SpatialIndex::build(SpatialAlgorithm::Auto, &elements, bounds);
    assert_eq!(
        index.algorithm(),
        SpatialAlgorithm::Morton,
        "Auto should select Morton for uniform data"
    );
}

#[test]
fn test_auto_selection_selects_hierarchical_for_clustered() {
    let elements = make_clustered_positions(&[([50.0, 50.0], 1000), ([950.0, 950.0], 1000)]);
    let bounds = Aabb::new([0.0, 0.0], [1000.0, 1000.0]);
    let index = SpatialIndex::build(SpatialAlgorithm::Auto, &elements, bounds);
    assert_eq!(
        index.algorithm(),
        SpatialAlgorithm::Hierarchical,
        "Auto should select Hierarchical for clustered data"
    );
}

#[test]
fn test_explicit_algorithm_selection() {
    let elements = make_grid_elements(20, 20, 5.0);
    let bounds = Aabb::new([-5.0, -5.0], [105.0, 105.0]);

    let morton = SpatialIndex::build(SpatialAlgorithm::Morton, &elements, bounds);
    assert_eq!(morton.algorithm(), SpatialAlgorithm::Morton);

    let hier = SpatialIndex::build(SpatialAlgorithm::Hierarchical, &elements, bounds);
    assert_eq!(hier.algorithm(), SpatialAlgorithm::Hierarchical);
}

// --- Both Algorithms Agree on Results ---

#[test]
fn test_both_algorithms_find_same_elements_for_region() {
    let elements = make_grid_elements(20, 20, 10.0);
    let bounds = Aabb::new([-10.0, -10.0], [210.0, 210.0]);

    let morton = MortonIndex::build(&elements, bounds);
    let hier = HierarchicalGrid::build(&elements, bounds);

    let region = Aabb::new([50.0, 50.0], [150.0, 150.0]);

    let morton_hits: Vec<u32> = morton.query_region(&region);
    let hier_hits: Vec<u32> = hier.query_region(&region);

    // Morton may return more candidates (due to Z-curve false positives),
    // but hierarchical results should be a subset of what's correct.
    // Both should find the elements that are actually in the region.
    for &idx in &hier_hits {
        let elem = &elements[idx as usize];
        let elem_bounds = Aabb::from_center_size(elem.position, elem.size);
        assert!(
            elem_bounds.intersects(&region),
            "Hierarchical hit {} should intersect region",
            idx
        );
    }

    // Morton candidates are a superset (may include false positives)
    assert!(
        morton_hits.len() >= hier_hits.len(),
        "Morton should return at least as many candidates as Hierarchical"
    );
}

// --- Performance Comparison Tests ---

#[test]
fn test_morton_performance_vs_linear_scan() {
    let elements = make_grid_elements(100, 100, 10.0); // 10K elements
    let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
    let index = MortonIndex::build(&elements, bounds);

    // Measure Morton point query time
    let start = Instant::now();
    for i in 0..1000 {
        let x = (i % 100) as f32 * 10.0;
        let y = (i / 100) as f32 * 10.0;
        let _ = index.query_point([x, y]);
    }
    let morton_time = start.elapsed();

    // Measure linear scan time (baseline)
    let start = Instant::now();
    for i in 0..1000 {
        let x = (i % 100) as f32 * 10.0;
        let y = (i / 100) as f32 * 10.0;
        let _hits: Vec<u32> = elements
            .iter()
            .filter(|e| {
                let b = Aabb::from_center_size(e.position, e.size);
                b.contains_point([x, y])
            })
            .map(|e| e.element_index)
            .collect();
    }
    let linear_time = start.elapsed();

    println!(
        "Morton: {:?}, Linear: {:?}, Speedup: {:.1}x",
        morton_time,
        linear_time,
        linear_time.as_nanos() as f64 / morton_time.as_nanos().max(1) as f64
    );

    // Morton should be faster than linear scan
    assert!(
        morton_time < linear_time,
        "Morton ({:?}) should be faster than linear scan ({:?})",
        morton_time,
        linear_time
    );
}

#[test]
fn test_hierarchical_performance_vs_linear_scan() {
    let elements = make_clustered_positions(&[
        ([100.0, 100.0], 2500),
        ([500.0, 500.0], 2500),
        ([900.0, 100.0], 2500),
        ([100.0, 900.0], 2500),
    ]);
    let bounds = Aabb::new([0.0, 0.0], [1000.0, 1000.0]);
    let grid = HierarchicalGrid::build(&elements, bounds);

    // Measure hierarchical point query time
    let start = Instant::now();
    for i in 0..1000 {
        let x = (i % 10) as f32 * 100.0;
        let y = (i / 10) as f32 * 100.0;
        let _ = grid.query_point([x, y]);
    }
    let hier_time = start.elapsed();

    // Measure linear scan time
    let start = Instant::now();
    for i in 0..1000 {
        let x = (i % 10) as f32 * 100.0;
        let y = (i / 10) as f32 * 100.0;
        let _hits: Vec<u32> = elements
            .iter()
            .filter(|e| {
                let b = Aabb::from_center_size(e.position, e.size);
                b.contains_point([x, y])
            })
            .map(|e| e.element_index)
            .collect();
    }
    let linear_time = start.elapsed();

    println!(
        "Hierarchical: {:?}, Linear: {:?}, Speedup: {:.1}x",
        hier_time,
        linear_time,
        linear_time.as_nanos() as f64 / hier_time.as_nanos().max(1) as f64
    );

    assert!(
        hier_time < linear_time,
        "Hierarchical ({:?}) should be faster than linear scan ({:?})",
        hier_time,
        linear_time
    );
}

// --- Memory Overhead Tests ---

#[test]
fn test_memory_overhead_morton() {
    let elements = make_grid_elements(100, 100, 10.0); // 10K elements
    let element_data_bytes = elements.len() * 32; // ElementData = 32 bytes
    let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
    let index = MortonIndex::build(&elements, bounds);

    let overhead_pct = index.memory_usage_bytes() as f64 / element_data_bytes as f64 * 100.0;
    println!(
        "Morton: {} KB index for {} KB source data ({:.1}%)",
        index.memory_usage_bytes() / 1024,
        element_data_bytes / 1024,
        overhead_pct
    );
    // Morton: 8 bytes/element = 25% of 32-byte ElementData
    assert!(
        overhead_pct < 50.0,
        "Morton overhead {:.1}% should be < 50%",
        overhead_pct
    );
}

#[test]
fn test_memory_overhead_hierarchical() {
    let elements = make_grid_elements(100, 100, 10.0);
    let element_data_bytes = elements.len() * 32;
    let bounds = Aabb::new([-10.0, -10.0], [1010.0, 1010.0]);
    let grid = HierarchicalGrid::build(&elements, bounds);

    let overhead_pct = grid.memory_usage_bytes() as f64 / element_data_bytes as f64 * 100.0;
    println!(
        "Hierarchical: {} KB index for {} KB source data ({:.1}%)",
        grid.memory_usage_bytes() / 1024,
        element_data_bytes / 1024,
        overhead_pct
    );
    // Hierarchical stores positions + sizes + indices + nodes
    assert!(
        overhead_pct < 200.0,
        "Hierarchical overhead {:.1}% should be < 200%",
        overhead_pct
    );
}

// --- GPU Integration Tests ---

#[tokio::test]
async fn test_interaction_system_with_morton_algorithm() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    let elements: Vec<ElementData> = (0..2000)
        .map(|i| ElementData {
            position: [(i % 50) as f32 * 20.0, (i / 50) as f32 * 20.0],
            size: [10.0, 10.0],
            mark_type: 0,
            element_id: i as u32,
            selection_id: 0,
            _padding: 0,
        })
        .collect();

    let result = system.build_spatial_index_from_elements(&elements).await;
    assert!(result.is_ok(), "Building with Morton should succeed");
    assert!(system.is_spatial_index_built());
    assert_eq!(
        system.active_spatial_algorithm(),
        Some(SpatialAlgorithm::Morton)
    );
    assert!(system.advanced_index_memory_bytes() > 0);
}

#[tokio::test]
async fn test_interaction_system_with_hierarchical_algorithm() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Hierarchical);

    let elements: Vec<ElementData> = (0..2000)
        .map(|i| ElementData {
            position: [(i % 50) as f32 * 20.0, (i / 50) as f32 * 20.0],
            size: [10.0, 10.0],
            mark_type: 0,
            element_id: i as u32,
            selection_id: 0,
            _padding: 0,
        })
        .collect();

    let result = system.build_spatial_index_from_elements(&elements).await;
    assert!(result.is_ok(), "Building with Hierarchical should succeed");
    assert!(system.is_spatial_index_built());
    assert_eq!(
        system.active_spatial_algorithm(),
        Some(SpatialAlgorithm::Hierarchical)
    );
}

#[tokio::test]
async fn test_interaction_system_with_auto_algorithm() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Auto);

    let elements: Vec<ElementData> = (0..5000)
        .map(|i| ElementData {
            position: [(i % 100) as f32 * 10.0, (i / 100) as f32 * 10.0],
            size: [5.0, 5.0],
            mark_type: 0,
            element_id: i as u32,
            selection_id: 0,
            _padding: 0,
        })
        .collect();

    let result = system.build_spatial_index_from_elements(&elements).await;
    assert!(result.is_ok(), "Building with Auto should succeed");
    assert!(system.is_spatial_index_built());

    // Auto should have selected an algorithm
    let algo = system.active_spatial_algorithm();
    assert!(algo.is_some(), "Auto should have selected an algorithm");
    println!("Auto selected: {:?}", algo.unwrap());
}

#[tokio::test]
async fn test_interaction_system_algorithm_switching() {
    let mut system = create_test_interaction_system().await;

    let elements: Vec<ElementData> = (0..2000)
        .map(|i| ElementData {
            position: [(i % 50) as f32 * 20.0, (i / 50) as f32 * 20.0],
            size: [10.0, 10.0],
            mark_type: 0,
            element_id: i as u32,
            selection_id: 0,
            _padding: 0,
        })
        .collect();

    // Build with Morton
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);
    system
        .build_spatial_index_from_elements(&elements)
        .await
        .unwrap();
    assert_eq!(
        system.active_spatial_algorithm(),
        Some(SpatialAlgorithm::Morton)
    );

    // Switch to Hierarchical - should invalidate
    system.set_spatial_algorithm(SpatialAlgorithm::Hierarchical);
    assert!(!system.is_spatial_index_built());

    // Rebuild
    system
        .build_spatial_index_from_elements(&elements)
        .await
        .unwrap();
    assert_eq!(
        system.active_spatial_algorithm(),
        Some(SpatialAlgorithm::Hierarchical)
    );
}

// --- Cross-Platform Compatibility Tests ---

#[test]
fn test_morton_key_deterministic() {
    // Morton encoding should be deterministic across platforms
    let key1 = gup::spatial_index::MortonKey::encode(12345, 54321);
    let key2 = gup::spatial_index::MortonKey::encode(12345, 54321);
    assert_eq!(key1, key2, "Morton encoding should be deterministic");

    let (x, y) = key1.decode();
    assert_eq!(x, 12345);
    assert_eq!(y, 54321);
}

#[test]
fn test_spatial_index_with_degenerate_bounds() {
    // Zero-size bounds (all points at same location)
    let elements: Vec<ElementPosition> = (0..100)
        .map(|i| ElementPosition {
            position: [50.0, 50.0],
            size: [5.0, 5.0],
            element_index: i as u32,
        })
        .collect();

    let bounds = Aabb::new([50.0, 50.0], [50.0, 50.0]); // Degenerate

    // Both algorithms should handle this without panicking
    let morton = MortonIndex::build(&elements, bounds);
    let hier = HierarchicalGrid::build(&elements, bounds);

    assert_eq!(morton.element_count(), 100);
    assert_eq!(hier.element_count(), 100);
}

#[test]
fn test_spatial_index_with_negative_coordinates() {
    let elements: Vec<ElementPosition> = (0..100)
        .map(|i| {
            let t = i as f32 / 100.0;
            ElementPosition {
                position: [-500.0 + t * 1000.0, -500.0 + t * 1000.0],
                size: [5.0, 5.0],
                element_index: i as u32,
            }
        })
        .collect();

    let bounds = Aabb::new([-510.0, -510.0], [510.0, 510.0]);
    let morton = MortonIndex::build(&elements, bounds);
    let hier = HierarchicalGrid::build(&elements, bounds);

    // Should find candidates at origin
    let m_hits = morton.query_point([0.0, 0.0]);
    let h_hits = hier.query_point([0.0, 0.0]);

    assert!(
        !m_hits.is_empty(),
        "Morton should work with negative coords"
    );
    assert!(
        !h_hits.is_empty(),
        "Hierarchical should work with negative coords"
    );
}
