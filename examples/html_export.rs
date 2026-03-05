// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! HTML Export Example
//!
//! Demonstrates exporting a Gup chart to a self-contained HTML file.
//! The example creates a `ComposedChart` with axes, a title, and grid
//! lines, then writes it to `chart.html` in the working directory.
//!
//! The exported HTML contains:
//!
//! * An SVG fallback for browsers without WebGPU.
//! * A PNG thumbnail in Open Graph `<meta>` tags.
//! * The chart definition serialised as JSON.
//! * A WASM bootstrap script (URL-based by default).
//!
//! Run with:
//!
//! ```sh
//! cargo run --example html_export
//! ```

use gup::chart_builder::{ChartConfig, ComposedChart, Margins, TitleAlignment, TitleConfig};
use gup::export::html::{HtmlExporter, WasmStrategy};
use gup::prelude::*;
use std::sync::Arc;

/// A data point for the scatter plot.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DataPoint {
    x: f32,
    y: f32,
}

/// Generate sample data.
fn sample_data() -> Vec<DataPoint> {
    vec![
        DataPoint { x: 1.0, y: 45.0 },
        DataPoint { x: 2.0, y: 55.0 },
        DataPoint { x: 3.0, y: 60.0 },
        DataPoint { x: 4.0, y: 65.0 },
        DataPoint { x: 5.0, y: 72.0 },
        DataPoint { x: 6.0, y: 78.0 },
        DataPoint { x: 7.0, y: 82.0 },
        DataPoint { x: 8.0, y: 88.0 },
        DataPoint { x: 9.0, y: 92.0 },
        DataPoint { x: 10.0, y: 95.0 },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("HTML Export Example");
    println!("===================");

    let data = sample_data();

    // Data range for axis scales
    let x_min = 0.0_f32;
    let x_max = 11.0_f32;
    let y_min = 30.0_f32;
    let y_max = 100.0_f32;

    let config = ChartConfig {
        title_config: Some(
            TitleConfig::new("Study Hours vs Test Score")
                .with_alignment(TitleAlignment::Center)
                .with_subtitle("Interactive HTML export with WebGPU & SVG fallback"),
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

    rt.block_on(async {
        let context = Arc::new(gup::RenderContext::new().await?);

        let selection = gup::selection::Selection::<DataPoint, gup::Circle>::new(data, context)?;

        let mut chart = ComposedChart::new(selection, config).with_default_axes();

        // --- Default export (URL strategy) ---
        let output_url = "chart.html";
        chart.export_html(output_url)?;
        let meta_url = std::fs::metadata(output_url)?;
        println!("Wrote {output_url} ({} bytes)", meta_url.len());

        // --- Custom export (URL strategy with full metadata) ---
        let output_custom = "chart_custom.html";
        let exporter =
            HtmlExporter::new(WasmStrategy::Url("https://cdn.example.com/gup.wasm".into()))
                .with_title("Custom Dashboard")
                .with_description("A scatter plot of study hours vs test scores")
                .with_author("Gup Example");
        exporter.export(&mut chart, output_custom)?;
        let meta_custom = std::fs::metadata(output_custom)?;
        println!("Wrote {output_custom} ({} bytes)", meta_custom.len());

        // Verify the output contains expected markers.
        let html = std::fs::read_to_string(output_url)?;
        let has_doctype = html.starts_with("<!DOCTYPE html>");
        let has_og = html.contains("og:image");
        let has_noscript = html.contains("<noscript>");
        let has_json = html.contains(r#"application/json"#);
        let has_webgpu_check = html.contains("navigator.gpu");

        println!("\nValidation:");
        println!("  DOCTYPE:      {has_doctype}");
        println!("  OG tags:      {has_og}");
        println!("  <noscript>:   {has_noscript}");
        println!("  JSON block:   {has_json}");
        println!("  WebGPU check: {has_webgpu_check}");

        Ok::<(), Box<dyn std::error::Error>>(())
    })?;

    println!("\nDone! Check chart.html and chart_custom.html");

    Ok(())
}
