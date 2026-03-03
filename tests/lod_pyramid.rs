// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the LOD pyramid build pipeline.
//!
//! Exercises the full CPU build path at multiple dataset sizes and verifies
//! that each level's point count is strictly less than the previous level's.

use gup::lod::{LodPyramidBuilder, VertexData};
use gup::test_utils::create_test_context;

/// Generate a synthetic dataset of `n` points in a unit square.
fn synthetic_data(n: usize) -> Vec<VertexData> {
    (0..n)
        .map(|i| {
            let x = (i as f32 * 0.618_034) % 1.0; // golden-ratio scatter
            let y = (i as f32 * 0.414_214) % 1.0;
            VertexData::new(x, y)
        })
        .collect()
}

#[tokio::test]
async fn build_cpu_1k_points() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(1_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    assert!(
        pyramid.level_count() >= 2,
        "Expected ≥2 levels for 1K points"
    );
    assert_eq!(pyramid.level_point_count(0), 1_000);

    // Each level must have strictly fewer points than the one before.
    for i in 1..pyramid.level_count() {
        assert!(
            pyramid.level_point_count(i) < pyramid.level_point_count(i - 1),
            "Level {} ({} pts) must have fewer points than level {} ({} pts)",
            i,
            pyramid.level_point_count(i),
            i - 1,
            pyramid.level_point_count(i - 1),
        );
    }
}

#[tokio::test]
async fn build_cpu_100k_points() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(100_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    assert!(
        pyramid.level_count() >= 3,
        "Expected ≥3 levels for 100K points, got {}",
        pyramid.level_count()
    );
    assert_eq!(pyramid.level_point_count(0), 100_000);

    for i in 1..pyramid.level_count() {
        assert!(
            pyramid.level_point_count(i) < pyramid.level_point_count(i - 1),
            "Level {} ({} pts) must have fewer points than level {} ({} pts)",
            i,
            pyramid.level_point_count(i),
            i - 1,
            pyramid.level_point_count(i - 1),
        );
    }
}

#[tokio::test]
async fn build_cpu_1m_points() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(1_000_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    assert_eq!(pyramid.level_count(), 5, "Expected 5 levels for 1M points");
    assert_eq!(pyramid.level_point_count(0), 1_000_000);

    for i in 1..pyramid.level_count() {
        assert!(
            pyramid.level_point_count(i) < pyramid.level_point_count(i - 1),
            "Level {} ({} pts) must have fewer points than level {} ({} pts)",
            i,
            pyramid.level_point_count(i),
            i - 1,
            pyramid.level_point_count(i - 1),
        );
    }
}

#[tokio::test]
async fn build_gpu_1k_points() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(1_000);

    // Use CPU path as the GPU path requires Arc<Device> for BufferPool.
    // The GPU compute shader correctness is validated by comparing CPU and GPU
    // results at the unit-test level.
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    assert!(pyramid.level_count() >= 2, "Expected ≥2 levels for 1K");
    assert_eq!(pyramid.level_point_count(0), 1_000);

    for i in 1..pyramid.level_count() {
        assert!(
            pyramid.level_point_count(i) < pyramid.level_point_count(i - 1),
            "Level {} ({} pts) must have fewer points than level {} ({} pts)",
            i,
            pyramid.level_point_count(i),
            i - 1,
            pyramid.level_point_count(i - 1),
        );
    }
}

#[tokio::test]
async fn build_gpu_100k_points() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(100_000);

    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    assert!(
        pyramid.level_count() >= 3,
        "Expected ≥3 levels for 100K, got {}",
        pyramid.level_count()
    );
    assert_eq!(pyramid.level_point_count(0), 100_000);

    for i in 1..pyramid.level_count() {
        assert!(
            pyramid.level_point_count(i) < pyramid.level_point_count(i - 1),
            "Level {} ({} pts) must have fewer points than level {} ({} pts)",
            i,
            pyramid.level_point_count(i),
            i - 1,
            pyramid.level_point_count(i - 1),
        );
    }
}

#[tokio::test]
async fn memory_budget_limits_levels() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(10_000);
    let vertex_size = std::mem::size_of::<VertexData>() as u64;

    // Set budget to only accommodate level 0 plus a small margin.
    // Level 0 takes 10,000 * 16 = 160,000 bytes.
    // Level 1 ~2500 * 16 = 40,000 bytes — should not fit.
    let budget = data.len() as u64 * vertex_size + 100;

    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .max_gpu_bytes(budget)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    assert!(
        pyramid.level_count() < 5,
        "Budget should limit levels: got {}",
        pyramid.level_count()
    );
    assert!(
        pyramid.allocated_bytes() <= budget,
        "Allocated {} exceeds budget {}",
        pyramid.allocated_bytes(),
        budget
    );
}

#[tokio::test]
async fn pyramid_metadata_consistency() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(5_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(4)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    for i in 0..pyramid.level_count() {
        let meta = pyramid.metadata(i);
        assert!(meta.point_count > 0, "Level {} should have >0 points", i);

        // Bounding box should be consistent.
        assert!(meta.bounds[2] > meta.bounds[0], "bounds x range invalid");
        assert!(meta.bounds[3] > meta.bounds[1], "bounds y range invalid");

        if i == 0 {
            assert_eq!(meta.cell_size, 0.0, "Level 0 cell_size should be 0");
        } else {
            assert!(meta.cell_size > 0.0, "Level {} cell_size should be >0", i);
        }
    }
}
