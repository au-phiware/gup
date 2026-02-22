// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual validation tests for blend mode rendering
//!
//! These tests ensure that blend modes produce the expected visual results
//! by rendering to offscreen textures and comparing pixel data.

use gup::{BlendMode, visual_test_utils::VisualTestUtils};

const TEST_WIDTH: u32 = 64;
const TEST_HEIGHT: u32 = 64;
const TOLERANCE: u8 = 2; // Allow small precision differences

#[tokio::test]
async fn test_blend_mode_none() {
    let utils = VisualTestUtils::new().await.unwrap();

    // Red background, blue foreground with no blending
    // Expected: foreground completely replaces background in overlap area
    let pixels = utils
        .render_blend_test(
            [1.0, 0.0, 0.0, 1.0], // Red background
            [0.0, 0.0, 1.0, 1.0], // Blue foreground
            BlendMode::None,
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .await
        .unwrap();

    // Sample a pixel in the foreground region (right side)
    // Should be pure blue
    let pixel_idx = (TEST_HEIGHT / 2 * TEST_WIDTH + TEST_WIDTH * 3 / 4) as usize * 4;
    let r = pixels[pixel_idx];
    let g = pixels[pixel_idx + 1];
    let b = pixels[pixel_idx + 2];
    let a = pixels[pixel_idx + 3];

    assert!(r < 10, "Red channel should be near 0, got {}", r);
    assert!(g < 10, "Green channel should be near 0, got {}", g);
    assert!(b > 245, "Blue channel should be near 255, got {}", b);
    assert!(a > 245, "Alpha channel should be near 255, got {}", a);
}

#[tokio::test]
async fn test_blend_mode_alpha_blending() {
    let utils = VisualTestUtils::new().await.unwrap();

    // Red background (50% alpha), blue foreground (50% alpha)
    // Expected: colors blend in overlap area
    let pixels = utils
        .render_blend_test(
            [1.0, 0.0, 0.0, 0.5], // Red, 50% alpha
            [0.0, 0.0, 1.0, 0.5], // Blue, 50% alpha
            BlendMode::AlphaBlending,
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .await
        .unwrap();

    // Sample a pixel in the overlap region (center)
    // Should be a blend of red and blue
    let pixel_idx = (TEST_HEIGHT / 2 * TEST_WIDTH + TEST_WIDTH / 2) as usize * 4;
    let r = pixels[pixel_idx];
    let g = pixels[pixel_idx + 1];
    let b = pixels[pixel_idx + 2];

    // With alpha blending, we expect some of both red and blue
    assert!(
        r > 30,
        "Red channel should have some contribution, got {}",
        r
    );
    assert!(
        b > 30,
        "Blue channel should have some contribution, got {}",
        b
    );
    assert!(g < 30, "Green channel should be minimal, got {}", g);
}

#[tokio::test]
async fn test_blend_mode_additive() {
    let utils = VisualTestUtils::new().await.unwrap();

    // Medium red background, medium blue foreground
    // Expected: colors add up (brighter) in overlap area
    let pixels = utils
        .render_blend_test(
            [0.5, 0.0, 0.0, 1.0], // Medium red
            [0.0, 0.0, 0.5, 1.0], // Medium blue
            BlendMode::Additive,
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .await
        .unwrap();

    // Sample a pixel in the overlap region (center)
    // Should have both red and blue contributing additively
    let pixel_idx = (TEST_HEIGHT / 2 * TEST_WIDTH + TEST_WIDTH / 2) as usize * 4;
    let r = pixels[pixel_idx];
    let g = pixels[pixel_idx + 1];
    let b = pixels[pixel_idx + 2];

    // With additive blending, colors should add
    assert!(r > 100, "Red channel should be present, got {}", r);
    assert!(b > 100, "Blue channel should be present, got {}", b);
    assert!(g < 30, "Green channel should be minimal, got {}", g);
}

#[tokio::test]
#[ignore] // TODO: Fix multiply blend mode - currently causes GPU timeout
async fn test_blend_mode_multiply() {
    let utils = VisualTestUtils::new().await.unwrap();

    // Light red background, light blue foreground
    // Expected: colors multiply (darker) in overlap area
    let pixels = utils
        .render_blend_test(
            [1.0, 0.5, 0.5, 1.0], // Light red (red + some white)
            [0.5, 0.5, 1.0, 1.0], // Light blue (blue + some white)
            BlendMode::Multiply,
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .await
        .unwrap();

    // Sample a pixel in the overlap region (center)
    // Multiply should darken the colors
    let pixel_idx = (TEST_HEIGHT / 2 * TEST_WIDTH + TEST_WIDTH / 2) as usize * 4;
    let r = pixels[pixel_idx];
    let g = pixels[pixel_idx + 1];
    let b = pixels[pixel_idx + 2];

    // With multiply blending, result should be darker than either source
    // Multiply of (1.0, 0.5, 0.5) and (0.5, 0.5, 1.0) should give (0.5, 0.25, 0.5)
    assert!(r > 100, "Red component should be present, got {}", r);
    assert!(
        g < 100,
        "Green should be darkest (lowest multiply), got {}",
        g
    );
    assert!(b > 100, "Blue component should be present, got {}", b);
}

#[tokio::test]
async fn test_reference_image_comparison() {
    let mut utils = VisualTestUtils::new().await.unwrap();

    // Generate reference images
    utils
        .generate_reference_images(TEST_WIDTH, TEST_HEIGHT)
        .await
        .unwrap();

    // Re-render the same scenes and compare
    let test_cases = vec![
        (
            "none_red_blue",
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            BlendMode::None,
        ),
        (
            "alpha_red_blue",
            [1.0, 0.0, 0.0, 0.5],
            [0.0, 0.0, 1.0, 0.5],
            BlendMode::AlphaBlending,
        ),
        (
            "additive_red_blue",
            [0.5, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.5, 1.0],
            BlendMode::Additive,
        ),
        (
            "multiply_red_blue",
            [1.0, 0.5, 0.5, 1.0],
            [0.5, 0.5, 1.0, 1.0],
            BlendMode::Multiply,
        ),
    ];

    for (name, bg_color, fg_color, blend_mode) in test_cases {
        let pixels = utils
            .render_blend_test(bg_color, fg_color, blend_mode, TEST_WIDTH, TEST_HEIGHT)
            .await
            .unwrap();

        assert!(
            utils.compare_with_reference(&pixels, name, TOLERANCE),
            "Reference comparison failed for blend mode: {}",
            name
        );
    }
}

#[tokio::test]
async fn test_cross_platform_precision() {
    let utils = VisualTestUtils::new().await.unwrap();

    // Render the same scene twice and ensure consistency
    let bg_color = [0.7, 0.3, 0.2, 1.0];
    let fg_color = [0.2, 0.5, 0.8, 0.8];

    let pixels1 = utils
        .render_blend_test(
            bg_color,
            fg_color,
            BlendMode::AlphaBlending,
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .await
        .unwrap();

    let pixels2 = utils
        .render_blend_test(
            bg_color,
            fg_color,
            BlendMode::AlphaBlending,
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .await
        .unwrap();

    // Pixels should be identical (or within tolerance for floating point)
    for (i, (p1, p2)) in pixels1.iter().zip(pixels2.iter()).enumerate() {
        let diff = (*p1 as i32 - *p2 as i32).unsigned_abs() as u8;
        assert!(
            diff <= TOLERANCE,
            "Pixel {} mismatch: {} vs {} (diff {})",
            i,
            p1,
            p2,
            diff
        );
    }
}

#[tokio::test]
async fn test_all_blend_modes_render() {
    let utils = VisualTestUtils::new().await.unwrap();

    // Test that all blend modes can be rendered without errors
    let blend_modes = vec![
        BlendMode::None,
        BlendMode::AlphaBlending,
        BlendMode::Additive,
        BlendMode::Multiply,
    ];

    for blend_mode in blend_modes {
        let result = utils
            .render_blend_test(
                [0.8, 0.2, 0.3, 0.9],
                [0.2, 0.7, 0.9, 0.8],
                blend_mode,
                TEST_WIDTH,
                TEST_HEIGHT,
            )
            .await;

        assert!(
            result.is_ok(),
            "Blend mode {:?} failed to render: {:?}",
            blend_mode,
            result.err()
        );
    }
}

#[tokio::test]
#[ignore] // TODO: Fix GPU resource management for multiple renderings
async fn test_different_resolutions() {
    let utils = VisualTestUtils::new().await.unwrap();

    let resolutions = vec![(32, 32), (64, 64), (128, 128), (256, 256)];

    for (width, height) in resolutions {
        let result = utils
            .render_blend_test(
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                BlendMode::AlphaBlending,
                width,
                height,
            )
            .await;

        assert!(
            result.is_ok(),
            "Failed to render at {}x{}: {:?}",
            width,
            height,
            result.err()
        );

        let pixels = result.unwrap();
        assert_eq!(
            pixels.len(),
            (width * height * 4) as usize,
            "Incorrect pixel buffer size for {}x{}",
            width,
            height
        );
    }
}
