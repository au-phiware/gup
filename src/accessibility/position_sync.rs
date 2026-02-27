// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Position Synchronization for DOM Overlay
//!
//! This module provides coordinate transformation and position synchronization
//! between GPU-rendered marks and DOM overlay elements. It handles:
//! - Querying mark positions from GPU buffers
//! - Transforming GPU coordinates to screen coordinates
//! - Updating overlay element positions
//! - Handling viewport changes (pan, zoom, resize)

#[cfg(target_arch = "wasm32")]
use crate::accessibility::aria::NodeId;
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;

/// Position in screen space (pixels from top-left)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPosition {
    pub x: f32,
    pub y: f32,
}

/// Position in GPU/normalized device coordinates (-1 to 1)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuPosition {
    pub x: f32,
    pub y: f32,
}

/// Viewport transformation for coordinate conversion
#[derive(Debug, Clone, Copy)]
pub struct ViewportTransform {
    /// Width of the viewport in pixels
    pub width: f32,
    /// Height of the viewport in pixels
    pub height: f32,
    /// X offset for panning
    pub pan_x: f32,
    /// Y offset for panning
    pub pan_y: f32,
    /// Zoom level (1.0 = no zoom)
    pub zoom: f32,
}

impl Default for ViewportTransform {
    fn default() -> Self {
        Self {
            width: 800.0,
            height: 600.0,
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl ViewportTransform {
    /// Create a new viewport transform with the given dimensions
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Apply pan offset to the transform
    pub fn with_pan(mut self, pan_x: f32, pan_y: f32) -> Self {
        self.pan_x = pan_x;
        self.pan_y = pan_y;
        self
    }

    /// Apply zoom level to the transform
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    /// Transform GPU coordinates to screen coordinates
    ///
    /// GPU coordinates are in normalized device space (-1 to 1, y-up)
    /// Screen coordinates are in pixels (0 to width/height, y-down)
    pub fn gpu_to_screen(&self, gpu_pos: GpuPosition) -> ScreenPosition {
        // Apply zoom
        let zoomed_x = gpu_pos.x * self.zoom;
        let zoomed_y = gpu_pos.y * self.zoom;

        // Convert from NDC (-1 to 1) to pixel coordinates (0 to width/height)
        // Note: GPU y-axis points up, screen y-axis points down
        let x = ((zoomed_x + 1.0) * 0.5 * self.width) + self.pan_x;
        let y = ((1.0 - zoomed_y) * 0.5 * self.height) + self.pan_y;

        ScreenPosition { x, y }
    }

    /// Transform screen coordinates to GPU coordinates
    pub fn screen_to_gpu(&self, screen_pos: ScreenPosition) -> GpuPosition {
        // Remove pan offset
        let x = screen_pos.x - self.pan_x;
        let y = screen_pos.y - self.pan_y;

        // Convert from pixels to NDC (-1 to 1)
        let ndc_x = (x / self.width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (y / self.height) * 2.0; // Flip y-axis

        // Apply zoom (inverse)
        let gpu_x = ndc_x / self.zoom;
        let gpu_y = ndc_y / self.zoom;

        GpuPosition { x: gpu_x, y: gpu_y }
    }
}

/// Position manager for tracking element positions
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default)]
pub struct PositionManager {
    /// Map of node IDs to their GPU positions
    positions: HashMap<NodeId, GpuPosition>,
    /// Current viewport transform
    transform: ViewportTransform,
    /// Whether positions have changed and need update
    dirty: bool,
}

#[cfg(target_arch = "wasm32")]
impl PositionManager {
    /// Create a new position manager
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            transform: ViewportTransform::default(),
            dirty: false,
        }
    }

    /// Update the viewport transform
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.transform.width = width;
        self.transform.height = height;
        self.dirty = true;
    }

    /// Update pan offset
    pub fn set_pan(&mut self, pan_x: f32, pan_y: f32) {
        self.transform.pan_x = pan_x;
        self.transform.pan_y = pan_y;
        self.dirty = true;
    }

    /// Update zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.transform.zoom = zoom;
        self.dirty = true;
    }

    /// Set the GPU position for a node
    pub fn set_position(&mut self, node_id: NodeId, position: GpuPosition) {
        self.positions.insert(node_id, position);
        self.dirty = true;
    }

    /// Remove a node's position
    pub fn remove_position(&mut self, node_id: NodeId) {
        self.positions.remove(&node_id);
    }

    /// Get the screen position for a node
    pub fn get_screen_position(&self, node_id: NodeId) -> Option<ScreenPosition> {
        self.positions
            .get(&node_id)
            .map(|gpu_pos| self.transform.gpu_to_screen(*gpu_pos))
    }

    /// Get the current viewport transform
    pub fn transform(&self) -> &ViewportTransform {
        &self.transform
    }

    /// Check if positions have changed and need update
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark positions as clean after update
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Get all node IDs with positions
    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.positions.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_transform_center() {
        let transform = ViewportTransform::new(800.0, 600.0);

        // Center of GPU space (0, 0) should map to center of screen
        let gpu_center = GpuPosition { x: 0.0, y: 0.0 };
        let screen_center = transform.gpu_to_screen(gpu_center);
        assert!((screen_center.x - 400.0).abs() < 0.01);
        assert!((screen_center.y - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_viewport_transform_corners() {
        let transform = ViewportTransform::new(800.0, 600.0);

        // Top-left corner in GPU space (-1, 1) should map to (0, 0)
        let gpu_top_left = GpuPosition { x: -1.0, y: 1.0 };
        let screen_top_left = transform.gpu_to_screen(gpu_top_left);
        assert!((screen_top_left.x - 0.0).abs() < 0.01);
        assert!((screen_top_left.y - 0.0).abs() < 0.01);

        // Bottom-right corner in GPU space (1, -1) should map to (800, 600)
        let gpu_bottom_right = GpuPosition { x: 1.0, y: -1.0 };
        let screen_bottom_right = transform.gpu_to_screen(gpu_bottom_right);
        assert!((screen_bottom_right.x - 800.0).abs() < 0.01);
        assert!((screen_bottom_right.y - 600.0).abs() < 0.01);
    }

    #[test]
    fn test_viewport_transform_with_zoom() {
        let transform = ViewportTransform::new(800.0, 600.0).with_zoom(2.0);

        // With 2x zoom, center point stays at center
        let gpu_center = GpuPosition { x: 0.0, y: 0.0 };
        let screen_center = transform.gpu_to_screen(gpu_center);
        assert!((screen_center.x - 400.0).abs() < 0.01);
        assert!((screen_center.y - 300.0).abs() < 0.01);

        // But a point at 0.5 in GPU space moves further out
        let gpu_offset = GpuPosition { x: 0.5, y: 0.0 };
        let screen_offset = transform.gpu_to_screen(gpu_offset);
        // 0.5 * 2.0 (zoom) = 1.0, then (1.0 + 1.0) * 0.5 * 800 = 800
        assert!((screen_offset.x - 800.0).abs() < 0.01);
    }

    #[test]
    fn test_viewport_transform_with_pan() {
        let transform = ViewportTransform::new(800.0, 600.0).with_pan(50.0, 30.0);

        // With pan, all positions shift by the pan offset
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

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_position_manager() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::new();

        // Initially no position
        assert!(manager.get_screen_position(node_id).is_none());
        assert!(!manager.is_dirty());

        // Set a position
        manager.set_position(node_id, GpuPosition { x: 0.0, y: 0.0 });
        assert!(manager.is_dirty());
        assert!(manager.get_screen_position(node_id).is_some());

        // Clear dirty flag
        manager.clear_dirty();
        assert!(!manager.is_dirty());

        // Remove position
        manager.remove_position(node_id);
        assert!(manager.get_screen_position(node_id).is_none());
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_position_manager_viewport_changes() {
        let mut manager = PositionManager::new();
        let node_id = NodeId::new();

        manager.set_position(node_id, GpuPosition { x: 0.0, y: 0.0 });
        manager.clear_dirty();

        let screen_pos_before = manager.get_screen_position(node_id).unwrap();

        // Change viewport
        manager.set_viewport(1600.0, 1200.0);
        assert!(manager.is_dirty());

        let screen_pos_after = manager.get_screen_position(node_id).unwrap();

        // Screen position should have changed with viewport
        assert!((screen_pos_after.x - 800.0).abs() < 0.01); // New center is 1600/2
        assert!((screen_pos_after.y - 600.0).abs() < 0.01); // New center is 1200/2
        assert!(screen_pos_after.x != screen_pos_before.x);
    }
}
