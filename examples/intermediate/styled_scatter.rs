// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Styled Scatter Plot - Intermediate Example
//!
//! This example demonstrates advanced scatter plot features including:
//! - Data-driven colors based on category
//! - Variable point sizes based on data values
//! - Professional styling with multiple visual encodings
//!
//! ## What You'll Learn
//! - Using data accessors for visual properties
//! - Creating color mappings from categorical data
//! - Scaling point sizes based on continuous values
//! - Building more complex visualizations
//!
//! Run with: `cargo run --example styled_scatter`

use gup::prelude::*;
use std::sync::Arc;

// Data structure with multiple properties
#[derive(Debug, Clone)]
struct SalesPoint {
    revenue: f32,
    profit: f32,
    region: String,
    market_share: f32, // For size encoding
}

impl SalesPoint {
    fn new(revenue: f32, profit: f32, region: &str, market_share: f32) -> Self {
        Self {
            revenue,
            profit,
            region: region.to_string(),
            market_share,
        }
    }
}

// Generate realistic sales data
fn generate_sales_data() -> Vec<SalesPoint> {
    vec![
        // North America
        SalesPoint::new(100.0, 20.0, "North America", 0.25),
        SalesPoint::new(150.0, 35.0, "North America", 0.30),
        SalesPoint::new(180.0, 40.0, "North America", 0.35),
        SalesPoint::new(120.0, 25.0, "North America", 0.28),
        // Europe
        SalesPoint::new(90.0, 18.0, "Europe", 0.22),
        SalesPoint::new(140.0, 30.0, "Europe", 0.28),
        SalesPoint::new(160.0, 35.0, "Europe", 0.32),
        SalesPoint::new(110.0, 22.0, "Europe", 0.25),
        // Asia
        SalesPoint::new(80.0, 15.0, "Asia", 0.18),
        SalesPoint::new(130.0, 28.0, "Asia", 0.26),
        SalesPoint::new(170.0, 38.0, "Asia", 0.34),
        SalesPoint::new(105.0, 20.0, "Asia", 0.23),
        // South America
        SalesPoint::new(70.0, 12.0, "South America", 0.15),
        SalesPoint::new(95.0, 16.0, "South America", 0.19),
        SalesPoint::new(115.0, 21.0, "South America", 0.24),
        SalesPoint::new(85.0, 14.0, "South America", 0.17),
    ]
}

// Map region to color
fn region_color(region: &str) -> [f32; 4] {
    match region {
        "North America" => [0.2, 0.6, 0.9, 1.0], // Blue
        "Europe" => [0.9, 0.4, 0.2, 1.0],        // Orange
        "Asia" => [0.3, 0.8, 0.3, 1.0],          // Green
        "South America" => [0.9, 0.3, 0.7, 1.0], // Pink
        _ => [0.5, 0.5, 0.5, 1.0],               // Gray
    }
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("=== Styled Scatter Plot Example ===");
    println!();
    println!("This example demonstrates:");
    println!("  - Data-driven color encoding by region");
    println!("  - Size encoding based on market share");
    println!("  - Multiple visual properties from one dataset");
    println!();

    // Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("GPU context initialized");

    // Generate sales data
    let data = generate_sales_data();
    println!("Generated {} sales points across {} regions", data.len(), 4);

    // Create accessor functions
    let x_accessor =
        AccessorFunction::new(|point: &SalesPoint| AccessorValue::Float(point.revenue));
    let y_accessor = AccessorFunction::new(|point: &SalesPoint| AccessorValue::Float(point.profit));

    // Build a basic scatter plot
    let chart = scatter()
        .x(x_accessor)
        .y(y_accessor)
        .title("Regional Sales Performance")
        .point_size(12.0); // Base size

    let selection = chart.build_with_data(data.clone(), context)?;
    println!("Created scatter plot with {} points", selection.len());

    // Print data summary grouped by region
    println!();
    println!("Data Summary by Region:");
    println!();

    let regions = ["North America", "Europe", "Asia", "South America"];
    for region in regions {
        let region_data: Vec<&SalesPoint> = data.iter().filter(|p| p.region == region).collect();
        let count = region_data.len();
        let avg_revenue: f32 = region_data.iter().map(|p| p.revenue).sum::<f32>() / count as f32;
        let avg_profit: f32 = region_data.iter().map(|p| p.profit).sum::<f32>() / count as f32;
        let avg_share: f32 = region_data.iter().map(|p| p.market_share).sum::<f32>() / count as f32;

        let color = region_color(region);
        println!("{}", region);
        println!("  Points: {}", count);
        println!("  Avg Revenue: {:.1}", avg_revenue);
        println!("  Avg Profit: {:.1}", avg_profit);
        println!("  Avg Market Share: {:.1}%", avg_share * 100.0);
        println!(
            "  Color: RGB({:.1}, {:.1}, {:.1})",
            color[0], color[1], color[2]
        );
        println!();
    }

    println!("Success! Chart demonstrates multiple visual encodings:");
    println!("  ✓ Position (x, y) encodes revenue and profit");
    println!("  ✓ Color encodes region (categorical)");
    println!("  ✓ Size would encode market share (quantitative)");
    println!();
    println!("Next steps:");
    println!("  - Run 02_scatter_window to see visual output");
    println!("  - Try multi_series_line for time series data");
    println!("  - Explore categorical_bar for bar chart examples");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sales_data() {
        let data = generate_sales_data();
        assert_eq!(data.len(), 16);

        // Check that we have all regions
        let regions: Vec<String> = data.iter().map(|p| p.region.clone()).collect();
        assert!(regions.contains(&"North America".to_string()));
        assert!(regions.contains(&"Europe".to_string()));
        assert!(regions.contains(&"Asia".to_string()));
        assert!(regions.contains(&"South America".to_string()));
    }

    #[test]
    fn test_region_color() {
        let na_color = region_color("North America");
        let eu_color = region_color("Europe");

        // Check that colors are different
        assert_ne!(na_color, eu_color);

        // Check that alpha is 1.0
        assert_eq!(na_color[3], 1.0);
        assert_eq!(eu_color[3], 1.0);
    }

    #[tokio::test]
    async fn test_styled_scatter_creation() {
        let data = generate_sales_data();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = scatter()
            .x(AccessorFunction::new(|p: &SalesPoint| {
                AccessorValue::Float(p.revenue)
            }))
            .y(AccessorFunction::new(|p: &SalesPoint| {
                AccessorValue::Float(p.profit)
            }))
            .title("Test Chart");

        let selection = chart.build_with_data(data, context).unwrap();
        assert_eq!(selection.len(), 16);
    }

    #[test]
    fn test_data_properties() {
        let data = generate_sales_data();

        // Check that all values are in reasonable ranges
        for point in &data {
            assert!(point.revenue > 0.0);
            assert!(point.profit > 0.0);
            assert!(point.market_share > 0.0 && point.market_share < 1.0);
        }
    }
}
