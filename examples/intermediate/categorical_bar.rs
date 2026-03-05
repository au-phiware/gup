// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Categorical Bar Chart - Intermediate Example
//!
//! This example demonstrates bar charts with categorical data,
//! showing proper handling of discrete categories and value comparisons.
//!
//! ## What You'll Learn
//! - Working with categorical data
//! - Creating horizontal and vertical bar charts
//! - Color encoding for categories
//! - Comparative visualization techniques
//!
//! Run with: `cargo run --example categorical_bar`

use gup::prelude::*;
use std::sync::Arc;

// Categorical data structure
#[derive(Debug, Clone)]
struct CategoryData {
    category: String,
    value: f32,
    index: f32,
}

impl CategoryData {
    fn new(category: &str, value: f32, index: f32) -> Self {
        Self {
            category: category.to_string(),
            value,
            index,
        }
    }
}

// Generate product category sales data
fn generate_product_categories() -> Vec<CategoryData> {
    vec![
        CategoryData::new("Electronics", 4500.0, 0.0),
        CategoryData::new("Clothing", 3800.0, 1.0),
        CategoryData::new("Home & Garden", 5200.0, 2.0),
        CategoryData::new("Sports & Outdoors", 2900.0, 3.0),
        CategoryData::new("Books & Media", 3400.0, 4.0),
        CategoryData::new("Toys & Games", 4100.0, 5.0),
        CategoryData::new("Health & Beauty", 3600.0, 6.0),
        CategoryData::new("Automotive", 2700.0, 7.0),
    ]
}

// Generate regional sales data
fn generate_regional_sales() -> Vec<CategoryData> {
    vec![
        CategoryData::new("North America", 12500.0, 0.0),
        CategoryData::new("Europe", 9800.0, 1.0),
        CategoryData::new("Asia Pacific", 14500.0, 2.0),
        CategoryData::new("Latin America", 5200.0, 3.0),
        CategoryData::new("Middle East", 6800.0, 4.0),
        CategoryData::new("Africa", 4200.0, 5.0),
    ]
}

// Color based on value (gradient)
fn value_gradient_color(value: f32, max_value: f32) -> [f32; 4] {
    let normalized = value / max_value;
    // Blue to orange gradient
    [
        0.2 + normalized * 0.7,         // R: 0.2 -> 0.9
        0.4 + (1.0 - normalized) * 0.2, // G: 0.6 -> 0.4
        0.9 - normalized * 0.7,         // B: 0.9 -> 0.2
        1.0,
    ]
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("=== Categorical Bar Chart Example ===");
    println!();
    println!("This example demonstrates:");
    println!("  - Categorical data visualization");
    println!("  - Vertical and horizontal orientations");
    println!("  - Value-based color gradients");
    println!("  - Comparative analysis across categories");
    println!();

    // Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("GPU context initialized");

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let data = generate_product_categories();
        let mut chart = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.index)
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.value)
            }))
            .fill(AccessorFunction::new(|_: &CategoryData| {
                AccessorValue::Color([0.3, 0.7, 0.9, 1.0])
            }))
            .vertical()
            .build_with_data(data, context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    // ========================================
    // Example 1: Product Categories (Vertical)
    // ========================================
    println!("\n--- Example 1: Product Category Sales (Vertical Bars) ---");
    let product_data = generate_product_categories();
    let max_product = product_data
        .iter()
        .map(|d| d.value)
        .fold(f32::NEG_INFINITY, f32::max);

    println!("Categories: {}", product_data.len());
    println!("Maximum value: ${:.0}", max_product);
    println!();

    let x_accessor = AccessorFunction::new(|d: &CategoryData| AccessorValue::Float(d.index));
    let y_accessor = AccessorFunction::new(|d: &CategoryData| AccessorValue::Float(d.value));

    let product_chart = bar()
        .x(x_accessor.clone())
        .y(y_accessor.clone())
        .fill(AccessorFunction::new(|_: &CategoryData| {
            AccessorValue::Color([0.3, 0.7, 0.9, 1.0])
        }))
        .bar_width(AccessorFunction::new(|_: &CategoryData| {
            AccessorValue::Float(0.8)
        }))
        .vertical();

    let product_selection = product_chart.build_with_data(product_data.clone(), context.clone())?;
    println!(
        "Created vertical bar chart with {} bars",
        product_selection.len()
    );
    println!();

    // Print sorted by value
    let mut sorted_products = product_data.clone();
    sorted_products.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap());

    println!("Top Categories:");
    for (i, data) in sorted_products.iter().take(3).enumerate() {
        let color = value_gradient_color(data.value, max_product);
        println!(
            "  {}. {}: ${:.0} (Color: RGB({:.2}, {:.2}, {:.2}))",
            i + 1,
            data.category,
            data.value,
            color[0],
            color[1],
            color[2]
        );
    }
    println!();

    // ========================================
    // Example 2: Regional Sales (Horizontal)
    // ========================================
    println!("--- Example 2: Regional Sales (Horizontal Bars) ---");
    let regional_data = generate_regional_sales();
    let max_regional = regional_data
        .iter()
        .map(|d| d.value)
        .fold(f32::NEG_INFINITY, f32::max);

    println!("Regions: {}", regional_data.len());
    println!("Maximum value: ${:.0}", max_regional);
    println!();

    let regional_chart = bar()
        .x(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.value)
        }))
        .y(AccessorFunction::new(|d: &CategoryData| {
            AccessorValue::Float(d.index)
        }))
        .fill(AccessorFunction::new(|_: &CategoryData| {
            AccessorValue::Color([0.3, 0.7, 0.4, 1.0])
        })) // Green
        .bar_width(AccessorFunction::new(|_: &CategoryData| {
            AccessorValue::Float(0.7)
        }))
        .horizontal();

    let regional_selection = regional_chart.build_with_data(regional_data.clone(), context)?;
    println!(
        "Created horizontal bar chart with {} bars",
        regional_selection.len()
    );
    println!();

    // Print all regions with percentages
    let total_sales: f32 = regional_data.iter().map(|d| d.value).sum();
    println!("Regional Breakdown:");
    for data in &regional_data {
        let percentage = (data.value / total_sales) * 100.0;
        println!(
            "  {}: ${:.0} ({:.1}% of total)",
            data.category, data.value, percentage
        );
    }
    println!();

    println!("Summary:");
    println!("  ✓ Created {} bar charts (vertical + horizontal)", 2);
    println!(
        "  ✓ Total categories visualized: {}",
        product_data.len() + regional_data.len()
    );
    println!(
        "  ✓ Value ranges: ${:.0} - ${:.0}",
        product_data
            .iter()
            .map(|d| d.value)
            .fold(f32::INFINITY, f32::min),
        max_regional
    );
    println!();

    println!("Key Features Demonstrated:");
    println!("  ✓ Categorical data handling");
    println!("  ✓ Both vertical and horizontal orientations");
    println!("  ✓ Color gradients based on values");
    println!("  ✓ Custom bar widths");
    println!("  ✓ Percentage calculations");
    println!();

    println!("Next steps:");
    println!("  - Combine with scatter plots for multi-chart visualizations");
    println!("  - Add interactive tooltips (see advanced examples)");
    println!("  - Try stacked bar charts (future feature)");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_product_categories() {
        let data = generate_product_categories();
        assert_eq!(data.len(), 8);

        // Check indices are sequential
        for (i, d) in data.iter().enumerate() {
            assert_eq!(d.index, i as f32);
        }
    }

    #[test]
    fn test_generate_regional_sales() {
        let data = generate_regional_sales();
        assert_eq!(data.len(), 6);

        // Check all values are positive
        for d in &data {
            assert!(d.value > 0.0);
        }
    }

    #[test]
    fn test_value_gradient_color() {
        let color_low = value_gradient_color(0.0, 100.0);
        let color_high = value_gradient_color(100.0, 100.0);

        // Check that low and high values produce different colors
        assert_ne!(color_low, color_high);

        // Check alpha is always 1.0
        assert_eq!(color_low[3], 1.0);
        assert_eq!(color_high[3], 1.0);
    }

    #[tokio::test]
    async fn test_vertical_bar_chart() {
        let data = generate_product_categories();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.index)
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.value)
            }))
            .vertical();

        let selection = chart.build_with_data(data, context).unwrap();
        assert_eq!(selection.len(), 8);
    }

    #[tokio::test]
    async fn test_horizontal_bar_chart() {
        let data = generate_regional_sales();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.value)
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.index)
            }))
            .horizontal();

        let selection = chart.build_with_data(data, context).unwrap();
        assert_eq!(selection.len(), 6);
    }
}
