// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-Category Box Plot Example
//!
//! Demonstrates grouped box plots for comparing distributions across categories.
//! Shows category grouping, automatic positioning, color differentiation, and
//! different ordering strategies.

use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{boxplot, AccessorFunction, ConfigurableBuilder};
use gup::chart_builder::ChartBuilder;
use gup::RenderContext;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Measurement {
    category: String,
    value: f32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Multi-Category Box Plot Example ===\n");

    // Create context
    let context = Arc::new(RenderContext::new().await?);

    // Sample data: test scores by grade level
    let test_scores = vec![
        // Grade 9 scores (lower mean)
        Measurement {
            category: "Grade 9".to_string(),
            value: 65.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 70.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 72.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 75.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 78.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 80.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 82.0,
        },
        Measurement {
            category: "Grade 9".to_string(),
            value: 45.0,
        }, // outlier
        // Grade 10 scores (medium mean)
        Measurement {
            category: "Grade 10".to_string(),
            value: 70.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 75.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 77.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 80.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 82.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 85.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 87.0,
        },
        Measurement {
            category: "Grade 10".to_string(),
            value: 90.0,
        },
        // Grade 11 scores (higher mean)
        Measurement {
            category: "Grade 11".to_string(),
            value: 75.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 80.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 82.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 85.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 87.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 90.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 92.0,
        },
        Measurement {
            category: "Grade 11".to_string(),
            value: 95.0,
        },
        // Grade 12 scores (highest mean)
        Measurement {
            category: "Grade 12".to_string(),
            value: 80.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 85.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 87.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 90.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 92.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 95.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 97.0,
        },
        Measurement {
            category: "Grade 12".to_string(),
            value: 100.0,
        },
    ];

    println!("Dataset: Test scores by grade level");
    println!("  Grades: 9, 10, 11, 12");
    println!("  Total measurements: {}", test_scores.len());
    println!();

    // Example 1: Basic multi-category box plot with alphabetical ordering
    println!("Example 1: Basic multi-category box plot (alphabetical order)");
    let _chart1 = boxplot()
        .y(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::Float(m.value)
        }))
        .category(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::String(m.category.clone())
        }))
        .title("Test Scores by Grade (Alphabetical)")
        .build_with_data(test_scores.clone(), context.clone())?;

    println!("  ✓ Created chart with alphabetically ordered categories");
    println!();

    // Example 2: Order by median value
    println!("Example 2: Order categories by median value");
    let _chart2 = boxplot()
        .y(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::Float(m.value)
        }))
        .category(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::String(m.category.clone())
        }))
        .order_by_median()
        .title("Test Scores by Grade (Ordered by Median)")
        .build_with_data(test_scores.clone(), context.clone())?;

    println!("  ✓ Categories ordered by median score");
    println!("    (Lower median grades appear first)");
    println!();

    // Example 3: Order by mean value
    println!("Example 3: Order categories by mean value");
    let _chart3 = boxplot()
        .y(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::Float(m.value)
        }))
        .category(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::String(m.category.clone())
        }))
        .order_by_mean()
        .title("Test Scores by Grade (Ordered by Mean)")
        .build_with_data(test_scores.clone(), context.clone())?;

    println!("  ✓ Categories ordered by mean score");
    println!();

    // Example 4: Custom spacing between categories
    println!("Example 4: Custom category spacing");
    let _chart4 = boxplot()
        .y(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::Float(m.value)
        }))
        .category(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::String(m.category.clone())
        }))
        .category_spacing(80.0)
        .box_width(50.0)
        .title("Test Scores by Grade (Wide Spacing)")
        .build_with_data(test_scores.clone(), context.clone())?;

    println!("  ✓ Wider spacing (80px) between categories");
    println!("  ✓ Wider boxes (50px) for better visibility");
    println!();

    // Example 5: Horizontal orientation with categories
    println!("Example 5: Horizontal multi-category box plot");
    let _chart5 = boxplot()
        .x(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::Float(m.value)
        }))
        .category(AccessorFunction::new(|m: &Measurement| {
            AccessorValue::String(m.category.clone())
        }))
        .horizontal()
        .order_alphabetically()
        .title("Test Scores by Grade (Horizontal)")
        .build_with_data(test_scores.clone(), context.clone())?;

    println!("  ✓ Horizontal orientation with category grouping");
    println!();

    // Example 6: Different dataset - Sales by region
    println!("Example 6: Sales by region with ordering");

    #[derive(Debug, Clone)]
    struct Sale {
        region: String,
        amount: f32,
    }

    let sales_data = vec![
        Sale {
            region: "North".to_string(),
            amount: 15000.0,
        },
        Sale {
            region: "North".to_string(),
            amount: 18000.0,
        },
        Sale {
            region: "North".to_string(),
            amount: 20000.0,
        },
        Sale {
            region: "North".to_string(),
            amount: 22000.0,
        },
        Sale {
            region: "South".to_string(),
            amount: 25000.0,
        },
        Sale {
            region: "South".to_string(),
            amount: 28000.0,
        },
        Sale {
            region: "South".to_string(),
            amount: 30000.0,
        },
        Sale {
            region: "South".to_string(),
            amount: 32000.0,
        },
        Sale {
            region: "East".to_string(),
            amount: 10000.0,
        },
        Sale {
            region: "East".to_string(),
            amount: 12000.0,
        },
        Sale {
            region: "East".to_string(),
            amount: 14000.0,
        },
        Sale {
            region: "East".to_string(),
            amount: 16000.0,
        },
        Sale {
            region: "West".to_string(),
            amount: 20000.0,
        },
        Sale {
            region: "West".to_string(),
            amount: 22000.0,
        },
        Sale {
            region: "West".to_string(),
            amount: 24000.0,
        },
        Sale {
            region: "West".to_string(),
            amount: 50000.0,
        }, // outlier
    ];

    let _chart6 = boxplot()
        .y(AccessorFunction::new(|s: &Sale| {
            AccessorValue::Float(s.amount)
        }))
        .category(AccessorFunction::new(|s: &Sale| {
            AccessorValue::String(s.region.clone())
        }))
        .order_by_median()
        .title("Sales by Region (Ordered by Median)")
        .width(1200.0)
        .height(600.0)
        .build_with_data(sales_data, context)?;

    println!("  ✓ Sales data grouped by region");
    println!("  ✓ Ordered by median sales value");
    println!("  ✓ Outlier (West region: $50,000) clearly visible");
    println!();

    println!("=== Key Features Demonstrated ===");
    println!("✓ Category-based grouping of data");
    println!("✓ Automatic positioning of box plots");
    println!("✓ Multiple ordering strategies:");
    println!("  - Alphabetical (default)");
    println!("  - By data appearance");
    println!("  - By median value");
    println!("  - By mean value");
    println!("✓ Configurable spacing between categories");
    println!("✓ Support for both vertical and horizontal orientations");
    println!("✓ Handles outliers within each category");
    println!("✓ Efficient computation for multiple groups");
    println!();

    println!("=== Example Complete ===");
    println!("Multi-category box plots provide clear visual comparison");
    println!("of distributions across different groups or categories.");
    println!();

    Ok(())
}
