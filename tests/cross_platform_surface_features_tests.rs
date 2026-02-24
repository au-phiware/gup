// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for cross-platform surface features and configuration.
//!
//! These tests validate platform-specific surface capabilities, configuration
//! builders, and advanced display features.

use gup::context::{PlatformSurfaceCapabilities, SurfaceConfigBuilder};
use wgpu::{CompositeAlphaMode, PresentMode, TextureFormat};

#[tokio::test]
async fn test_surface_config_builder_defaults() {
    let config = SurfaceConfigBuilder::new();

    assert_eq!(config.width, 800);
    assert_eq!(config.height, 600);
    assert!(config.present_mode.is_none());
    assert!(config.alpha_mode.is_none());
    assert!(config.format.is_none());
    assert!(config.view_formats.is_empty());
    assert!(config.desired_maximum_frame_latency.is_none());
}

#[tokio::test]
async fn test_surface_config_builder_with_size() {
    let config = SurfaceConfigBuilder::new().with_size(1920, 1080);

    assert_eq!(config.width, 1920);
    assert_eq!(config.height, 1080);
}

#[tokio::test]
async fn test_surface_config_builder_with_present_mode() {
    let config = SurfaceConfigBuilder::new().with_present_mode(PresentMode::Immediate);

    assert_eq!(config.present_mode, Some(PresentMode::Immediate));
}

#[tokio::test]
async fn test_surface_config_builder_with_alpha_mode() {
    let config = SurfaceConfigBuilder::new().with_alpha_mode(CompositeAlphaMode::Opaque);

    assert_eq!(config.alpha_mode, Some(CompositeAlphaMode::Opaque));
}

#[tokio::test]
async fn test_surface_config_builder_with_format() {
    let config = SurfaceConfigBuilder::new().with_format(TextureFormat::Bgra8UnormSrgb);

    assert_eq!(config.format, Some(TextureFormat::Bgra8UnormSrgb));
}

#[tokio::test]
async fn test_surface_config_builder_with_view_formats() {
    let view_formats = vec![TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm];
    let config = SurfaceConfigBuilder::new().with_view_formats(view_formats.clone());

    assert_eq!(config.view_formats, view_formats);
}

#[tokio::test]
async fn test_surface_config_builder_with_frame_latency() {
    let config = SurfaceConfigBuilder::new().with_frame_latency(1);
    assert_eq!(config.desired_maximum_frame_latency, Some(1));

    // Test clamping
    let config_high = SurfaceConfigBuilder::new().with_frame_latency(10);
    assert_eq!(config_high.desired_maximum_frame_latency, Some(3));

    let config_low = SurfaceConfigBuilder::new().with_frame_latency(0);
    assert_eq!(config_low.desired_maximum_frame_latency, Some(1));
}

#[tokio::test]
async fn test_surface_config_builder_chaining() {
    let config = SurfaceConfigBuilder::new()
        .with_size(1920, 1080)
        .with_present_mode(PresentMode::Mailbox)
        .with_alpha_mode(CompositeAlphaMode::Opaque)
        .with_frame_latency(1)
        .with_view_formats(vec![TextureFormat::Bgra8Unorm]);

    assert_eq!(config.width, 1920);
    assert_eq!(config.height, 1080);
    assert_eq!(config.present_mode, Some(PresentMode::Mailbox));
    assert_eq!(config.alpha_mode, Some(CompositeAlphaMode::Opaque));
    assert_eq!(config.desired_maximum_frame_latency, Some(1));
    assert_eq!(config.view_formats, vec![TextureFormat::Bgra8Unorm]);
}

#[tokio::test]
async fn test_platform_surface_capabilities_conversion() {
    let caps = wgpu::SurfaceCapabilities {
        formats: vec![TextureFormat::Bgra8UnormSrgb, TextureFormat::Rgba8UnormSrgb],
        present_modes: vec![PresentMode::Fifo, PresentMode::Immediate],
        alpha_modes: vec![
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PreMultiplied,
        ],
        usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
    };

    let platform_caps = PlatformSurfaceCapabilities::from(&caps);

    assert_eq!(platform_caps.formats.len(), 2);
    assert_eq!(platform_caps.present_modes.len(), 2);
    assert_eq!(platform_caps.alpha_modes.len(), 2);
    assert_eq!(platform_caps.usages, wgpu::TextureUsages::RENDER_ATTACHMENT);
}

// Note: Native integration tests are disabled because winit's EventLoop
// requires initialization on the main thread, which conflicts with tokio::test.
// These tests can be run manually in examples or with a custom test harness.
#[cfg(any())] // Tests disabled: winit's EventLoop requires main thread, conflicts with tokio::test
mod native_tests {
    use super::*;
    use gup::context::PhysicalSize;
    use std::sync::Arc;
    use winit::event_loop::EventLoop;
    use winit::window::WindowAttributes;

    // Helper to get a mutable context for testing
    async fn get_test_context() -> GupContext {
        let context_arc = GupContext::headless().await.unwrap();
        Arc::try_unwrap(context_arc).expect("Failed to unwrap Arc - context is still shared")
    }

    #[tokio::test]
    async fn test_add_surface_with_default_config() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        let config = SurfaceConfigBuilder::new().with_size(1024, 768);

        let id = SurfaceId::new();
        let result = context.add_surface_with_config(id, window.clone(), config);

        assert!(result.is_ok());

        // Verify surface was added
        let resize_result = context.resize_surface(id, PhysicalSize::new(800, 600));
        assert!(resize_result.is_ok());
    }

    #[tokio::test]
    async fn test_add_surface_with_low_latency_config() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        // Configure for low latency (interactive applications)
        let config = SurfaceConfigBuilder::new()
            .with_size(1920, 1080)
            .with_frame_latency(1);

        let id = SurfaceId::new();
        let result = context.add_surface_with_config(id, window.clone(), config);

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_query_surface_capabilities() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let context = get_test_context().await;
        let caps_result = context.query_surface_capabilities(window);

        assert!(caps_result.is_ok());

        let caps = caps_result.unwrap();
        assert!(!caps.formats.is_empty(), "Should have available formats");
        assert!(
            !caps.present_modes.is_empty(),
            "Should have available present modes"
        );
        assert!(
            !caps.alpha_modes.is_empty(),
            "Should have available alpha modes"
        );
    }

    #[tokio::test]
    async fn test_add_surface_with_invalid_present_mode() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        // Query capabilities first
        let caps = context.query_surface_capabilities(window.clone()).unwrap();

        // Try to use an unsupported mode (if any)
        // Note: PresentMode::Fifo is always supported, so we can't test with that
        // This test documents the validation behavior
        let config = SurfaceConfigBuilder::new().with_present_mode(PresentMode::Fifo);

        let id = SurfaceId::new();
        let result = context.add_surface_with_config(id, window.clone(), config);

        // Fifo is always supported, so this should succeed
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_surface_with_view_formats() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        // Query capabilities to get valid formats
        let caps = context.query_surface_capabilities(window.clone()).unwrap();

        // Use a format from the capabilities as a view format
        let view_format = caps.formats.first().copied().unwrap();

        let config = SurfaceConfigBuilder::new()
            .with_size(800, 600)
            .with_view_formats(vec![view_format]);

        let id = SurfaceId::new();
        let result = context.add_surface_with_config(id, window.clone(), config);

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_surface_with_multiple_view_formats() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        // Query capabilities
        let caps = context.query_surface_capabilities(window.clone()).unwrap();

        // Try to use multiple view formats (if available)
        let view_formats: Vec<_> = caps.formats.iter().take(2).copied().collect();

        if view_formats.len() >= 2 {
            let config = SurfaceConfigBuilder::new()
                .with_size(800, 600)
                .with_view_formats(view_formats);

            let id = SurfaceId::new();
            let result = context.add_surface_with_config(id, window.clone(), config);

            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_duplicate_surface_id_with_config() {
        let event_loop = EventLoop::new().unwrap();
        let window1 = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );
        let window2 = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        let config = SurfaceConfigBuilder::new();
        let id = SurfaceId::new();

        // Add first surface
        let result1 = context.add_surface_with_config(id, window1, config.clone());
        assert!(result1.is_ok());

        // Try to add second surface with same ID - should fail
        let result2 = context.add_surface_with_config(id, window2, config);
        assert!(result2.is_err());
        assert!(matches!(
            result2.unwrap_err(),
            GupError::ResourceError { message: _ }
        ));
    }

    #[tokio::test]
    async fn test_surface_config_comprehensive() {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .unwrap(),
        );

        let mut context = get_test_context().await;

        // Query capabilities to ensure we use supported values
        let caps = context.query_surface_capabilities(window.clone()).unwrap();

        let format = caps.formats.first().copied().unwrap();
        let present_mode = caps.present_modes.first().copied().unwrap();
        let alpha_mode = caps.alpha_modes.first().copied().unwrap();

        let config = SurfaceConfigBuilder::new()
            .with_size(1280, 720)
            .with_format(format)
            .with_present_mode(present_mode)
            .with_alpha_mode(alpha_mode)
            .with_frame_latency(2)
            .with_view_formats(vec![format]);

        let id = SurfaceId::new();
        let result = context.add_surface_with_config(id, window.clone(), config);

        assert!(result.is_ok());

        // Verify surface can be used
        let resize_result = context.resize_surface(id, PhysicalSize::new(1024, 768));
        assert!(resize_result.is_ok());
    }
}
