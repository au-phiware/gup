// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Multi-Series Line Chart - Intermediate Example
//!
//! This example demonstrates creating line charts with multiple data series,
//! each with different colors and interpolation methods.
//!
//! ## What You'll Learn
//! - Creating and managing multiple line series
//! - Using different colors for each series
//! - Working with time series data
//! - Building comparative visualizations
//!
//! Run with: `cargo run --example multi_series_line`

use gup::prelude::*;
use std::sync::Arc;

// Time series data point
#[derive(Debug, Clone)]
struct TimeSeriesPoint {
    month: f32,
    value: f32,
    series_name: String,
}

impl TimeSeriesPoint {
    fn new(month: f32, value: f32, series_name: &str) -> Self {
        Self {
            month,
            value,
            series_name: series_name.to_string(),
        }
    }
}

// Generate multi-series financial data
fn generate_financial_data() -> Vec<Vec<TimeSeriesPoint>> {
    let mut all_series = Vec::new();

    // Series 1: Stock Price
    let stock_price: Vec<TimeSeriesPoint> = (1..=12)
        .map(|month| {
            let base = 100.0;
            let trend = month as f32 * 3.0;
            let seasonal = (month as f32 * std::f32::consts::PI / 6.0).sin() * 10.0;
            let value = base + trend + seasonal;
            TimeSeriesPoint::new(month as f32, value, "Stock Price")
        })
        .collect();

    // Series 2: Revenue
    let revenue: Vec<TimeSeriesPoint> = (1..=12)
        .map(|month| {
            let base = 80.0;
            let trend = month as f32 * 2.5;
            let seasonal = (month as f32 * std::f32::consts::PI / 3.0).cos() * 8.0;
            let value = base + trend + seasonal;
            TimeSeriesPoint::new(month as f32, value, "Revenue")
        })
        .collect();

    // Series 3: Costs
    let costs: Vec<TimeSeriesPoint> = (1..=12)
        .map(|month| {
            let base = 60.0;
            let trend = month as f32 * 1.8;
            let seasonal = (month as f32 * std::f32::consts::PI / 4.0).sin() * 5.0;
            let value = base + trend + seasonal;
            TimeSeriesPoint::new(month as f32, value, "Operating Costs")
        })
        .collect();

    all_series.push(stock_price);
    all_series.push(revenue);
    all_series.push(costs);

    all_series
}

// Colors for each series
fn series_color(index: usize) -> [f32; 4] {
    match index {
        0 => [0.2, 0.6, 0.9, 1.0], // Blue for Stock Price
        1 => [0.3, 0.8, 0.3, 1.0], // Green for Revenue
        2 => [0.9, 0.4, 0.2, 1.0], // Orange for Costs
        _ => [0.5, 0.5, 0.5, 1.0],
    }
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("=== Multi-Series Line Chart Example ===");
    println!();
    println!("This example demonstrates:");
    println!("  - Multiple time series in one visualization");
    println!("  - Different colors for each series");
    println!("  - Financial data analysis");
    println!("  - Comparative trend visualization");
    println!();

    // Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("GPU context initialized");

    // Generate financial data
    let all_series = generate_financial_data();

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let series_data = &all_series[0];
        let mut chart = line()
            .x(AccessorFunction::new(|p: &TimeSeriesPoint| {
                AccessorValue::Float(p.month)
            }))
            .y(AccessorFunction::new(|p: &TimeSeriesPoint| {
                AccessorValue::Float(p.value)
            }))
            .stroke_color(series_color(0))
            .stroke_width_px(2.0)
            .title("Stock Price Trend")
            .build_with_data(series_data.clone(), context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    println!(
        "Generated {} time series with 12 months each",
        all_series.len()
    );
    println!();

    // Create a line chart for each series
    let mut charts = Vec::new();
    for (index, series_data) in all_series.iter().enumerate() {
        let color = series_color(index);
        let series_name = &series_data[0].series_name;

        println!("Series {}: {}", index + 1, series_name);
        println!("  Months: {}", series_data.len());
        println!(
            "  Color: RGB({:.1}, {:.1}, {:.1})",
            color[0], color[1], color[2]
        );

        let x_accessor = AccessorFunction::new(|p: &TimeSeriesPoint| AccessorValue::Float(p.month));
        let y_accessor = AccessorFunction::new(|p: &TimeSeriesPoint| AccessorValue::Float(p.value));

        let chart = line()
            .x(x_accessor)
            .y(y_accessor)
            .stroke_color(color)
            .stroke_width_px(2.0)
            .linear(); // Linear interpolation

        let selection = chart.build_with_data(series_data.clone(), context.clone())?;

        // Calculate statistics
        let values: Vec<f32> = series_data.iter().map(|p| p.value).collect();
        let min_val = values.iter().copied().fold(f32::INFINITY, f32::min);
        let max_val = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let avg_val = values.iter().sum::<f32>() / values.len() as f32;

        println!("  Min: {:.1}", min_val);
        println!("  Max: {:.1}", max_val);
        println!("  Avg: {:.1}", avg_val);
        println!("  Points created: {}", selection.len());
        println!();

        charts.push(selection);
    }

    println!("Summary:");
    println!("  ✓ Created {} line series", charts.len());
    println!(
        "  ✓ Total data points: {}",
        charts.iter().map(|c| c.len()).sum::<usize>()
    );
    println!("  ✓ Time range: 12 months");
    println!();

    // Analysis insights
    println!("Key Insights:");
    println!("  • Stock price shows upward trend with seasonal variation");
    println!("  • Revenue growth aligns with stock performance");
    println!("  • Operating costs increase at a slower rate");
    println!("  • Profit margin (Revenue - Costs) is increasing");
    println!();

    println!("Success! Multi-series line charts demonstrate:");
    println!("  ✓ Temporal data visualization");
    println!("  ✓ Comparative analysis across metrics");
    println!("  ✓ Color-coded series identification");
    println!("  ✓ Professional financial charting");
    println!();
    println!("Next steps:");
    println!("  - Run advanced/combined_charts for layered visualizations");
    println!("  - Try showcase examples for publication-quality charts");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_financial_data() {
        let series = generate_financial_data();
        assert_eq!(series.len(), 3);

        for s in &series {
            assert_eq!(s.len(), 12); // 12 months
        }
    }

    #[test]
    fn test_series_names() {
        let series = generate_financial_data();
        assert_eq!(series[0][0].series_name, "Stock Price");
        assert_eq!(series[1][0].series_name, "Revenue");
        assert_eq!(series[2][0].series_name, "Operating Costs");
    }

    #[test]
    fn test_series_color() {
        for i in 0..3 {
            let color = series_color(i);
            assert_eq!(color.len(), 4);
            assert_eq!(color[3], 1.0); // Alpha should be 1.0
        }
    }

    #[tokio::test]
    async fn test_line_chart_creation() {
        let series = generate_financial_data();
        let context = Arc::new(RenderContext::new().await.unwrap());

        for series_data in series.iter() {
            let chart = line()
                .x(AccessorFunction::new(|p: &TimeSeriesPoint| {
                    AccessorValue::Float(p.month)
                }))
                .y(AccessorFunction::new(|p: &TimeSeriesPoint| {
                    AccessorValue::Float(p.value)
                }));

            let selection = chart
                .build_with_data(series_data.clone(), context.clone())
                .unwrap();
            assert_eq!(selection.len(), 12);
        }
    }

    #[test]
    fn test_data_statistics() {
        let series = generate_financial_data();

        for series_data in series.iter() {
            let values: Vec<f32> = series_data.iter().map(|p| p.value).collect();
            let min = values.iter().copied().fold(f32::INFINITY, f32::min);
            let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);

            assert!(max > min);
            assert!(min > 0.0);
        }
    }
}
