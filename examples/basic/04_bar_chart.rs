// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! # Bar Chart Example
//!
//! This example demonstrates how to create bar charts using
//! the Observable Plot-style API.
//!
//! ## What You'll Learn
//! - How to use the `bar()` chart builder
//! - How to configure bar orientation (vertical/horizontal)
//! - How to style bars with colors
//! - How to work with categorical data
//!
//! Run with: `cargo run --example 04_bar_chart`
//!
//! Note: The Rectangle mark is currently in development. This example
//! demonstrates the API, but full visual rendering will be available
//! in a future release.

use gup::chart_builder::builders::BarOrientation;
use gup::prelude::*;
use std::sync::Arc;

// ========================================
// Step 1: Define your categorical data
// ========================================
#[derive(Debug, Clone)]
struct SalesData {
    /// Category name (e.g., product name)
    category: String,
    /// Value (e.g., sales amount)
    value: f32,
    /// Optional: category index for x-axis positioning
    index: f32,
}

impl SalesData {
    fn new(category: &str, value: f32, index: f32) -> Self {
        Self {
            category: category.to_string(),
            value,
            index,
        }
    }
}

// ========================================
// Step 2: Create sample categorical data
// ========================================
fn create_quarterly_sales() -> Vec<SalesData> {
    vec![
        SalesData::new("Q1", 150.0, 0.0),
        SalesData::new("Q2", 230.0, 1.0),
        SalesData::new("Q3", 180.0, 2.0),
        SalesData::new("Q4", 310.0, 3.0),
    ]
}

fn create_product_sales() -> Vec<SalesData> {
    vec![
        SalesData::new("Widgets", 450.0, 0.0),
        SalesData::new("Gadgets", 680.0, 1.0),
        SalesData::new("Gizmos", 320.0, 2.0),
        SalesData::new("Doodads", 520.0, 3.0),
        SalesData::new("Thingamajigs", 290.0, 4.0),
    ]
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("=== Gup Bar Chart Example ===");
    println!();

    // Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("GPU context initialized");

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let data = create_quarterly_sales();
        let mut chart = bar()
            .x(AccessorFunction::new(|d: &SalesData| {
                AccessorValue::Float(d.index)
            }))
            .y(AccessorFunction::new(|d: &SalesData| {
                AccessorValue::Float(d.value)
            }))
            .show_grid(true)
            .show_axes(true)
            .title("Quarterly Sales")
            .build_with_data(data, context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    // ========================================
    // Example 1: Basic vertical bar chart
    // ========================================
    println!("\n--- Example 1: Basic Vertical Bar Chart ---");
    let quarterly_data = create_quarterly_sales();
    println!("Quarterly sales data: {} categories", quarterly_data.len());

    let vertical_chart = bar()
        .x(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.index)
        }))
        .y(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.value)
        }))
        .vertical() // Default orientation
        .title("Quarterly Sales");

    let vertical_selection =
        vertical_chart.build_with_data(quarterly_data.clone(), context.clone())?;
    println!(
        "Vertical bar chart created with {} bars",
        vertical_selection.len()
    );

    // ========================================
    // Example 2: Horizontal bar chart
    // ========================================
    println!("\n--- Example 2: Horizontal Bar Chart ---");
    let product_data = create_product_sales();
    println!("Product sales data: {} products", product_data.len());

    let horizontal_chart = bar()
        .x(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.value)
        }))
        .y(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.index)
        }))
        .horizontal()
        .title("Product Sales Comparison");

    let horizontal_selection =
        horizontal_chart.build_with_data(product_data.clone(), context.clone())?;
    println!(
        "Horizontal bar chart created with {} bars",
        horizontal_selection.len()
    );

    // ========================================
    // Example 3: Colored bar chart
    // ========================================
    println!("\n--- Example 3: Colored Bar Chart ---");

    // Color accessor based on value (gradient from blue to green)
    let color_accessor = AccessorFunction::new(|d: &SalesData| {
        let normalized = (d.value - 100.0) / 300.0; // Normalize to 0-1
        let normalized = normalized.clamp(0.0, 1.0);
        AccessorValue::Color([
            0.2,                    // Red (low)
            0.4 + normalized * 0.4, // Green (increases with value)
            0.8 - normalized * 0.4, // Blue (decreases with value)
            0.9,                    // Alpha
        ])
    });

    let colored_chart = bar()
        .x(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.index)
        }))
        .y(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.value)
        }))
        .color(color_accessor)
        .title("Sales by Quarter (Color-coded)");

    let colored_selection =
        colored_chart.build_with_data(quarterly_data.clone(), context.clone())?;
    println!(
        "Colored bar chart: {} bars with value-based colors",
        colored_selection.len()
    );

    // ========================================
    // Example 4: Bar chart with grid
    // ========================================
    println!("\n--- Example 4: Bar Chart with Grid ---");
    let grid_chart = bar()
        .x(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.index)
        }))
        .y(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.value)
        }))
        .show_grid(true)
        .show_axes(true)
        .title("Quarterly Sales with Grid");

    let grid_selection = grid_chart.build_with_data(quarterly_data.clone(), context.clone())?;
    println!(
        "Bar chart with grid: {} bars, grid=enabled",
        grid_selection.len()
    );

    // ========================================
    // Example 5: Stacked bar chart (API demo)
    // ========================================
    println!("\n--- Example 5: Stacked Bar Chart (API Demo) ---");
    let stacked_chart = bar()
        .x(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.index)
        }))
        .y(AccessorFunction::new(|d: &SalesData| {
            AccessorValue::Float(d.value)
        }))
        .stack() // Enable stacking mode
        .title("Stacked Sales");

    let stacked_selection =
        stacked_chart.build_with_data(quarterly_data.clone(), context.clone())?;
    println!(
        "Stacked bar chart: {} bars (stacking mode enabled)",
        stacked_selection.len()
    );

    // ========================================
    // Summary
    // ========================================
    println!("\n=== Summary ===");
    println!("The bar() chart builder supports:");
    println!("  - Vertical bars: .vertical() (default)");
    println!("  - Horizontal bars: .horizontal()");
    println!("  - Custom colors: .color(accessor) or .fill(accessor)");
    println!("  - Bar width: .bar_width(accessor)");
    println!("  - Stacking: .stack()");
    println!("  - Grid and axes: .show_grid(true), .show_axes(true)");
    println!();
    println!("Available orientations:");
    let orientations = [BarOrientation::Vertical, BarOrientation::Horizontal];
    for orientation in orientations {
        println!("  {:?}", orientation);
    }
    println!();
    println!("Example categories:");
    for data in create_quarterly_sales() {
        println!("  {}: ${:.0}k", data.category, data.value);
    }
    println!();
    println!("Next steps:");
    println!("  - See 01_hello_chart.rs for the basics");
    println!("  - See 02_scatter_window.rs for visual window rendering");
    println!("  - See 03_line_chart.rs for line charts");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_bar_chart() {
        let data = create_quarterly_sales();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = bar()
            .x(AccessorFunction::new(|d: &SalesData| {
                AccessorValue::Float(d.index)
            }))
            .y(AccessorFunction::new(|d: &SalesData| {
                AccessorValue::Float(d.value)
            }));

        let selection = chart.build_with_data(data.clone(), context).unwrap();
        assert_eq!(selection.len(), data.len());
    }

    #[tokio::test]
    async fn test_horizontal_bar_chart() {
        let data = create_product_sales();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = bar()
            .x(AccessorFunction::new(|d: &SalesData| {
                AccessorValue::Float(d.value)
            }))
            .y(AccessorFunction::new(|d: &SalesData| {
                AccessorValue::Float(d.index)
            }))
            .horizontal();

        let selection = chart.build_with_data(data, context).unwrap();
        assert!(!selection.is_empty());
    }

    #[test]
    fn test_orientation_options() {
        // Test that orientation methods can be called without errors
        // (We can't access private fields, so we just verify the API works)
        let _vertical = bar::<SalesData>().vertical();
        let _horizontal = bar::<SalesData>().horizontal();
        // If we get here without panicking, the API works
    }

    #[test]
    fn test_data_creation() {
        let quarterly = create_quarterly_sales();
        assert_eq!(quarterly.len(), 4);

        let products = create_product_sales();
        assert_eq!(products.len(), 5);
    }

    #[test]
    fn test_stacking_mode() {
        // Test that stacking can be enabled without errors
        let _builder = bar::<SalesData>().stack();
        // If we get here without panicking, the API works
    }
}
