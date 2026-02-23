// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-Category Box Plot Tests
//!
//! Tests for grouped box plots with category-based data organization.

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, boxplot};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct CategoryData {
    category: String,
    value: f32,
}

#[tokio::test]
async fn test_multi_category_grouping() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        CategoryData {
            category: "A".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 20.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 40.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 50.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 60.0,
        },
    ];

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .build_with_data(data, context)
        .unwrap();

    // Should create 3 box plots (one per category)
    assert_eq!(chart.len(), 3, "Should have 3 box plots for 3 categories");
}

#[tokio::test]
async fn test_category_ordering_alphabetical() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        CategoryData {
            category: "C".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 20.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 40.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 50.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 60.0,
        },
    ];

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .order_alphabetically()
        .build_with_data(data, context)
        .unwrap();

    // Verify positions are ordered (assuming default spacing)
    let positions: Vec<f32> = chart
        .visualization
        .data()
        .iter()
        .map(|m| m.position.x)
        .collect();

    // Should be in ascending order (A, B, C alphabetically)
    assert!(
        positions[0] < positions[1] && positions[1] < positions[2],
        "Positions should be ordered: {:?}",
        positions
    );
}

#[tokio::test]
async fn test_category_ordering_by_median() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    // Category A: median = 35
    // Category B: median = 75
    // Category C: median = 55
    let data = vec![
        CategoryData {
            category: "A".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 35.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 40.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 70.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 75.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 80.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 50.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 55.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 60.0,
        },
    ];

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .order_by_median()
        .build_with_data(data, context)
        .unwrap();

    // Verify medians are in ascending order
    let medians: Vec<f32> = chart
        .visualization
        .data()
        .iter()
        .map(|m| m.median)
        .collect();

    assert!(
        medians[0] < medians[1] && medians[1] < medians[2],
        "Medians should be ordered: {:?}",
        medians
    );

    // First should be A (35), then C (55), then B (75)
    assert!(
        (medians[0] - 35.0).abs() < 0.1,
        "First median should be ~35"
    );
    assert!(
        (medians[1] - 55.0).abs() < 0.1,
        "Second median should be ~55"
    );
    assert!(
        (medians[2] - 75.0).abs() < 0.1,
        "Third median should be ~75"
    );
}

#[tokio::test]
async fn test_category_spacing() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        CategoryData {
            category: "A".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 20.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 40.0,
        },
    ];

    let custom_spacing = 100.0;
    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .category_spacing(custom_spacing)
        .build_with_data(data, context)
        .unwrap();

    let positions: Vec<f32> = chart
        .visualization
        .data()
        .iter()
        .map(|m| m.position.x)
        .collect();

    // Verify spacing between categories
    assert!(
        (positions[1] - positions[0] - custom_spacing).abs() < 0.1,
        "Spacing should be {}, got {}",
        custom_spacing,
        positions[1] - positions[0]
    );
}

#[tokio::test]
async fn test_horizontal_orientation_categories() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        CategoryData {
            category: "A".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 20.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 40.0,
        },
    ];

    let chart = boxplot()
        .x(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .horizontal()
        .build_with_data(data, context)
        .unwrap();

    // For horizontal orientation, spacing should be in Y axis
    let positions: Vec<f32> = chart
        .visualization
        .data()
        .iter()
        .map(|m| m.position.y)
        .collect();

    assert_eq!(chart.len(), 2, "Should have 2 box plots");
    assert!(
        positions[1] > positions[0],
        "Y positions should differ: {:?}",
        positions
    );
}

#[tokio::test]
async fn test_varying_sample_sizes() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        // Category A: 2 values
        CategoryData {
            category: "A".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 20.0,
        },
        // Category B: 5 values
        CategoryData {
            category: "B".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 35.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 40.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 45.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 50.0,
        },
        // Category C: 3 values
        CategoryData {
            category: "C".to_string(),
            value: 60.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 65.0,
        },
        CategoryData {
            category: "C".to_string(),
            value: 70.0,
        },
    ];

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .build_with_data(data, context)
        .unwrap();

    assert_eq!(chart.len(), 3, "Should handle varying sample sizes");

    // Each category should have valid statistical values
    for attrs in chart.visualization.data() {
        assert!(attrs.min <= attrs.q1);
        assert!(attrs.q1 <= attrs.median);
        assert!(attrs.median <= attrs.q3);
        assert!(attrs.q3 <= attrs.max);
    }
}

#[tokio::test]
async fn test_outliers_per_category() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        // Category A with outlier
        CategoryData {
            category: "A".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 12.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 14.0,
        },
        CategoryData {
            category: "A".to_string(),
            value: 50.0,
        }, // outlier
        // Category B without outliers
        CategoryData {
            category: "B".to_string(),
            value: 30.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 32.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 34.0,
        },
        CategoryData {
            category: "B".to_string(),
            value: 36.0,
        },
    ];

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .order_alphabetically()
        .build_with_data(data, context)
        .unwrap();

    // Category A (first) should have outliers
    assert!(
        !chart.visualization.data()[0].outliers.is_empty(),
        "Category A should have outliers"
    );

    // Category B (second) should have no outliers
    assert!(
        chart.visualization.data()[1].outliers.is_empty(),
        "Category B should have no outliers"
    );
}

#[tokio::test]
async fn test_many_categories() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    // Create data for 10 categories
    let mut data = Vec::new();
    for i in 0..10 {
        let category = format!("Cat{:02}", i);
        for j in 0..5 {
            data.push(CategoryData {
                category: category.clone(),
                value: (i * 10 + j) as f32,
            });
        }
    }

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .build_with_data(data, context)
        .unwrap();

    assert_eq!(chart.len(), 10, "Should handle 10 categories");

    // Verify positions are distinct
    let positions: Vec<f32> = chart
        .visualization
        .data()
        .iter()
        .map(|m| m.position.x)
        .collect();
    for i in 0..positions.len() - 1 {
        assert!(
            positions[i] < positions[i + 1],
            "Positions should be strictly increasing"
        );
    }
}

#[tokio::test]
async fn test_single_category_fallback() {
    let context = Arc::new(RenderContext::new().await.unwrap());

    let data = vec![
        CategoryData {
            category: "Only".to_string(),
            value: 10.0,
        },
        CategoryData {
            category: "Only".to_string(),
            value: 20.0,
        },
        CategoryData {
            category: "Only".to_string(),
            value: 30.0,
        },
    ];

    let chart = boxplot()
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .category(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::String(d.category.clone())
        }))
        .build_with_data(data, context)
        .unwrap();

    // Single category should still work
    assert_eq!(chart.len(), 1, "Should handle single category");
}
