// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box Plot Builder API Example
//!
//! Demonstrates the Observable Plot-style builder API for creating box plots.
//! Shows the fluent API, statistical computation, and configuration options.

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, boxplot,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct MeasurementSet {
    category: String,
    values: Vec<f32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Box Plot Builder API Demo ===\n");

    // Create some sample data with different distributions
    let data = vec![
        MeasurementSet {
            category: "Control".to_string(),
            values: vec![
                42.0, 45.0, 48.0, 50.0, 52.0, 54.0, 56.0, 58.0, 60.0, 62.0, 44.0, 46.0, 48.0, 52.0,
                54.0, 56.0, 58.0,
            ],
        },
        MeasurementSet {
            category: "Treatment A".to_string(),
            values: vec![
                52.0, 55.0, 58.0, 61.0, 64.0, 67.0, 70.0, 73.0, 76.0, 79.0, 82.0, 54.0, 57.0, 60.0,
                63.0, 100.0, // outlier
            ],
        },
        MeasurementSet {
            category: "Treatment B".to_string(),
            values: vec![
                35.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0, 46.0, 47.0, 48.0,
            ],
        },
    ];

    println!("Sample data:");
    for dataset in &data {
        println!("  {}: {} values", dataset.category, dataset.values.len());
    }
    println!();

    // Create render context
    let context = Arc::new(RenderContext::new().await?);

    // Example 1: Basic box plot with minimal configuration
    println!("Example 1: Basic box plot");
    let basic_chart = boxplot()
        .y(AccessorFunction::new(|d: &MeasurementSet| {
            AccessorValue::FloatArray(d.values.clone())
        }))
        .title("Basic Box Plot")
        .build_with_data(data.clone(), context.clone())?;

    println!("  Created basic box plot with default settings");
    println!("  Chart has {} box plot(s)", basic_chart.len());
    println!();

    // Example 2: Vertical box plot with custom styling
    println!("Example 2: Vertical box plot with custom styling");
    let _styled_chart = boxplot()
        .y(AccessorFunction::new(|d: &MeasurementSet| {
            AccessorValue::FloatArray(d.values.clone())
        }))
        .vertical()
        .box_width(60.0)
        .fill_color([0.2, 0.6, 1.0, 0.8])
        .title("Styled Box Plot")
        .width(1000.0)
        .height(600.0)
        .build_with_data(data.clone(), context.clone())?;

    println!("  Created styled box plot with:");
    println!("    - Width: 60.0 pixels");
    println!("    - Fill color: blue");
    println!("    - Dimensions: 1000x600");
    println!();

    // Example 3: Box plot with grid styling
    println!("Example 3: Box plot with professional grid");
    let _grid_chart = boxplot()
        .y(AccessorFunction::new(|d: &MeasurementSet| {
            AccessorValue::FloatArray(d.values.clone())
        }))
        .vertical()
        .title("Box Plot with Grid")
        .light_grid()
        .show_axes(true)
        .build_with_data(data.clone(), context.clone())?;

    println!("  Created box plot with:");
    println!("    - Light theme grid");
    println!("    - Axes enabled");
    println!("    - Vertical orientation");
    println!();

    // Example 4: Using single values instead of arrays
    println!("Example 4: Box plot from individual data points");

    #[derive(Debug, Clone)]
    struct DataPoint {
        group: String,
        value: f32,
    }

    let individual_data: Vec<DataPoint> = vec![
        DataPoint {
            group: "A".to_string(),
            value: 10.0,
        },
        DataPoint {
            group: "A".to_string(),
            value: 15.0,
        },
        DataPoint {
            group: "A".to_string(),
            value: 20.0,
        },
        DataPoint {
            group: "A".to_string(),
            value: 25.0,
        },
        DataPoint {
            group: "A".to_string(),
            value: 30.0,
        },
        DataPoint {
            group: "A".to_string(),
            value: 100.0,
        }, // outlier
    ];

    let _individual_chart = boxplot()
        .y(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.value)
        }))
        .title("Box Plot from Individual Points")
        .box_width(80.0)
        .build_with_data(individual_data, context.clone())?;

    println!("  Created box plot from individual data points");
    println!("  The builder automatically aggregates values into statistics");
    println!();

    // Example 5: Minimal configuration (Observable Plot style)
    println!("Example 5: Minimal Observable Plot-style API");
    let _minimal = boxplot()
        .y(AccessorFunction::new(|d: &MeasurementSet| {
            AccessorValue::FloatArray(d.values.clone())
        }))
        .build_with_data(data.clone(), context)?;

    println!("  Created box plot with just one line:");
    println!("  boxplot().y(accessor).build_with_data(data, context)?");
    println!("  Minimal API provides sensible defaults for all settings");
    println!();

    println!("=== Builder API Benefits ===");
    println!("✓ Fluent, chainable API");
    println!("✓ Automatic statistical computation");
    println!("✓ Observable Plot compatibility");
    println!("✓ Type-safe configuration");
    println!("✓ Sensible defaults for rapid prototyping");
    println!("✓ Full control when needed");
    println!();

    println!("=== Demo Complete ===");
    println!("All box plots created successfully using the builder API!");
    println!("The builder automatically computes:");
    println!("  - Quartiles (Q1, Median, Q3)");
    println!("  - Whiskers (min/max within 1.5×IQR)");
    println!("  - Outlier detection");
    println!();

    Ok(())
}
