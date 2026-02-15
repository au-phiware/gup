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

//! # Hello Chart - The Simplest Gup Example
//!
//! This is the minimal example to get started with Gup.
//! It creates a simple scatter plot with 5 data points.
//!
//! ## What You'll Learn
//! - How to define a data structure for your chart
//! - How to create accessor functions to map data to visual properties
//! - How to use the Observable Plot-style `scatter()` API
//! - How to build a chart with GPU acceleration
//!
//! Run with: `cargo run --example 01_hello_chart`

use gup::prelude::*;
use std::sync::Arc;

// Step 1: Define your data structure
// Your data can be any Rust struct with Debug and Clone traits
#[derive(Debug, Clone)]
struct Point {
    x: f32,
    y: f32,
}

#[tokio::main]
async fn main() -> GupResult<()> {
    // Step 2: Create your data
    // This is a simple dataset with 5 points
    let data = vec![
        Point { x: 1.0, y: 2.0 },
        Point { x: 2.0, y: 4.0 },
        Point { x: 3.0, y: 3.0 },
        Point { x: 4.0, y: 5.0 },
        Point { x: 5.0, y: 4.5 },
    ];

    // Step 3: Initialize GPU context
    // This creates a connection to the GPU for hardware-accelerated rendering
    let context = Arc::new(RenderContext::new().await?);

    // Step 4: Create accessor functions
    // Accessors tell Gup how to extract values from your data structure
    let x_accessor = AccessorFunction::new(|point: &Point| AccessorValue::Float(point.x));
    let y_accessor = AccessorFunction::new(|point: &Point| AccessorValue::Float(point.y));

    // Step 5: Create a scatter plot using the Observable Plot-style API
    // The fluent API lets you chain configuration methods
    let chart = scatter()
        .x(x_accessor) // Map x field to x-axis
        .y(y_accessor) // Map y field to y-axis
        .title("Hello Gup!") // Set chart title
        .point_size(10.0) // Set point radius in pixels
        .fill_color([0.2, 0.6, 0.9, 1.0]); // Set point color (RGBA)

    // Step 6: Build the chart with your data
    // This creates a GPU-accelerated selection ready for rendering
    let selection = chart.build_with_data(data, context)?;

    // Success! The chart is ready for rendering
    println!("Hello Gup!");
    println!("Created a scatter plot with {} points", selection.len());
    println!("GPU-accelerated and ready for rendering!");
    println!();
    println!("Next steps:");
    println!("  - See 02_scatter_window.rs to display this in a window");
    println!("  - See 03_line_chart.rs for line charts");
    println!("  - See 04_bar_chart.rs for bar charts");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hello_chart_creates_selection() {
        let data = vec![Point { x: 1.0, y: 2.0 }, Point { x: 2.0, y: 4.0 }];
        let context = Arc::new(RenderContext::new().await.unwrap());

        let x_accessor = AccessorFunction::new(|p: &Point| AccessorValue::Float(p.x));
        let y_accessor = AccessorFunction::new(|p: &Point| AccessorValue::Float(p.y));

        let chart = scatter().x(x_accessor).y(y_accessor);
        let selection = chart.build_with_data(data, context).unwrap();

        assert_eq!(selection.len(), 2);
    }

    #[tokio::test]
    async fn test_hello_chart_with_styling() {
        let data = vec![Point { x: 1.0, y: 1.0 }];
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = scatter()
            .x(AccessorFunction::new(|p: &Point| AccessorValue::Float(p.x)))
            .y(AccessorFunction::new(|p: &Point| AccessorValue::Float(p.y)))
            .title("Test Chart")
            .point_size(15.0)
            .fill_color([1.0, 0.0, 0.0, 1.0]);

        let selection = chart.build_with_data(data, context).unwrap();
        assert_eq!(selection.len(), 1);
    }
}
