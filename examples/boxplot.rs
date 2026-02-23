// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box Plot Visualization Example
//!
//! Demonstrates GPU-accelerated box plot statistical computations.
//! Shows how to compute quartiles, whiskers, and detect outliers using GUP-139 statistics.

use gup::{BoxPlotAttributes, BoxPlotOrientation};
use gup::shader_function::Vec2;

fn main() {
    println!("\n=== Box Plot Statistical Analysis Demo ===\n");

    // Sample data for different distributions
    let datasets = vec![
        ("Normal Distribution", vec![
            42.0, 45.0, 48.0, 50.0, 52.0, 54.0, 56.0, 58.0, 60.0, 62.0,
            44.0, 46.0, 48.0, 52.0, 54.0, 56.0, 58.0, 60.0, 50.0, 52.0,
            45.0, 47.0, 49.0, 51.0, 53.0, 55.0, 57.0, 59.0, 61.0, 48.0,
        ]),
        ("Skewed Distribution", vec![
            60.0, 62.0, 64.0, 66.0, 68.0, 70.0, 72.0, 75.0, 80.0, 85.0,
            61.0, 63.0, 65.0, 67.0, 69.0, 71.0, 76.0, 82.0, 88.0, 95.0,
            62.0, 64.0, 66.0, 68.0, 70.0, 73.0, 78.0, 84.0, 90.0, 65.0,
        ]),
        ("Uniform Distribution", vec![
            30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0, 32.0,
            33.0, 37.0, 42.0, 47.0, 52.0, 57.0, 62.0, 67.0, 69.0, 34.0,
            36.0, 39.0, 44.0, 49.0, 54.0, 59.0, 64.0, 68.0, 70.0, 38.0,
        ]),
        ("Distribution with Outliers", vec![
            42.0, 44.0, 45.0, 46.0, 47.0, 48.0, 49.0, 50.0, 51.0, 52.0,
            43.0, 44.0, 45.0, 46.0, 47.0, 48.0, 49.0, 50.0, 51.0, 52.0,
            // Add outliers
            15.0, 20.0, 75.0, 80.0, 85.0,
        ]),
    ];

    for (name, data) in datasets {
        println!("Dataset: {}", name);
        println!("  Sample size: {}", data.len());
        
        // Create box plot attributes - this computes all statistics using GUP-139
        let attrs = BoxPlotAttributes::from_data(
            &data,
            Vec2 { x: 0.0, y: 0.0 }, // position
            50.0,                     // width
            BoxPlotOrientation::Vertical,
        );

        // Display five-number summary
        println!("  Five-number summary:");
        println!("    Minimum:     {:.2}", attrs.min);
        println!("    Q1 (25th):   {:.2}", attrs.q1);
        println!("    Median:      {:.2}", attrs.median);
        println!("    Q3 (75th):   {:.2}", attrs.q3);
        println!("    Maximum:     {:.2}", attrs.max);
        
        // Display IQR
        let iqr = attrs.iqr();
        println!("  IQR:           {:.2}", iqr);
        
        // Display outliers
        if attrs.outliers.is_empty() {
            println!("  Outliers:      None");
        } else {
            println!("  Outliers:      {} detected", attrs.outliers.len());
            for outlier in &attrs.outliers {
                println!("    - {:.2}", outlier);
            }
        }
        
        // Verify outlier detection
        let lower_fence = attrs.q1 - 1.5 * iqr;
        let upper_fence = attrs.q3 + 1.5 * iqr;
        println!("  Outlier fences:");
        println!("    Lower:       {:.2}", lower_fence);
        println!("    Upper:       {:.2}", upper_fence);
        
        println!();
    }

    println!("=== Box Plot Statistics Demonstration Complete ===");
    println!("\nNote: This example demonstrates statistical computation.");
    println!("Full GPU rendering of box plots will be integrated with the Selection API.");
}
