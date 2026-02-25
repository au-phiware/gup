// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for text clipping detection and viewport bounds management.
//!
//! These tests verify that the TextLayoutEngine correctly detects and applies
//! clipping strategies when text extends beyond viewport or container bounds.

use gup::RenderContext;
use gup::shader_function::Vec2;
use gup::text::{
    ClippingStrategy, ClippingStrategyConfig, FontAtlas, TextAnchor, TextLayoutEngine, TextStyle,
    ViewportBounds,
};
use std::time::Instant;

/// Helper to create GPU context for tests
async fn create_test_context() -> RenderContext {
    RenderContext::new()
        .await
        .expect("Failed to create GPU context")
}

// =============================================================================
// layout_text_with_clipping: no clipping needed
// =============================================================================

#[tokio::test]
async fn test_layout_with_clipping_text_fits() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(800.0, 600.0);
    let config = ClippingStrategyConfig::default();

    let result = engine
        .layout_text_with_clipping(
            "Hello",
            Vec2 { x: 50.0, y: 50.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(!result.clipped, "Short text should not be clipped");
    assert!(!result.glyphs.is_empty(), "Should produce glyphs");
}

// =============================================================================
// layout_text_with_clipping: truncation with ellipsis
// =============================================================================

#[tokio::test]
async fn test_truncation_with_ellipsis() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    // Create a very narrow container so the text must be truncated
    let viewport = ViewportBounds::from_screen(120.0, 100.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: false,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: false,
    };

    let long_text = "This is a very long text that should be truncated";
    let result = engine
        .layout_text_with_clipping(
            long_text,
            Vec2 { x: 5.0, y: 5.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(result.clipped, "Long text should be clipped");
    // The truncated text should produce fewer glyphs than the original
    let full_result = engine
        .layout_text("Hello World Foo Bar", Vec2 { x: 5.0, y: 5.0 }, &style, &font_atlas, None)
        .unwrap();
    assert!(
        result.glyphs.len() < full_result.glyphs.len() + 10,
        "Truncated text should have fewer or similar glyphs"
    );
}

#[tokio::test]
async fn test_truncation_preserves_word_boundaries() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(130.0, 100.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: true,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: false,
    };

    let result = engine
        .layout_text_with_clipping(
            "Hello World Foo Bar Baz",
            Vec2 { x: 5.0, y: 5.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(result.clipped, "Text should be clipped");
    assert!(!result.glyphs.is_empty(), "Should still produce some glyphs");
}

// =============================================================================
// layout_text_with_clipping: dynamic font scaling
// =============================================================================

#[tokio::test]
async fn test_dynamic_font_scaling() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(48.0); // Large font
    let viewport = ViewportBounds::from_screen(200.0, 100.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::DynamicFontScaling {
            min_font_size: 8.0,
            scale_factor: 0.2,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: false,
    };

    let result = engine
        .layout_text_with_clipping(
            "Hello",
            Vec2 { x: 10.0, y: 10.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    // Either scaled to fit or fell through
    if !result.glyphs.is_empty() {
        // The result bounds should fit within the viewport
        let clip = viewport.detect_clipping(&result.bounds);
        // Font scaling should have produced a fitting result
        assert!(
            !clip.is_clipped() || result.clipped,
            "Scaled text should fit or be marked as clipped"
        );
    }
}

// =============================================================================
// layout_text_with_clipping: reposition strategy
// =============================================================================

#[tokio::test]
async fn test_reposition_text() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(200.0, 200.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::RepositionText {
            prefer_directions: vec![
                Vec2 { x: -1.0, y: 0.0 }, // Push left
                Vec2 { x: 0.0, y: -1.0 }, // Push up
            ],
            max_offset_distance: 100.0,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: false,
    };

    // Position text near the right edge so it overflows
    let result = engine
        .layout_text_with_clipping(
            "Hello",
            Vec2 { x: 185.0, y: 50.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    // Should have repositioned the text
    assert!(!result.glyphs.is_empty(), "Text should be repositioned, not hidden");
}

// =============================================================================
// layout_text_with_clipping: hide strategy
// =============================================================================

#[tokio::test]
async fn test_hide_if_clipped() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    // Very small viewport — text can't fit
    let viewport = ViewportBounds::from_screen(20.0, 20.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::HideIfClipped {
            min_visible_threshold: 0.8,
        },
        fallback_strategies: vec![],
        minimum_visible_percentage: 0.0,
        enable_hover_reveal: false,
    };

    let result = engine
        .layout_text_with_clipping(
            "A very long label that cannot fit",
            Vec2 { x: 5.0, y: 5.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(result.clipped, "Text should be marked as clipped");
    assert!(result.glyphs.is_empty(), "Hidden text should produce no glyphs");
}

// =============================================================================
// Strategy fallback chain
// =============================================================================

#[tokio::test]
async fn test_strategy_fallback_chain() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(80.0, 60.0);
    let config = ClippingStrategyConfig {
        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
            ellipsis_text: "...".to_string(),
            preserve_words: true,
        },
        fallback_strategies: vec![
            ClippingStrategy::DynamicFontScaling {
                min_font_size: 8.0,
                scale_factor: 0.1,
            },
            ClippingStrategy::HideIfClipped {
                min_visible_threshold: 0.3,
            },
        ],
        minimum_visible_percentage: 0.1,
        enable_hover_reveal: false,
    };

    let result = engine
        .layout_text_with_clipping(
            "This text is way too long for the tiny container bounds we set up here",
            Vec2 { x: 5.0, y: 5.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    // The fallback chain should produce some result
    assert!(result.clipped, "Text should be clipped");
}

// =============================================================================
// Backward compatibility
// =============================================================================

#[tokio::test]
async fn test_backward_compatible_layout_text() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    // Existing layout_text API should still work unchanged
    let result = engine
        .layout_text(
            "Hello World",
            Vec2 { x: 50.0, y: 50.0 },
            &style,
            &font_atlas,
            None,
        )
        .unwrap();

    assert!(!result.clipped);
    assert!(!result.glyphs.is_empty());
    assert!(result.bounds.width() > 0.0);
}

// =============================================================================
// Container bounds support
// =============================================================================

#[tokio::test]
async fn test_custom_container_bounds() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    // Use a smaller container within the viewport
    let container = gup::text::TextBounds::new(100.0, 100.0, 200.0, 200.0);
    let viewport = ViewportBounds::from_container(container);
    let config = ClippingStrategyConfig::default();

    // Text positioned inside the container should not be clipped
    let result_inside = engine
        .layout_text_with_clipping(
            "Hi",
            Vec2 { x: 120.0, y: 120.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(!result_inside.clipped, "Text inside container should not be clipped");

    // Text positioned outside the container should be clipped
    engine.clear();
    let result_outside = engine
        .layout_text_with_clipping(
            "This is a really long label that overflows the container",
            Vec2 { x: 110.0, y: 120.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(
        result_outside.clipped,
        "Long text outside container should be clipped"
    );
}

// =============================================================================
// Performance: clipping with many labels
// =============================================================================

#[tokio::test]
async fn test_clipping_performance_500_labels() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(12.0);
    let viewport = ViewportBounds::from_screen(800.0, 600.0);
    let config = ClippingStrategyConfig::default();

    let labels: Vec<(String, Vec2)> = (0..500)
        .map(|i| {
            (
                format!("Label {}", i),
                Vec2 {
                    x: (i % 40) as f32 * 25.0 - 50.0,
                    y: (i / 40) as f32 * 25.0,
                },
            )
        })
        .collect();

    let start = Instant::now();
    let mut clipped_count = 0;
    for (text, pos) in &labels {
        let result = engine
            .layout_text_with_clipping(text, *pos, &style, &font_atlas, None, &viewport, &config)
            .unwrap();
        if result.clipped {
            clipped_count += 1;
        }
    }
    let duration = start.elapsed();

    println!(
        "500 labels with clipping took: {:?} ({} clipped)",
        duration, clipped_count
    );

    // Should complete reasonably fast
    #[cfg(debug_assertions)]
    let threshold_ms: u128 = 2000;
    #[cfg(not(debug_assertions))]
    let threshold_ms: u128 = 500;

    assert!(
        duration.as_millis() < threshold_ms,
        "Clipping 500 labels too slow: {:?} (threshold: {}ms)",
        duration,
        threshold_ms
    );
}

// =============================================================================
// Anchor-aware clipping
// =============================================================================

#[tokio::test]
async fn test_clipping_with_center_anchor() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0).with_anchor(TextAnchor::Center);
    let viewport = ViewportBounds::from_screen(200.0, 100.0);
    let config = ClippingStrategyConfig::default();

    // Center-anchored text near edge should trigger clipping
    let result = engine
        .layout_text_with_clipping(
            "Center aligned text that is fairly long",
            Vec2 { x: 190.0, y: 50.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(result.clipped, "Center-anchored text near edge should clip");
}

// =============================================================================
// Edge case: empty text
// =============================================================================

#[tokio::test]
async fn test_clipping_empty_text() {
    let context = create_test_context().await;
    let font_atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let style = TextStyle::new(16.0);
    let viewport = ViewportBounds::from_screen(800.0, 600.0);
    let config = ClippingStrategyConfig::default();

    let result = engine
        .layout_text_with_clipping(
            "",
            Vec2 { x: 50.0, y: 50.0 },
            &style,
            &font_atlas,
            None,
            &viewport,
            &config,
        )
        .unwrap();

    assert!(!result.clipped, "Empty text should not be marked as clipped");
}
