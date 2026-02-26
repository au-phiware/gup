// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the interactive clipping hover reveal system.
//!
//! These tests verify that the hover reveal system correctly integrates
//! with the text layout pipeline: clipped text is registered, hover
//! detection works, and tooltip layout is computed correctly.

use gup::RenderContext;
use gup::shader_function::Vec2;
use gup::text::hover_reveal::{
    ClippedTextRegistry, HoverRevealState, TooltipConfig, compute_tooltip_layout,
};
use gup::text::{
    ClippingStrategy, ClippingStrategyConfig, FontAtlas, TextLayoutEngine, TextStyle,
    ViewportBounds,
};

/// Helper to create GPU context for tests.
async fn create_test_context() -> RenderContext {
    RenderContext::new()
        .await
        .expect("Failed to create GPU context")
}

// =============================================================================
// LayoutResult.original_text with enable_hover_reveal
// =============================================================================

#[tokio::test]
async fn test_hover_reveal_stores_original_text_on_truncation() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    // Very narrow viewport forces truncation
    let viewport = ViewportBounds::from_screen(60.0, 600.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: false,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: true,
    };

    let long_text = "This is a very long label that should be truncated";
    let result = engine
        .layout_text_with_clipping(
            long_text,
            Vec2 { x: 5.0, y: 10.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(
        result.clipped,
        "Long text should be clipped in narrow viewport"
    );
    assert_eq!(
        result.original_text.as_deref(),
        Some(long_text),
        "original_text should contain the full un-truncated text"
    );
}

#[tokio::test]
async fn test_hover_reveal_disabled_no_original_text() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(60.0, 600.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: false,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: false, // Disabled
    };

    let result = engine
        .layout_text_with_clipping(
            "This is a very long label that should be truncated",
            Vec2 { x: 5.0, y: 10.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(result.clipped);
    assert!(
        result.original_text.is_none(),
        "original_text should be None when hover reveal is disabled"
    );
}

#[tokio::test]
async fn test_no_clipping_no_original_text() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    // Generous viewport — text should fit without clipping
    let viewport = ViewportBounds::from_screen(800.0, 600.0);
    let config = ClippingStrategyConfig {
        enable_hover_reveal: true,
        ..ClippingStrategyConfig::default()
    };

    let result = engine
        .layout_text_with_clipping(
            "Hi",
            Vec2 { x: 50.0, y: 50.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(!result.clipped);
    assert!(
        result.original_text.is_none(),
        "Non-clipped text should not store original_text"
    );
}

// =============================================================================
// Registry + HoverRevealState integration
// =============================================================================

#[tokio::test]
async fn test_registry_with_layout_result() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(60.0, 600.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: false,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: true,
    };

    let long_text = "Revenue per Quarter (2024)";
    let result = engine
        .layout_text_with_clipping(
            long_text,
            Vec2 { x: 5.0, y: 10.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    // Register with the registry
    let mut registry = ClippedTextRegistry::new();
    if let Some(original) = &result.original_text {
        registry.register(result.bounds, original);
    }

    assert_eq!(registry.len(), 1);

    // Hover over the truncated text
    let center = result.bounds.center();
    let hit = registry.hit_test(center.x, center.y);
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().original_text, long_text);
}

#[tokio::test]
async fn test_hover_state_with_registry() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(60.0, 600.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: false,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: true,
    };

    let long_text = "Total Sales Figure for Department";
    let result = engine
        .layout_text_with_clipping(
            long_text,
            Vec2 { x: 5.0, y: 10.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    // Register
    let mut registry = ClippedTextRegistry::new();
    if let Some(original) = &result.original_text {
        registry.register(result.bounds, original);
    }

    // Create hover state with no delay (instant)
    let tooltip_config = TooltipConfig {
        show_delay: 0.0,
        fade_in_duration: 0.0,
        fade_out_duration: 0.0,
        ..Default::default()
    };
    let mut hover_state = HoverRevealState::new(tooltip_config);

    // Hover over the text
    let center = result.bounds.center();
    hover_state.update(&registry, center.x, center.y, 0.016);

    let tooltip = hover_state.active_tooltip();
    assert!(
        tooltip.is_some(),
        "Should show tooltip when hovering clipped text"
    );
    assert_eq!(tooltip.unwrap().text, long_text);
}

// =============================================================================
// Tooltip layout with real text measurements
// =============================================================================

#[tokio::test]
async fn test_tooltip_layout_with_real_measurements() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 14.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(14.0);
    // Measure the tooltip text
    let tooltip_text = "Revenue per Quarter (2024)";
    let text_bounds = engine
        .layout_text(
            tooltip_text,
            Vec2 { x: 0.0, y: 0.0 },
            &style,
            &font_atlas,
            None,
        )
        .unwrap()
        .bounds;

    let config = TooltipConfig::default();
    let active = gup::text::hover_reveal::ActiveTooltip {
        text: tooltip_text.to_string(),
        position: Vec2 { x: 200.0, y: 80.0 },
        opacity: 1.0,
        source_bounds: gup::text::TextBounds::new(150.0, 60.0, 250.0, 76.0),
    };

    let layout = compute_tooltip_layout(
        &active,
        &config,
        text_bounds.width(),
        text_bounds.height(),
        800.0,
        600.0,
    );

    // Background should be larger than text (includes padding)
    assert!(layout.background_bounds.width() > text_bounds.width());
    assert!(layout.background_bounds.height() > text_bounds.height());

    // Text position inside background
    assert!(layout.text_position.x >= layout.background_bounds.left);
    assert!(layout.text_position.y >= layout.background_bounds.top);
    assert!(layout.text_position.x < layout.background_bounds.right);
    assert!(layout.text_position.y < layout.background_bounds.bottom);
}

// =============================================================================
// Performance: hover detection overhead
// =============================================================================

#[test]
fn test_hover_detection_performance() {
    use std::time::Instant;

    let mut registry = ClippedTextRegistry::new();

    // Register 100 clipped text entries
    for i in 0..100 {
        let left = (i % 10) as f32 * 80.0;
        let top = (i / 10) as f32 * 30.0;
        registry.register(
            gup::text::TextBounds::new(left, top, left + 70.0, top + 20.0),
            &format!("This is the original long label text number {}", i),
        );
    }

    let mut hover_state = HoverRevealState::new(TooltipConfig {
        show_delay: 0.0,
        fade_in_duration: 0.0,
        ..Default::default()
    });

    // Time 10000 hover update cycles
    let start = Instant::now();
    for i in 0..10_000 {
        let x = (i % 800) as f32;
        let y = (i % 300) as f32;
        hover_state.update(&registry, x, y, 0.016);
    }
    let elapsed = start.elapsed();

    // Should complete well under 100ms for 10K updates with 100 entries
    assert!(
        elapsed.as_millis() < 100,
        "Hover detection took {}ms for 10K updates (should be <100ms)",
        elapsed.as_millis()
    );
}
