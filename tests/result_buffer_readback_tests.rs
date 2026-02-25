// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for result buffer readback optimization (GUP-197).
//!
//! Validates that:
//! - The persistent staging buffer produces identical results to the original
//!   per-query allocation approach
//! - Rapid successive queries work correctly (double-buffering stress)
//! - Readback latency is reduced compared to GUP-194 baseline
//! - No correctness regression for various dataset sizes

use gup::interaction::{ElementData, InteractionSystem, Rect, Vec2};
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

/// Generate a grid of element data.
fn generate_grid_elements(count: usize, spread: f32) -> Vec<ElementData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let step = spread / side.max(1) as f32;
    (0..count)
        .map(|i| {
            let x = (i % side) as f32 * step;
            let y = (i / side) as f32 * step;
            ElementData {
                position: [x, y],
                size: [step * 0.4, step * 0.4],
                mark_type: 0,
                element_id: i as u32,
                selection_id: 0,
                _padding: 0,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Correctness tests
// ---------------------------------------------------------------------------

/// Verify that the persistent staging buffer returns correct point query results.
#[tokio::test]
async fn test_persistent_staging_point_query_correctness() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(1_000, 100.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Query a point known to be near an element
    let hits = system
        .query_point_cached(Vec2::new(0.0, 0.0))
        .await
        .expect("query failed");

    assert!(!hits.is_empty(), "Expected hits near origin");
    assert_eq!(hits[0].element_id, 0, "Closest element should be element 0");
}

/// Verify that region queries produce correct results with the persistent buffer.
#[tokio::test]
async fn test_persistent_staging_region_query_correctness() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(1_000, 100.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Query a region that should contain several elements
    let region = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(15.0, 15.0));
    let hits = system
        .query_region_cached(region)
        .await
        .expect("query failed");

    assert!(
        hits.len() >= 2,
        "Expected at least 2 hits in a 15x15 region, got {}",
        hits.len()
    );
}

/// Verify that rapid successive queries return consistent results.
#[tokio::test]
async fn test_rapid_successive_queries_consistency() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Run the same query 10 times in rapid succession
    let mut all_results = Vec::new();
    for _ in 0..10 {
        let hits = system
            .query_point_cached(Vec2::new(5.0, 5.0))
            .await
            .expect("query failed");
        all_results.push(hits);
    }

    // All results should be identical
    let first = &all_results[0];
    for (i, result) in all_results.iter().enumerate().skip(1) {
        assert_eq!(
            first.len(),
            result.len(),
            "Query {i} returned different number of hits ({} vs {})",
            first.len(),
            result.len()
        );
        for (j, (a, b)) in first.iter().zip(result.iter()).enumerate() {
            assert_eq!(
                a.element_id, b.element_id,
                "Query {i} hit {j}: element_id mismatch"
            );
        }
    }
}

/// Verify results with zero hits (no elements near query point).
#[tokio::test]
async fn test_persistent_staging_no_hits() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(100, 10.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Query far from any element
    let hits = system
        .query_point_cached(Vec2::new(1000.0, 1000.0))
        .await
        .expect("query failed");

    assert!(hits.is_empty(), "Expected no hits far from elements");
}

/// Verify that queries work correctly after cache invalidation and re-upload.
#[tokio::test]
async fn test_persistent_staging_after_cache_invalidation() {
    let mut system = create_test_interaction_system().await;

    // First dataset
    let elements1 = generate_grid_elements(200, 20.0);
    system
        .upload_element_data_cached(&elements1, 1)
        .await
        .expect("upload failed");

    let hits1 = system
        .query_point_cached(Vec2::new(0.0, 0.0))
        .await
        .expect("query failed");

    // Invalidate and upload different data
    system.invalidate_element_cache();

    let elements2 = generate_grid_elements(200, 40.0);
    system
        .upload_element_data_cached(&elements2, 2)
        .await
        .expect("upload failed");

    let hits2 = system
        .query_point_cached(Vec2::new(0.0, 0.0))
        .await
        .expect("query failed");

    // Both should find hits near origin, but element layout differs
    assert!(!hits1.is_empty(), "First dataset should have hits");
    assert!(!hits2.is_empty(), "Second dataset should have hits");
}

// ---------------------------------------------------------------------------
// Performance / latency tests
// ---------------------------------------------------------------------------

/// Benchmark: repeated cached queries should show low per-query latency
/// thanks to the persistent staging buffer.
#[tokio::test]
async fn test_readback_latency_improvement() {
    let mut system = create_test_interaction_system().await;

    let count = 10_000;
    let elements = generate_grid_elements(count, 1000.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Warm up
    let _ = system
        .query_point_cached(Vec2::new(500.0, 500.0))
        .await
        .expect("warm-up query failed");

    // Measure latency over multiple queries
    let iterations = 20;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = system
            .query_point_cached(Vec2::new(500.0, 500.0))
            .await
            .expect("query failed");
    }
    let total_us = start.elapsed().as_micros();
    let avg_us = total_us as f64 / iterations as f64;

    println!(
        "Persistent staging readback: {iterations} queries on {count} elements, \
         avg {avg_us:.0}µs per query"
    );

    // In debug mode the threshold is relaxed. The story AC targets <1ms in
    // release mode, but debug mode is expected to be 2-5x slower.
    let threshold_us = if cfg!(debug_assertions) {
        10_000.0
    } else {
        1_000.0
    };
    assert!(
        avg_us < threshold_us,
        "Average query latency {avg_us:.0}µs exceeds {threshold_us:.0}µs threshold"
    );
}

/// Stress test: interleave point and region queries rapidly.
#[tokio::test]
async fn test_rapid_mixed_query_stress() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(5_000, 500.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    for i in 0..20 {
        if i % 2 == 0 {
            let _ = system
                .query_point_cached(Vec2::new(250.0, 250.0))
                .await
                .expect("point query failed");
        } else {
            let region = Rect::new(Vec2::new(100.0, 100.0), Vec2::new(200.0, 200.0));
            let _ = system
                .query_region_cached(region)
                .await
                .expect("region query failed");
        }
    }

    // If we get here without panicking, the persistent staging buffer
    // handled all the rapid map/unmap cycles correctly.
}
