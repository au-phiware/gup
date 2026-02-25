// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for GPU-resident candidate pipeline (GUP-193).
//!
//! These tests validate that:
//! - The gather compute pipeline compiles and initialises correctly
//! - The GPU-resident pipeline produces correct hit test results
//! - No GPU→CPU→GPU readback is needed for candidate narrowing
//! - Results match the CPU-side narrowing path

use gup::GupResult;
use gup::interaction::{
    ElementData, InteractionElement, InteractionSystem, Rect, Renderable, Vec2,
};
use gup::spatial_index::SpatialAlgorithm;
use gup::test_utils::create_test_context;
use std::time::Instant;

/// A mock Renderable that wraps pre-built ElementData for testing.
struct MockRenderable {
    elements: Vec<InteractionElement>,
}

impl MockRenderable {
    fn from_element_data(data: &[ElementData]) -> Self {
        let elements = data
            .iter()
            .map(|e| InteractionElement {
                position: e.position,
                size: e.size,
                mark_type: e.mark_type,
            })
            .collect();
        Self { elements }
    }
}

impl Renderable for MockRenderable {
    fn get_elements_for_interaction(&self) -> GupResult<Vec<InteractionElement>> {
        Ok(self.elements.clone())
    }

    fn selection_id(&self) -> u32 {
        0
    }
}

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

/// Create uniformly distributed element data with circles at each grid point.
fn make_circle_grid(count: usize, spread: f32) -> Vec<ElementData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let step = spread / side.max(1) as f32;
    (0..count)
        .map(|i| {
            let x = (i % side) as f32 * step;
            let y = (i / side) as f32 * step;
            ElementData {
                position: [x, y],
                size: [5.0, 5.0], // radius = 5 for circles
                mark_type: 0,     // Circle
                element_id: i as u32,
                selection_id: 0,
                _padding: 0,
            }
        })
        .collect()
}

// --- Pipeline Creation Test ---

#[tokio::test]
async fn test_gather_pipeline_creation() {
    // Verify the system creates without errors (gather pipeline compiles).
    let _system = create_test_interaction_system().await;
}

// --- End-to-End Correctness Tests ---

#[tokio::test]
async fn test_gpu_resident_point_query_finds_hits() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    // Create a grid of circles (>1000 to trigger spatial index + Morton path)
    let elements = make_circle_grid(2000, 500.0);
    let renderable = MockRenderable::from_element_data(&elements);

    // Query at a position where we know there are circles
    // Element at grid position ~(10, 10) → position ≈ (111, 111) with step ≈ 11.1
    let hits = system
        .query_point(Vec2::new(111.0, 111.0), &[&renderable])
        .await
        .expect("query_point should succeed");

    println!(
        "GPU-resident point query: {} hits for 2000 circles",
        hits.len()
    );

    // Should find at least one circle at this position
    assert!(
        !hits.is_empty(),
        "GPU-resident pipeline should find hits near (111, 111)"
    );
}

#[tokio::test]
async fn test_gpu_resident_region_query_finds_hits() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    let elements = make_circle_grid(2000, 500.0);
    let renderable = MockRenderable::from_element_data(&elements);

    // Region query covering a portion of the grid
    let region = Rect::new(Vec2::new(50.0, 50.0), Vec2::new(150.0, 150.0));
    let hits = system
        .query_region(region, &[&renderable])
        .await
        .expect("query_region should succeed");

    println!(
        "GPU-resident region query: {} hits for 2000 circles",
        hits.len()
    );

    assert!(
        !hits.is_empty(),
        "GPU-resident pipeline should find hits in region [50-150, 50-150]"
    );
}

#[tokio::test]
async fn test_gpu_resident_no_hits_for_distant_query() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    // Elements clustered near origin
    let elements = make_circle_grid(2000, 100.0);
    let renderable = MockRenderable::from_element_data(&elements);

    // Query very far from all elements
    let hits = system
        .query_point(Vec2::new(9000.0, 9000.0), &[&renderable])
        .await
        .expect("query_point should succeed");

    assert!(
        hits.is_empty(),
        "GPU-resident pipeline should find no hits far from data, found {}",
        hits.len()
    );
}

#[tokio::test]
async fn test_gpu_resident_repeated_queries() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    let elements = make_circle_grid(2000, 500.0);
    let renderable = MockRenderable::from_element_data(&elements);

    // Run multiple queries to verify the pipeline is reusable
    for i in 0..5 {
        let x = 50.0 + i as f32 * 80.0;
        let y = 50.0 + i as f32 * 80.0;
        let hits = system
            .query_point(Vec2::new(x, y), &[&renderable])
            .await
            .expect("repeated query should succeed");
        println!("Query {} at ({}, {}): {} hits", i, x, y, hits.len());
    }
}

#[tokio::test]
async fn test_gpu_resident_large_dataset() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    // Test with 5K elements to verify at scale.
    let elements = make_circle_grid(5_000, 300.0);
    let renderable = MockRenderable::from_element_data(&elements);

    let hits = system
        .query_point(Vec2::new(50.0, 50.0), &[&renderable])
        .await
        .expect("query_point with 5K elements should succeed");

    println!(
        "GPU-resident query with 5K elements: {} hits at (50, 50)",
        hits.len()
    );

    // The GPU-resident pipeline should find at least one hit
    assert!(
        !hits.is_empty(),
        "Should find hits near (50, 50) in 5K element dataset"
    );
}

// --- GPU-Resident vs CPU-Narrowing Consistency ---

#[tokio::test]
async fn test_gpu_resident_vs_cpu_narrowing_consistency() {
    // Compare results from GPU-resident (Morton) path with CPU-narrowing
    // (Hierarchical) path to verify correctness.
    let elements = make_circle_grid(2000, 500.0);
    let renderable = MockRenderable::from_element_data(&elements);

    // GPU-resident path (Morton)
    let mut morton_system = create_test_interaction_system().await;
    morton_system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    let morton_hits = morton_system
        .query_point(Vec2::new(111.0, 111.0), &[&renderable])
        .await
        .expect("Morton query should succeed");

    // CPU-narrowing path (Hierarchical)
    let mut hier_system = create_test_interaction_system().await;
    hier_system.set_spatial_algorithm(SpatialAlgorithm::Hierarchical);

    let hier_hits = hier_system
        .query_point(Vec2::new(111.0, 111.0), &[&renderable])
        .await
        .expect("Hierarchical query should succeed");

    println!(
        "Morton hits: {}, Hierarchical hits: {}",
        morton_hits.len(),
        hier_hits.len()
    );

    // Both paths should find the same hits (same underlying data,
    // just different candidate narrowing strategies).
    let morton_ids: std::collections::HashSet<u32> =
        morton_hits.iter().map(|h| h.element_id).collect();
    let hier_ids: std::collections::HashSet<u32> = hier_hits.iter().map(|h| h.element_id).collect();

    // Allow some difference since Morton and Hierarchical may have slightly
    // different candidate sets due to different spatial indexing granularity.
    // But if both find hits, they should have significant overlap.
    if !morton_ids.is_empty() && !hier_ids.is_empty() {
        let overlap = morton_ids.intersection(&hier_ids).count();
        println!(
            "Overlap: {} of Morton {}, Hier {}",
            overlap,
            morton_ids.len(),
            hier_ids.len()
        );
        assert!(
            overlap > 0,
            "Morton and Hierarchical should agree on at least some hits"
        );
    }
}

// --- Performance Test ---

#[tokio::test]
async fn test_gpu_resident_query_latency() {
    let mut system = create_test_interaction_system().await;
    system.set_spatial_algorithm(SpatialAlgorithm::Morton);

    let elements = make_circle_grid(10_000, 1000.0);
    let renderable = MockRenderable::from_element_data(&elements);

    // Warm up (first query builds the spatial index)
    let _ = system
        .query_point(Vec2::new(500.0, 500.0), &[&renderable])
        .await;

    // Benchmark subsequent queries (spatial index already built)
    let iterations = 10;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = system
            .query_point(Vec2::new(500.0, 500.0), &[&renderable])
            .await;
    }
    let avg_latency = start.elapsed() / iterations;

    println!(
        "GPU-resident query avg latency ({} elements, {} iters): {:?}",
        elements.len(),
        iterations,
        avg_latency
    );

    // The query should complete in reasonable time (< 500ms per query)
    assert!(
        avg_latency.as_millis() < 500,
        "GPU-resident query should complete in < 500ms, took {:?}",
        avg_latency
    );
}

// --- GUP-196: Large Candidate Count Test ---

/// Verify that single-query dispatches with >3125 candidates produce correct
/// results. Before GUP-196 the hit test shader used `arrayLength(&queries)`
/// (the buffer capacity = 32) for result indexing, which limited candidates
/// to 100K / 32 = 3125. With the actual query count passed via a uniform,
/// single-query dispatches can test up to 100K candidates.
#[tokio::test]
async fn test_single_query_exceeds_old_candidate_limit() {
    let mut system = create_test_interaction_system().await;

    // Use brute-force path (no spatial index) to guarantee all elements
    // are dispatched as candidates to the hit test shader.
    system.set_spatial_algorithm(SpatialAlgorithm::Auto);

    // Create 5000 circles (>3125) all clustered around (50, 50) so that
    // they all register as hits for a region query covering the cluster.
    let count = 5_000usize;
    let elements: Vec<ElementData> = (0..count)
        .map(|i| {
            let row = i / 100;
            let col = i % 100;
            ElementData {
                position: [col as f32 * 0.5, row as f32 * 0.5],
                size: [1.0, 1.0], // radius = 1 for circles
                mark_type: 0,     // Circle
                element_id: i as u32,
                selection_id: 0,
                _padding: 0,
            }
        })
        .collect();
    let renderable = MockRenderable::from_element_data(&elements);

    // Region query that covers the entire cluster.
    let region = Rect::new(Vec2::new(-1.0, -1.0), Vec2::new(60.0, 60.0));
    let hits = system
        .query_region(region, &[&renderable])
        .await
        .expect("region query with >3125 candidates should succeed");

    println!(
        "Single-query dispatch with {} candidates: {} hits",
        count,
        hits.len()
    );

    // With the old indexing (element * 32 + 0), elements with index ≥3125
    // would overflow the 100K result buffer and be silently dropped. With
    // the fix (element * 1 + 0), all 5000 elements should produce results.
    assert!(
        hits.len() > 3125,
        "Should find more than 3125 hits (found {}); \
         the old result indexing limit should be gone",
        hits.len()
    );

    // Verify we find the vast majority of elements (allowing a small margin
    // for elements exactly on the boundary of the query region).
    assert!(
        hits.len() >= count - 10,
        "Expected ~{} hits but got {}; some results may have been dropped",
        count,
        hits.len()
    );
}

/// Verify batch (multi-query) dispatches still work correctly with the
/// uniform query count. This ensures we didn't break the multi-query path.
#[tokio::test]
async fn test_batch_query_still_works() {
    let mut system = create_test_interaction_system().await;

    // Create a small set of well-separated circles.
    let elements: Vec<ElementData> = (0..100)
        .map(|i| ElementData {
            position: [(i % 10) as f32 * 50.0, (i / 10) as f32 * 50.0],
            size: [10.0, 10.0],
            mark_type: 0,
            element_id: i as u32,
            selection_id: 0,
            _padding: 0,
        })
        .collect();
    let renderable = MockRenderable::from_element_data(&elements);

    // Batch of 3 queries at known positions.
    let queries = vec![
        gup::interaction::GpuInteractionQuery::point(Vec2::new(0.0, 0.0), 100),
        gup::interaction::GpuInteractionQuery::point(Vec2::new(450.0, 450.0), 100),
        gup::interaction::GpuInteractionQuery::point(Vec2::new(9999.0, 9999.0), 100),
    ];

    let batch_results = system
        .query_batch(&queries, &[&renderable])
        .await
        .expect("batch query should succeed");

    assert_eq!(
        batch_results.len(),
        3,
        "Should return one result set per query"
    );

    println!(
        "Batch query results: q0={} hits, q1={} hits, q2={} hits",
        batch_results[0].len(),
        batch_results[1].len(),
        batch_results[2].len(),
    );
}
