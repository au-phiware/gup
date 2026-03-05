// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! PNG Export Example
//!
//! Demonstrates exporting a Gup chart to a PNG file using GPU off-screen
//! rendering.  The example creates a `ComposedChart` with axes, a title,
//! and grid lines, renders it to an off-screen texture at the requested
//! resolution, reads the pixels back via a staging buffer, and writes a
//! valid PNG file to the working directory.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example export_png
//! ```

use gup::chart_builder::{ChartConfig, ComposedChart, Margins, TitleAlignment, TitleConfig};
use gup::mark::circle::CircleInstance;
use gup::prelude::*;
use std::sync::Arc;

/// A data point for the scatter plot.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DataPoint {
    x: f32,
    y: f32,
}

/// Generate sample data representing the relationship between study hours
/// and test scores.
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
    println!("PNG Export Example");
    println!("==================");

    let data = sample_data();

    // Data range for axis scales
    let x_min = 0.0_f32;
    let x_max = 11.0_f32;
    let y_min = 30.0_f32;
    let y_max = 100.0_f32;

    // Chart configuration
    let config = ChartConfig {
        title_config: Some(
            TitleConfig::new("Study Hours vs Test Score")
                .with_alignment(TitleAlignment::Center)
                .with_subtitle("Exported to PNG via GPU off-screen rendering"),
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

    // Create a headless GPU context and build the chart.
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = Arc::new(gup::RenderContext::new().await?);

        let mut selection = gup::selection::Selection::<DataPoint, gup::Circle>::new(
            data.clone(),
            context.clone(),
        )?;

        // Prepare GPU buffers so data marks appear in the exported PNG.
        // Map data coordinates to clip space using the same linear scales
        // configured on the chart axes.
        selection.prepare_render(
            context.device(),
            context.queue(),
            |d: &DataPoint| {
                let cx = (d.x - x_min) / (x_max - x_min) * 2.0 - 1.0;
                let cy = (d.y - y_min) / (y_max - y_min) * 2.0 - 1.0;
                CircleInstance {
                    center: [cx, cy],
                    radius: 0.02,
                    _pad0: 0.0,
                    fill_color: [0.22, 0.46, 0.82, 1.0], // Steel blue
                    stroke_width: 0.005,
                    _pad1: [0.0; 3],
                    stroke_color: [0.1, 0.1, 0.1, 1.0],
                }
            },
            None,
            None,
        )?;

        let mut chart = ComposedChart::new(selection, config.clone()).with_default_axes();

        // Gallery screenshot support
        if let Some(req) = gup::export::gallery::screenshot_request() {
            chart.export_png(&req.path, req.width, req.height)?;
            return Ok(());
        }

        // --- 1× export (800×600) ---
        let output_1x = "chart.png";
        chart.export_png(output_1x, 800, 600)?;
        let meta = std::fs::metadata(output_1x)?;
        println!("Wrote {output_1x} ({} bytes)", meta.len());

        // --- 2× HiDPI export (1600×1200 pixels at logical 800×600) ---
        let output_2x = "chart@2x.png";
        let png_2x = chart.render_to_png_scaled(800, 600, 2.0)?;
        std::fs::write(output_2x, &png_2x)?;
        println!("Wrote {output_2x} ({} bytes)", png_2x.len());

        // --- Large export (2400×1600) ---
        let output_large = "chart_large.png";
        chart.export_png(output_large, 2400, 1600)?;
        let meta_large = std::fs::metadata(output_large)?;
        println!("Wrote {output_large} ({} bytes)", meta_large.len());

        // Verify the output by decoding
        let png_bytes = std::fs::read(output_1x)?;
        println!(
            "\nPNG magic bytes: {:?}",
            &png_bytes[..8.min(png_bytes.len())]
        );

        Ok::<(), Box<dyn std::error::Error>>(())
    })?;

    println!("\nDone! Check chart.png, chart@2x.png, and chart_large.png");

    Ok(())
}
