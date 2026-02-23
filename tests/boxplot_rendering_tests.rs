// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box Plot Rendering Tests
//!
//! Tests for box plot statistical computation and component generation.
//! These tests validate that box plots correctly compute quartiles, identify outliers,
//! and generate renderable components.

use gup::mark::BoxPlotAttributes;
use gup::mark::BoxPlotOrientation;
use gup::shader_function::Vec2;

#[test]
fn test_boxplot_normal_distribution() {
    // Normal distribution: should have no or few outliers
    let data = vec![
        42.0, 45.0, 48.0, 50.0, 52.0, 54.0, 56.0, 58.0, 60.0, 62.0, 44.0, 46.0, 48.0, 52.0, 54.0,
        56.0, 58.0, 60.0, 50.0, 52.0,
    ];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // Verify statistical values are reasonable
    assert!(attrs.min < attrs.q1);
    assert!(attrs.q1 < attrs.median);
    assert!(attrs.median < attrs.q3);
    assert!(attrs.q3 < attrs.max);

    // Normal distribution should have few or no outliers
    assert!(
        attrs.outliers.len() < 3,
        "Normal distribution should have few outliers, got {}",
        attrs.outliers.len()
    );

    // Check IQR is reasonable
    let iqr = attrs.iqr();
    assert!(iqr > 0.0, "IQR should be positive");

    println!("Normal distribution:");
    println!(
        "  min={}, Q1={}, median={}, Q3={}, max={}",
        attrs.min, attrs.q1, attrs.median, attrs.q3, attrs.max
    );
    println!("  IQR={}, outliers={}", iqr, attrs.outliers.len());
}

#[test]
fn test_boxplot_with_outliers() {
    // Dataset deliberately designed with outliers
    let data = vec![
        42.0, 44.0, 45.0, 46.0, 47.0, 48.0, 49.0, 50.0, 51.0, 52.0, 43.0, 44.0, 45.0, 46.0, 47.0,
        48.0, 49.0, 50.0, 51.0, 52.0, // Main data cluster
        15.0, 20.0, 75.0, 80.0, 85.0, // Obvious outliers
    ];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // Should have detected the outliers
    assert!(
        attrs.outliers.len() >= 5,
        "Should detect at least 5 outliers, got {}",
        attrs.outliers.len()
    );

    // Verify the outliers are actually outside the whiskers
    for &outlier in &attrs.outliers {
        let is_outlier = attrs.is_outlier(outlier);
        assert!(
            is_outlier,
            "Value {} should be classified as outlier",
            outlier
        );

        // Outliers should be outside the whisker range
        assert!(
            outlier < attrs.min || outlier > attrs.max,
            "Outlier {} should be outside whisker range [{}, {}]",
            outlier,
            attrs.min,
            attrs.max
        );
    }

    println!("With outliers:");
    println!(
        "  min={}, Q1={}, median={}, Q3={}, max={}",
        attrs.min, attrs.q1, attrs.median, attrs.q3, attrs.max
    );
    println!("  outliers={:?}", attrs.outliers);
}

#[test]
fn test_boxplot_skewed_distribution() {
    // Right-skewed distribution
    let data = vec![
        60.0, 62.0, 64.0, 66.0, 68.0, 70.0, 72.0, 75.0, 80.0, 85.0, 61.0, 63.0, 65.0, 67.0, 69.0,
        71.0, 76.0, 82.0, 88.0, 95.0,
    ];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // For skewed distribution, median should be closer to Q1
    let q1_to_median = attrs.median - attrs.q1;
    let median_to_q3 = attrs.q3 - attrs.median;

    // Right-skewed means median closer to Q1
    // (though this might not always hold depending on the data)
    println!("Skewed distribution:");
    println!("  Q1 to median: {}", q1_to_median);
    println!("  Median to Q3: {}", median_to_q3);
    println!(
        "  min={}, Q1={}, median={}, Q3={}, max={}",
        attrs.min, attrs.q1, attrs.median, attrs.q3, attrs.max
    );

    // Basic sanity checks
    assert!(attrs.q1 < attrs.median);
    assert!(attrs.median < attrs.q3);
}

#[test]
fn test_boxplot_uniform_distribution() {
    // Uniform distribution: evenly spaced values
    let data = vec![
        30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0, 32.0, 33.0, 37.0, 42.0, 47.0, 52.0,
        57.0, 62.0, 67.0,
    ];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // Uniform distribution should have median roughly in the middle
    let range = attrs.max - attrs.min;
    let median_position = (attrs.median - attrs.min) / range;

    println!("Uniform distribution:");
    println!("  Median position in range: {:.2}", median_position);
    println!(
        "  min={}, Q1={}, median={}, Q3={}, max={}",
        attrs.min, attrs.q1, attrs.median, attrs.q3, attrs.max
    );

    // Median should be roughly centered (allow some variance)
    assert!(
        median_position > 0.3 && median_position < 0.7,
        "Median should be roughly centered, got {:.2}",
        median_position
    );
}

#[test]
fn test_boxplot_orientation() {
    let data = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0];

    // Vertical orientation
    let vertical = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // Horizontal orientation
    let horizontal = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Horizontal,
    );

    // Statistics should be the same regardless of orientation
    assert_eq!(vertical.min, horizontal.min);
    assert_eq!(vertical.q1, horizontal.q1);
    assert_eq!(vertical.median, horizontal.median);
    assert_eq!(vertical.q3, horizontal.q3);
    assert_eq!(vertical.max, horizontal.max);

    // Orientation should be stored correctly
    assert_eq!(vertical.orientation, BoxPlotOrientation::Vertical);
    assert_eq!(horizontal.orientation, BoxPlotOrientation::Horizontal);
}

#[test]
fn test_boxplot_single_value() {
    // Edge case: all same value
    let data = vec![50.0, 50.0, 50.0, 50.0, 50.0];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // All quartiles should be the same
    assert_eq!(attrs.min, 50.0);
    assert_eq!(attrs.q1, 50.0);
    assert_eq!(attrs.median, 50.0);
    assert_eq!(attrs.q3, 50.0);
    assert_eq!(attrs.max, 50.0);

    // IQR should be zero
    assert_eq!(attrs.iqr(), 0.0);

    // No outliers
    assert_eq!(attrs.outliers.len(), 0);

    println!("Single value:");
    println!("  All values: {}", attrs.median);
}

#[test]
fn test_boxplot_iqr_calculation() {
    let data = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    let iqr = attrs.iqr();

    // IQR should equal Q3 - Q1
    assert_eq!(iqr, attrs.q3 - attrs.q1);

    // IQR should be positive for non-uniform data
    assert!(iqr > 0.0);

    // Whiskers should extend to min/max or 1.5*IQR, whichever is closer
    let lower_fence = attrs.q1 - 1.5 * iqr;
    let upper_fence = attrs.q3 + 1.5 * iqr;

    // Whisker min/max should be within fences
    assert!(
        attrs.min >= lower_fence,
        "Min whisker {} should be >= lower fence {}",
        attrs.min,
        lower_fence
    );
    assert!(
        attrs.max <= upper_fence,
        "Max whisker {} should be <= upper fence {}",
        attrs.max,
        upper_fence
    );

    println!("IQR calculation:");
    println!("  IQR: {}", iqr);
    println!(
        "  Lower fence: {}, Upper fence: {}",
        lower_fence, upper_fence
    );
    println!("  Whisker min: {}, Whisker max: {}", attrs.min, attrs.max);
}

#[test]
fn test_boxplot_position_and_width() {
    let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];

    let position = Vec2 { x: 100.0, y: 200.0 };
    let width = 0.25;

    let attrs = BoxPlotAttributes::from_data(&data, position, width, BoxPlotOrientation::Vertical);

    // Position and width should be stored
    assert_eq!(attrs.position.x, 100.0);
    assert_eq!(attrs.position.y, 200.0);
    assert_eq!(attrs.width, 0.25);
}

#[test]
fn test_boxplot_colors() {
    let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];

    let attrs = BoxPlotAttributes::from_data(
        &data,
        Vec2 { x: 0.0, y: 0.0 },
        0.1,
        BoxPlotOrientation::Vertical,
    );

    // Verify default colors are set
    // Box fill should be blue-ish
    assert!(attrs.box_fill_color.z > 0.9, "Box fill should be bluish");

    // Median should be red
    assert!(attrs.median_color.x > 0.9, "Median should be reddish");

    // Outliers should be orange
    assert!(
        attrs.outlier_color.x > 0.9 && attrs.outlier_color.y > 0.4,
        "Outliers should be orange-ish"
    );

    // Stroke colors should be set
    assert_eq!(attrs.box_stroke_color.w, 1.0);
    assert_eq!(attrs.whisker_color.w, 1.0);
}

#[test]
fn test_boxplot_multiple_instances() {
    // Test creating multiple box plots for comparison
    let datasets = [
        vec![10.0, 20.0, 30.0, 40.0, 50.0],
        vec![60.0, 70.0, 80.0, 90.0, 100.0],
        vec![5.0, 15.0, 25.0, 35.0, 45.0],
    ];

    let mut all_attrs = Vec::new();

    for (i, data) in datasets.iter().enumerate() {
        let position = Vec2 {
            x: (i as f32) * 0.3 - 0.3,
            y: 0.0,
        };
        let attrs = BoxPlotAttributes::from_data(data, position, 0.1, BoxPlotOrientation::Vertical);
        all_attrs.push(attrs);
    }

    assert_eq!(all_attrs.len(), 3);

    // Each should have different medians
    assert!(all_attrs[0].median < all_attrs[1].median);
    assert!(all_attrs[2].median < all_attrs[0].median);

    println!("Multiple instances:");
    for (i, attrs) in all_attrs.iter().enumerate() {
        println!("  Dataset {}: median={}", i, attrs.median);
    }
}
