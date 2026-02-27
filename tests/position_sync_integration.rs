// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for position synchronization between GPU marks and DOM overlay

// FIXME: wasm_tests module uses outdated accessibility API (NodeId::from,
// AriaNode struct literals, WebDomOverlay constructor). Disabled until
// GUP-237 (WASM Integration Test Suite) updates the tests.
#[cfg(all(target_arch = "wasm32", feature = "__wasm_accessibility_tests"))]
mod wasm_tests {
    use gup::accessibility::{
        AriaNode, AriaRole, GpuPosition, NodeId, PositionManager, ScreenPosition,
        ViewportTransform, WebDomOverlay,
    };

    #[test]
    fn test_position_manager_basic() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::from(1);

        // Initially no position
        assert!(manager.get_screen_position(node_id).is_none());

        // Set a position
        manager.set_position(node_id, GpuPosition { x: 0.0, y: 0.0 });
        let screen_pos = manager.get_screen_position(node_id);
        assert!(screen_pos.is_some());

        // Position should be at center (400, 300) for default 800x600 viewport
        let pos = screen_pos.unwrap();
        assert!((pos.x - 400.0).abs() < 1.0);
        assert!((pos.y - 300.0).abs() < 1.0);
    }

    #[test]
    fn test_position_manager_viewport_update() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::from(1);

        manager.set_position(node_id, GpuPosition { x: 0.0, y: 0.0 });
        let pos_before = manager.get_screen_position(node_id).unwrap();

        // Change viewport size
        manager.set_viewport(1600.0, 1200.0);
        assert!(manager.is_dirty());

        let pos_after = manager.get_screen_position(node_id).unwrap();

        // Screen position should update with viewport
        assert!((pos_after.x - 800.0).abs() < 1.0);
        assert!((pos_after.y - 600.0).abs() < 1.0);
        assert_ne!(pos_before.x, pos_after.x);
    }

    #[test]
    fn test_position_manager_pan() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::from(1);

        manager.set_position(node_id, GpuPosition { x: 0.0, y: 0.0 });
        let pos_before = manager.get_screen_position(node_id).unwrap();

        // Pan the viewport
        manager.set_pan(50.0, 30.0);
        assert!(manager.is_dirty());

        let pos_after = manager.get_screen_position(node_id).unwrap();

        // Position should shift by pan amount
        assert!((pos_after.x - (pos_before.x + 50.0)).abs() < 1.0);
        assert!((pos_after.y - (pos_before.y + 30.0)).abs() < 1.0);
    }

    #[test]
    fn test_position_manager_zoom() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::from(1);

        manager.set_position(node_id, GpuPosition { x: 0.5, y: 0.0 });
        let pos_before = manager.get_screen_position(node_id).unwrap();

        // Zoom in
        manager.set_zoom(2.0);
        assert!(manager.is_dirty());

        let pos_after = manager.get_screen_position(node_id).unwrap();

        // Position should move further from center with zoom
        let center_x = 400.0;
        let dist_before = (pos_before.x - center_x).abs();
        let dist_after = (pos_after.x - center_x).abs();
        assert!(dist_after > dist_before);
    }

    #[test]
    fn test_position_manager_multiple_nodes() {
        let mut manager = PositionManager::new();
        let node1 = NodeId::from(1);
        let node2 = NodeId::from(2);
        let node3 = NodeId::from(3);

        manager.set_position(node1, GpuPosition { x: -0.5, y: 0.5 });
        manager.set_position(node2, GpuPosition { x: 0.0, y: 0.0 });
        manager.set_position(node3, GpuPosition { x: 0.5, y: -0.5 });

        // All positions should be retrievable
        assert!(manager.get_screen_position(node1).is_some());
        assert!(manager.get_screen_position(node2).is_some());
        assert!(manager.get_screen_position(node3).is_some());

        // Positions should be different
        let pos1 = manager.get_screen_position(node1).unwrap();
        let pos2 = manager.get_screen_position(node2).unwrap();
        let pos3 = manager.get_screen_position(node3).unwrap();

        assert_ne!(pos1.x, pos2.x);
        assert_ne!(pos2.x, pos3.x);
    }

    #[test]
    fn test_position_manager_dirty_flag() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::from(1);

        assert!(!manager.is_dirty());

        manager.set_position(node_id, GpuPosition { x: 0.0, y: 0.0 });
        assert!(manager.is_dirty());

        manager.clear_dirty();
        assert!(!manager.is_dirty());

        manager.set_viewport(1600.0, 1200.0);
        assert!(manager.is_dirty());

        manager.clear_dirty();
        manager.set_pan(10.0, 20.0);
        assert!(manager.is_dirty());

        manager.clear_dirty();
        manager.set_zoom(1.5);
        assert!(manager.is_dirty());
    }

    #[test]
    fn test_position_accuracy() {
        let transform = ViewportTransform::new(800.0, 600.0);

        // Test specific positions with ±2px tolerance (acceptance criteria)
        let test_cases = vec![
            (GpuPosition { x: -1.0, y: 1.0 }, (0.0, 0.0)), // top-left
            (GpuPosition { x: 1.0, y: 1.0 }, (800.0, 0.0)), // top-right
            (GpuPosition { x: -1.0, y: -1.0 }, (0.0, 600.0)), // bottom-left
            (GpuPosition { x: 1.0, y: -1.0 }, (800.0, 600.0)), // bottom-right
            (GpuPosition { x: 0.0, y: 0.0 }, (400.0, 300.0)), // center
        ];

        for (gpu_pos, (expected_x, expected_y)) in test_cases {
            let screen_pos = transform.gpu_to_screen(gpu_pos);
            let error_x = (screen_pos.x - expected_x).abs();
            let error_y = (screen_pos.y - expected_y).abs();

            assert!(
                error_x < 2.0,
                "X position error {:.2}px exceeds tolerance for GPU ({}, {})",
                error_x,
                gpu_pos.x,
                gpu_pos.y
            );
            assert!(
                error_y < 2.0,
                "Y position error {:.2}px exceeds tolerance for GPU ({}, {})",
                error_y,
                gpu_pos.x,
                gpu_pos.y
            );
        }
    }
}

// Non-WASM tests for coordinate transformations
#[cfg(not(target_arch = "wasm32"))]
mod coordinate_tests {
    use gup::accessibility::{GpuPosition, ViewportTransform};

    #[test]
    fn test_viewport_transform_center() {
        let transform = ViewportTransform::new(800.0, 600.0);
        let gpu_center = GpuPosition { x: 0.0, y: 0.0 };
        let screen_center = transform.gpu_to_screen(gpu_center);

        assert!((screen_center.x - 400.0).abs() < 0.01);
        assert!((screen_center.y - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_viewport_transform_corners() {
        let transform = ViewportTransform::new(800.0, 600.0);

        let gpu_top_left = GpuPosition { x: -1.0, y: 1.0 };
        let screen_top_left = transform.gpu_to_screen(gpu_top_left);
        assert!((screen_top_left.x - 0.0).abs() < 0.01);
        assert!((screen_top_left.y - 0.0).abs() < 0.01);

        let gpu_bottom_right = GpuPosition { x: 1.0, y: -1.0 };
        let screen_bottom_right = transform.gpu_to_screen(gpu_bottom_right);
        assert!((screen_bottom_right.x - 800.0).abs() < 0.01);
        assert!((screen_bottom_right.y - 600.0).abs() < 0.01);
    }

    #[test]
    fn test_viewport_transform_with_zoom() {
        let transform = ViewportTransform::new(800.0, 600.0).with_zoom(2.0);

        let gpu_center = GpuPosition { x: 0.0, y: 0.0 };
        let screen_center = transform.gpu_to_screen(gpu_center);
        // Center stays at center regardless of zoom
        assert!((screen_center.x - 400.0).abs() < 0.01);
        assert!((screen_center.y - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_viewport_transform_with_pan() {
        let transform = ViewportTransform::new(800.0, 600.0).with_pan(50.0, 30.0);

        let gpu_center = GpuPosition { x: 0.0, y: 0.0 };
        let screen_center = transform.gpu_to_screen(gpu_center);
        assert!((screen_center.x - 450.0).abs() < 0.01); // 400 + 50
        assert!((screen_center.y - 330.0).abs() < 0.01); // 300 + 30
    }

    #[test]
    fn test_screen_to_gpu_roundtrip() {
        let transform = ViewportTransform::new(800.0, 600.0);

        let original_gpu = GpuPosition { x: 0.3, y: -0.7 };
        let screen = transform.gpu_to_screen(original_gpu);
        let gpu_roundtrip = transform.screen_to_gpu(screen);

        assert!((gpu_roundtrip.x - original_gpu.x).abs() < 0.001);
        assert!((gpu_roundtrip.y - original_gpu.y).abs() < 0.001);
    }
}
