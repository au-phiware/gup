// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for the spatial index system (GUP-076).
//!
//! These tests validate that:
//! - Spatial index bind groups can be successfully created
//! - CPU-side spatial index building is correct
//! - The interaction system works with spatial indexing enabled
//! - Buffer layouts match between Rust and WGSL

use gup::interaction::{ElementData, InteractionSystem, SpatialCell, SpatialIndexConfig};
use gup::test_utils::create_test_context;

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

/// Helper to create element data spread across the coordinate space.
fn make_elements(count: usize, spread: f32) -> Vec<ElementData> {
    (0..count)
        .map(|i| {
            let t = i as f32 / count.max(1) as f32;
            ElementData {
                position: [t * spread, t * spread],
                size: [5.0, 5.0],
                mark_type: 0,
                element_id: i as u32,
                selection_id: 0,
                _padding: 0,
            }
        })
        .collect()
}

/// Helper to create clustered element data in specific cells.
fn make_clustered_elements(clusters: &[([f32; 2], usize)], // (center, count)
) -> Vec<ElementData> {
    let mut elements = Vec::new();
    let mut id = 0u32;
    for &(center, count) in clusters {
        for j in 0..count {
            elements.push(ElementData {
                position: [center[0] + (j as f32 * 0.1), center[1] + (j as f32 * 0.1)],
                size: [5.0, 5.0],
                mark_type: 0,
                element_id: id,
                selection_id: 0,
                _padding: 0,
            });
            id += 1;
        }
    }
    elements
}

// --- GPU Integration Tests ---

#[tokio::test]
async fn test_interaction_system_creation_with_spatial_pipelines() {
    // This test validates AC: "Spatial index compute pipeline successfully creates bind groups"
    // If the explicit bind group layout is wrong, this will panic during pipeline creation.
    let system = create_test_interaction_system().await;
    assert!(
        !system.is_spatial_index_built(),
        "Spatial index should not be built initially"
    );
}

#[tokio::test]
async fn test_spatial_index_build_empty() {
    let mut system = create_test_interaction_system().await;
    let result = system.build_spatial_index_from_elements(&[]).await;
    assert!(
        result.is_ok(),
        "Building spatial index with empty data should succeed"
    );
    // Empty data means the index isn't "built" because the early return fires
    assert!(!system.is_spatial_index_built());
}

#[tokio::test]
async fn test_spatial_index_build_small_dataset() {
    let mut system = create_test_interaction_system().await;
    let elements = make_elements(10, 100.0);

    let result = system.build_spatial_index_from_elements(&elements).await;
    assert!(result.is_ok(), "Spatial index build failed: {result:?}");
    assert!(system.is_spatial_index_built());

    let config = system.spatial_config();
    // Bounds should encompass all element positions with padding
    assert!(
        config.world_bounds_min[0] < 0.0,
        "min x should have padding"
    );
    assert!(
        config.world_bounds_max[0] > 90.0,
        "max x should encompass data"
    );
}

#[tokio::test]
async fn test_spatial_index_build_large_dataset() {
    let mut system = create_test_interaction_system().await;
    let elements = make_elements(5_000, 1000.0);

    let result = system.build_spatial_index_from_elements(&elements).await;
    assert!(
        result.is_ok(),
        "Spatial index build failed for 5K elements: {result:?}"
    );
    assert!(system.is_spatial_index_built());
}

#[tokio::test]
async fn test_spatial_index_build_clustered_data() {
    let mut system = create_test_interaction_system().await;
    let elements = make_clustered_elements(&[
        ([100.0, 100.0], 50),
        ([500.0, 500.0], 30),
        ([900.0, 900.0], 20),
    ]);

    let result = system.build_spatial_index_from_elements(&elements).await;
    assert!(
        result.is_ok(),
        "Clustered spatial index build failed: {result:?}"
    );
    assert!(system.is_spatial_index_built());
}

#[tokio::test]
async fn test_spatial_index_invalidation() {
    let mut system = create_test_interaction_system().await;
    let elements = make_elements(10, 100.0);

    system
        .build_spatial_index_from_elements(&elements)
        .await
        .unwrap();
    assert!(system.is_spatial_index_built());

    system.invalidate_spatial_index();
    assert!(
        !system.is_spatial_index_built(),
        "Invalidation should clear built flag"
    );
}

#[tokio::test]
async fn test_spatial_index_rebuild_with_new_data() {
    let mut system = create_test_interaction_system().await;

    // Build with first dataset
    let elements1 = make_elements(10, 100.0);
    system
        .build_spatial_index_from_elements(&elements1)
        .await
        .unwrap();
    let config1_max = system.spatial_config().world_bounds_max;

    // Invalidate and rebuild with different data
    system.invalidate_spatial_index();
    let elements2 = make_elements(10, 500.0);
    system
        .build_spatial_index_from_elements(&elements2)
        .await
        .unwrap();
    let config2_max = system.spatial_config().world_bounds_max;

    // Bounds should differ since data spread is different
    assert!(
        config2_max[0] > config1_max[0],
        "Rebuilt index should reflect new data bounds"
    );
}

// --- Struct Alignment Tests (complement the unit tests) ---

#[test]
fn test_spatial_structs_bytemuck_roundtrip() {
    let config = SpatialIndexConfig {
        grid_size: [100, 100],
        cell_size: [10.0, 10.0],
        world_bounds_min: [-50.0, -50.0],
        world_bounds_max: [950.0, 950.0],
    };
    let bytes = bytemuck::bytes_of(&config);
    let restored: &SpatialIndexConfig = bytemuck::from_bytes(bytes);
    assert_eq!(restored.grid_size, config.grid_size);
    assert_eq!(restored.cell_size, config.cell_size);
    assert_eq!(restored.world_bounds_min, config.world_bounds_min);
    assert_eq!(restored.world_bounds_max, config.world_bounds_max);

    let cell = SpatialCell {
        element_count: 42,
        element_start_index: 100,
        bounds_min: [0.0, 0.0],
        bounds_max: [10.0, 10.0],
    };
    let bytes = bytemuck::bytes_of(&cell);
    let restored: &SpatialCell = bytemuck::from_bytes(bytes);
    assert_eq!(restored.element_count, 42);
    assert_eq!(restored.element_start_index, 100);
    assert_eq!(restored.bounds_min, [0.0, 0.0]);
    assert_eq!(restored.bounds_max, [10.0, 10.0]);
}

#[test]
fn test_spatial_index_config_wgsl_alignment() {
    // WGSL SpatialIndex struct layout:
    //   grid_size:        vec2<u32>  @ offset 0  (align 8, size 8)
    //   cell_size:        vec2<f32>  @ offset 8  (align 8, size 8)
    //   world_bounds_min: vec2<f32>  @ offset 16 (align 8, size 8)
    //   world_bounds_max: vec2<f32>  @ offset 24 (align 8, size 8)
    //   Total: 32 bytes
    assert_eq!(std::mem::size_of::<SpatialIndexConfig>(), 32);
    assert_eq!(std::mem::align_of::<SpatialIndexConfig>(), 4);

    // Validate that the Rust layout matches WGSL expectations
    use std::mem::offset_of;
    assert_eq!(offset_of!(SpatialIndexConfig, grid_size), 0);
    assert_eq!(offset_of!(SpatialIndexConfig, cell_size), 8);
    assert_eq!(offset_of!(SpatialIndexConfig, world_bounds_min), 16);
    assert_eq!(offset_of!(SpatialIndexConfig, world_bounds_max), 24);
}

#[test]
fn test_spatial_cell_wgsl_alignment() {
    // WGSL SpatialCell struct layout:
    //   element_count:       u32        @ offset 0  (align 4, size 4)
    //   element_start_index: u32        @ offset 4  (align 4, size 4)
    //   bounds_min:          vec2<f32>  @ offset 8  (align 8, size 8)
    //   bounds_max:          vec2<f32>  @ offset 16 (align 8, size 8)
    //   Total: 24 bytes
    assert_eq!(std::mem::size_of::<SpatialCell>(), 24);

    use std::mem::offset_of;
    assert_eq!(offset_of!(SpatialCell, element_count), 0);
    assert_eq!(offset_of!(SpatialCell, element_start_index), 4);
    assert_eq!(offset_of!(SpatialCell, bounds_min), 8);
    assert_eq!(offset_of!(SpatialCell, bounds_max), 16);
}

// --- Adaptive grid size tests (GUP-176) ---

#[tokio::test]
async fn test_adaptive_grid_small_dataset() {
    let mut system = create_test_interaction_system().await;
    let elements = make_elements(10, 100.0);

    system
        .build_spatial_index_from_elements(&elements)
        .await
        .unwrap();
    let config = system.spatial_config();

    // 10 elements → √10 ≈ 3.16, below MIN_GRID_SIDE (4)
    assert_eq!(config.grid_size, [4, 4], "tiny dataset should use minimum grid");
}

#[tokio::test]
async fn test_adaptive_grid_medium_dataset() {
    let mut system = create_test_interaction_system().await;
    let elements = make_elements(100, 500.0);

    system
        .build_spatial_index_from_elements(&elements)
        .await
        .unwrap();
    let config = system.spatial_config();

    // 100 elements → √100 = 10
    assert_eq!(config.grid_size, [10, 10], "100 elements should use 10×10 grid");
}

#[tokio::test]
async fn test_adaptive_grid_large_dataset() {
    let mut system = create_test_interaction_system().await;
    let elements = make_elements(5_000, 1000.0);

    system
        .build_spatial_index_from_elements(&elements)
        .await
        .unwrap();
    let config = system.spatial_config();

    // 5000 elements → √5000 ≈ 70.7, ceil → 71
    // max_spatial_cells = 10_000 → max_side = 100, so 71 fits
    assert_eq!(config.grid_size[0], 71);
    assert_eq!(config.grid_size[1], 71);
}

#[tokio::test]
async fn test_adaptive_grid_different_sizes_produce_different_grids() {
    let mut system = create_test_interaction_system().await;

    // Build with small dataset
    let small = make_elements(10, 100.0);
    system
        .build_spatial_index_from_elements(&small)
        .await
        .unwrap();
    let small_grid = system.spatial_config().grid_size;

    // Rebuild with large dataset
    system.invalidate_spatial_index();
    let large = make_elements(2_500, 1000.0);
    system
        .build_spatial_index_from_elements(&large)
        .await
        .unwrap();
    let large_grid = system.spatial_config().grid_size;

    assert!(
        large_grid[0] > small_grid[0],
        "larger dataset should produce finer grid: small={small_grid:?} large={large_grid:?}"
    );
}
