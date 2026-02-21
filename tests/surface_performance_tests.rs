// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for surface performance optimization features (GUP-049).

use gup::context::{GupContext, RenderPriority, SurfaceId, SurfaceRenderConfig};
use std::sync::Arc;

#[tokio::test]
async fn test_surface_render_config() {
    let _context = GupContext::new().await.expect("Failed to create context");

    // Create a config with custom settings
    let config = SurfaceRenderConfig {
        target_fps: Some(30.0),
        priority: RenderPriority::Background,
        frame_skipping_enabled: true,
        resource_pool_size: 16,
    };

    // Note: We can't actually set config without a surface, but we can test the struct
    assert_eq!(config.target_fps, Some(30.0));
    assert_eq!(config.priority, RenderPriority::Background);
    assert!(config.frame_skipping_enabled);
    assert_eq!(config.resource_pool_size, 16);
}

#[tokio::test]
async fn test_render_priority_ordering() {
    // Verify priority ordering
    assert!(RenderPriority::Foreground > RenderPriority::Background);
    assert!(RenderPriority::Background > RenderPriority::Minimized);
    assert!(RenderPriority::Foreground > RenderPriority::Minimized);
}

#[tokio::test]
async fn test_default_render_config() {
    let config = SurfaceRenderConfig::default();
    assert_eq!(config.target_fps, Some(60.0));
    assert_eq!(config.priority, RenderPriority::Foreground);
    assert!(config.frame_skipping_enabled);
    assert_eq!(config.resource_pool_size, 8);
}

#[tokio::test]
async fn test_multi_surface_stats() {
    let context = GupContext::new().await.expect("Failed to create context");
    let stats = context.get_render_statistics();

    // Initial stats should be empty
    assert_eq!(stats.total_frames, 0);
    assert_eq!(stats.total_skipped, 0);
    assert!(stats.scheduling_overhead < 0.02); // <2% overhead
}

#[tokio::test]
async fn test_memory_optimization() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");

    // Should not panic
    context_mut
        .optimize_memory_usage()
        .expect("Memory optimization failed");
}

#[tokio::test]
async fn test_should_render_surface_invalid_id() {
    let context = GupContext::new().await.expect("Failed to create context");
    let invalid_id = SurfaceId::new();

    // Should return false for non-existent surface
    assert!(!context.should_render_surface(invalid_id));
}

#[tokio::test]
async fn test_render_config_cloning() {
    let config1 = SurfaceRenderConfig {
        target_fps: Some(120.0),
        priority: RenderPriority::Foreground,
        frame_skipping_enabled: false,
        resource_pool_size: 32,
    };

    let config2 = config1.clone();
    assert_eq!(config1.target_fps, config2.target_fps);
    assert_eq!(config1.priority, config2.priority);
    assert_eq!(
        config1.frame_skipping_enabled,
        config2.frame_skipping_enabled
    );
    assert_eq!(config1.resource_pool_size, config2.resource_pool_size);
}

#[tokio::test]
async fn test_render_priority_default() {
    let priority = RenderPriority::default();
    assert_eq!(priority, RenderPriority::Background);
}

#[tokio::test]
async fn test_surface_stats_cloning() {
    let context = GupContext::new().await.expect("Failed to create context");
    let stats1 = context.get_render_statistics();
    let stats2 = stats1.clone();

    assert_eq!(stats1.total_frames, stats2.total_frames);
    assert_eq!(stats1.total_skipped, stats2.total_skipped);
    assert_eq!(stats1.scheduling_overhead, stats2.scheduling_overhead);
}
