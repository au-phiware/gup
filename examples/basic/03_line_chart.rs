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

//! # Line Chart Example
//!
//! This example demonstrates how to create a line chart using
//! the Observable Plot-style API.
//!
//! ## What You'll Learn
//! - How to use the `line()` chart builder
//! - How to configure line interpolation methods
//! - How to style lines with stroke color and width
//! - How to work with time series data
//!
//! Run with: `cargo run --example 03_line_chart`
//!
//! Note: The Line mark is currently in development. This example
//! demonstrates the API, but full visual rendering will be available
//! in a future release.

use gup::chart_builder::builders::LineInterpolation;
use gup::prelude::*;
use std::sync::Arc;

// ========================================
// Step 1: Define your time series data
// ========================================
#[derive(Debug, Clone)]
struct TimePoint {
    /// Time value (e.g., month index)
    time: f32,
    /// Measurement value
    value: f32,
    /// Optional series identifier
    series: String,
}

impl TimePoint {
    fn new(time: f32, value: f32, series: &str) -> Self {
        Self {
            time,
            value,
            series: series.to_string(),
        }
    }
}

// ========================================
// Step 2: Create sample time series data
// ========================================
fn create_sample_data() -> Vec<TimePoint> {
    // Simulated monthly temperature data
    vec![
        TimePoint::new(1.0, 5.0, "Temperature"),   // January
        TimePoint::new(2.0, 7.0, "Temperature"),   // February
        TimePoint::new(3.0, 12.0, "Temperature"),  // March
        TimePoint::new(4.0, 18.0, "Temperature"),  // April
        TimePoint::new(5.0, 23.0, "Temperature"),  // May
        TimePoint::new(6.0, 27.0, "Temperature"),  // June
        TimePoint::new(7.0, 29.0, "Temperature"),  // July
        TimePoint::new(8.0, 28.0, "Temperature"),  // August
        TimePoint::new(9.0, 24.0, "Temperature"),  // September
        TimePoint::new(10.0, 17.0, "Temperature"), // October
        TimePoint::new(11.0, 10.0, "Temperature"), // November
        TimePoint::new(12.0, 6.0, "Temperature"),  // December
    ]
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("=== Gup Line Chart Example ===");
    println!();

    // Step 3: Create the data
    let data = create_sample_data();
    println!("Created time series data with {} points", data.len());

    // Step 4: Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("GPU context initialized");

    // Step 5: Create accessor functions
    let x_accessor = AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.time));
    let y_accessor = AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.value));

    // ========================================
    // Example 1: Basic line chart
    // ========================================
    println!("\n--- Example 1: Basic Line Chart ---");
    let basic_chart = line()
        .x(x_accessor.clone())
        .y(y_accessor.clone())
        .title("Monthly Temperature");

    let selection = basic_chart.build_with_data(data.clone(), context.clone())?;
    println!(
        "Basic line chart created with {} data points",
        selection.len()
    );

    // ========================================
    // Example 2: Styled line chart
    // ========================================
    println!("\n--- Example 2: Styled Line Chart ---");
    let styled_chart = line()
        .x(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.time)))
        .y(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.value)))
        .stroke_color([0.2, 0.6, 1.0, 1.0]) // Blue line
        .stroke_width_px(3.0)
        .title("Temperature Trend")
        .width(800.0)
        .height(400.0);

    let styled_selection = styled_chart.build_with_data(data.clone(), context.clone())?;
    println!(
        "Styled line chart: {} points, 3px stroke, blue color",
        styled_selection.len()
    );

    // ========================================
    // Example 3: Smooth curve interpolation
    // ========================================
    println!("\n--- Example 3: Smooth Curve ---");
    let smooth_chart = line()
        .x(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.time)))
        .y(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.value)))
        .smooth() // Use smooth curve interpolation
        .stroke_color([1.0, 0.4, 0.2, 1.0]) // Orange line
        .title("Smooth Temperature Curve");

    let smooth_selection = smooth_chart.build_with_data(data.clone(), context.clone())?;
    println!(
        "Smooth curve: {} points, interpolation=Curve",
        smooth_selection.len()
    );

    // ========================================
    // Example 4: Step function interpolation
    // ========================================
    println!("\n--- Example 4: Step Function ---");
    let step_chart = line()
        .x(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.time)))
        .y(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.value)))
        .step() // Step-before interpolation
        .stroke_color([0.2, 0.8, 0.4, 1.0]) // Green line
        .stroke_width_px(2.0)
        .title("Step Temperature");

    let step_selection = step_chart.build_with_data(data.clone(), context.clone())?;
    println!(
        "Step function: {} points, interpolation=StepBefore",
        step_selection.len()
    );

    // ========================================
    // Example 5: Line chart with grid
    // ========================================
    println!("\n--- Example 5: Line Chart with Grid ---");
    let grid_chart = line()
        .x(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.time)))
        .y(AccessorFunction::new(|p: &TimePoint| AccessorValue::Float(p.value)))
        .stroke_color([0.4, 0.2, 0.8, 1.0]) // Purple line
        .show_grid(true)
        .show_axes(true)
        .title("Temperature with Grid");

    let grid_selection = grid_chart.build_with_data(data.clone(), context.clone())?;
    println!(
        "Grid chart: {} points, grid=enabled, axes=enabled",
        grid_selection.len()
    );

    // ========================================
    // Summary
    // ========================================
    println!("\n=== Summary ===");
    println!("The line() chart builder supports:");
    println!("  - Basic line charts with x/y accessors");
    println!("  - Custom stroke colors and widths");
    println!("  - Interpolation methods:");
    println!("    - .linear() - straight lines (default)");
    println!("    - .smooth() - curved interpolation");
    println!("    - .step()   - step function");
    println!("  - Grid and axes display");
    println!("  - Titles and dimensions");
    println!();
    println!("Available interpolation modes:");
    let modes = [
        LineInterpolation::Linear,
        LineInterpolation::Curve,
        LineInterpolation::StepBefore,
        LineInterpolation::StepAfter,
    ];
    for mode in modes {
        println!("  {:?}", mode);
    }
    println!();
    println!("Next steps:");
    println!("  - See 02_scatter_window.rs for visual window rendering");
    println!("  - See 04_bar_chart.rs for bar charts");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_line_chart() {
        let data = create_sample_data();
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = line()
            .x(AccessorFunction::new(|p: &TimePoint| {
                AccessorValue::Float(p.time)
            }))
            .y(AccessorFunction::new(|p: &TimePoint| {
                AccessorValue::Float(p.value)
            }));

        let selection = chart.build_with_data(data.clone(), context).unwrap();
        assert_eq!(selection.len(), data.len());
    }

    #[tokio::test]
    async fn test_styled_line_chart() {
        let data = vec![
            TimePoint::new(1.0, 10.0, "A"),
            TimePoint::new(2.0, 20.0, "A"),
        ];
        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = line()
            .x(AccessorFunction::new(|p: &TimePoint| {
                AccessorValue::Float(p.time)
            }))
            .y(AccessorFunction::new(|p: &TimePoint| {
                AccessorValue::Float(p.value)
            }))
            .stroke_color([1.0, 0.0, 0.0, 1.0])
            .stroke_width_px(2.0);

        let selection = chart.build_with_data(data, context).unwrap();
        assert_eq!(selection.len(), 2);
    }

    #[test]
    fn test_interpolation_methods() {
        // Test that interpolation methods can be called without errors
        // (We can't access private fields, so we just verify the API works)
        let _builder = line::<TimePoint>().linear();
        let _builder = line::<TimePoint>().smooth();
        let _builder = line::<TimePoint>().step();
        // If we get here without panicking, the API works
    }

    #[test]
    fn test_data_creation() {
        let data = create_sample_data();
        assert_eq!(data.len(), 12); // 12 months
        assert!(data.iter().all(|p| p.series == "Temperature"));
    }
}
