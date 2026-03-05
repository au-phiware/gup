// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! SVG Export Example
//!
//! Demonstrates exporting a chart to SVG format without requiring a GPU.
//! The example creates a `ComposedChart` with axes, a title, and data marks
//! (circles for a scatter plot) and writes the result to `output.svg`.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example svg_export
//! ```

use gup::chart_builder::{ChartConfig, ComposedChart, Margins, TitleAlignment, TitleConfig};
use gup::export::svg::element::rgba_to_css;
use gup::export::svg::{SvgElement, SvgExportOptions};
use gup::prelude::*;
use std::sync::Arc;

/// A data point for the scatter plot.
#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
    value: f32,
}

/// Generate sample data representing the relationship between study hours
/// and test scores.
fn sample_data() -> Vec<DataPoint> {
    vec![
        DataPoint {
            x: 1.0,
            y: 45.0,
            value: 1.0,
        },
        DataPoint {
            x: 2.0,
            y: 55.0,
            value: 2.0,
        },
        DataPoint {
            x: 3.0,
            y: 60.0,
            value: 3.0,
        },
        DataPoint {
            x: 4.0,
            y: 65.0,
            value: 4.0,
        },
        DataPoint {
            x: 5.0,
            y: 72.0,
            value: 5.0,
        },
        DataPoint {
            x: 6.0,
            y: 78.0,
            value: 6.0,
        },
        DataPoint {
            x: 7.0,
            y: 82.0,
            value: 7.0,
        },
        DataPoint {
            x: 8.0,
            y: 88.0,
            value: 8.0,
        },
        DataPoint {
            x: 9.0,
            y: 92.0,
            value: 9.0,
        },
        DataPoint {
            x: 10.0,
            y: 95.0,
            value: 10.0,
        },
    ]
}

/// Convert a data point to an SVG circle element in the chart's coordinate
/// system.
fn data_point_to_svg(
    point: &DataPoint,
    chart_x: f32,
    chart_y: f32,
    chart_width: f32,
    chart_height: f32,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
) -> SvgElement {
    // Map data coordinates to chart area pixel coordinates
    let px = chart_x + (point.x - x_min) / (x_max - x_min) * chart_width;
    let py = chart_y + chart_height - (point.y - y_min) / (y_max - y_min) * chart_height;

    // Colour gradient: blue → red based on value
    let t = (point.value - 1.0) / 9.0;
    let r = t;
    let g = 0.2;
    let b = 1.0 - t;

    SvgElement::Circle {
        cx: px,
        cy: py,
        r: 6.0,
        fill: rgba_to_css(r, g, b, 0.8),
        stroke: Some(rgba_to_css(0.0, 0.0, 0.0, 0.3)),
        stroke_width: Some(1.0),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SVG Export Example");
    println!("==================");

    let data = sample_data();

    // Data range for mapping
    let x_min = 0.0_f32;
    let x_max = 11.0_f32;
    let y_min = 30.0_f32;
    let y_max = 100.0_f32;

    // Chart configuration
    let config = ChartConfig {
        title_config: Some(
            TitleConfig::new("Study Hours vs Test Score")
                .with_alignment(TitleAlignment::Center)
                .with_subtitle("Sample scatter plot exported to SVG"),
        ),
        width: 800.0,
        height: 600.0,
        margins: Margins {
            top: 60.0,
            right: 40.0,
            bottom: 60.0,
            left: 60.0,
        },
        background_color: Some([1.0, 1.0, 1.0, 1.0]),
        show_axes: true,
        show_grid: true,
        ..ChartConfig::default()
    }
    .with_x_scale(LinearScale::new(x_min, x_max, -1.0, 1.0))
    .with_y_scale(LinearScale::new(y_min, y_max, -1.0, 1.0));

    // Build data mark SVG elements manually (no GPU required).
    // The chart area is computed from config dimensions minus margins.
    let chart_x = config.margins.left;
    let chart_y = config.margins.top;
    let chart_width = config.width - config.margins.left - config.margins.right;
    let chart_height = config.height - config.margins.top - config.margins.bottom;

    let data_marks: Vec<SvgElement> = data
        .iter()
        .map(|pt| {
            data_point_to_svg(
                pt,
                chart_x,
                chart_y,
                chart_width,
                chart_height,
                x_min,
                x_max,
                y_min,
                y_max,
            )
        })
        .collect();

    // Create the chart (needs a GPU context for the Selection, so we use
    // a minimal headless context).
    let rt = tokio::runtime::Runtime::new()?;

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        rt.block_on(async {
            let context = Arc::new(gup::RenderContext::new().await?);
            let selection = gup::selection::Selection::<DataPoint, gup::Circle>::new(
                data.clone(),
                context,
            )?;
            let mut chart = ComposedChart::new(selection, config.clone()).with_default_axes();
            chart.export_png(&req.path, req.width, req.height)?;
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
        return Ok(());
    }

    let svg = rt.block_on(async {
        let context = Arc::new(gup::RenderContext::new().await?);

        let selection = gup::selection::Selection::<DataPoint, gup::Circle>::new(vec![], context)?;

        let chart = ComposedChart::new(selection, config).with_default_axes();

        let options = SvgExportOptions::new(800, 600)
            .with_background([1.0, 1.0, 1.0, 1.0])
            .with_css(".marks circle:hover { opacity: 1; stroke-width: 2; stroke: black; }");

        chart.export_svg_with_marks(&options, &data_marks)
    })?;

    // Write to file
    let output_path = "output.svg";
    std::fs::write(output_path, &svg)?;
    println!("SVG written to {output_path}");
    println!("SVG size: {} bytes", svg.len());

    // Also print a summary
    let circle_count = svg.matches("<circle").count();
    let line_count = svg.matches("<line").count();
    let text_count = svg.matches("<text").count();
    println!("Elements: {circle_count} circles, {line_count} lines, {text_count} text labels");

    Ok(())
}
