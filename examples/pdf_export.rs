// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF Export Example
//!
//! Demonstrates exporting charts to PDF format without requiring a GPU for
//! the export step.  This example shows both:
//!
//! 1. **Single-chart export** via `chart.export_pdf()`
//! 2. **Multi-page export** via `PdfDocument::new()` / `add_page_from_elements()`
//!
//! Run with:
//!
//! ```sh
//! cargo run --features pdf --example pdf_export
//! ```

use gup::chart_builder::{ChartConfig, ComposedChart, Margins, TitleAlignment, TitleConfig};
use gup::export::pdf::{Orientation, PdfDocument, PdfOptions};
use gup::export::svg::SvgElement;
use gup::export::svg::element::rgba_to_css;
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

/// Convert a data point to an SVG circle element in chart pixel coordinates.
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

/// Build a set of bar-chart SVG elements for the multi-page demo.
fn bar_chart_elements(chart_x: f32, chart_y: f32, chart_w: f32, chart_h: f32) -> Vec<SvgElement> {
    let categories = ["Mon", "Tue", "Wed", "Thu", "Fri"];
    let values: [f32; 5] = [42.0, 67.0, 55.0, 80.0, 73.0];
    let max_val: f32 = 100.0;
    let bar_width = chart_w / categories.len() as f32 * 0.7;
    let gap = chart_w / categories.len() as f32 * 0.15;

    let mut elems = Vec::new();
    for (i, (&label, &val)) in categories.iter().zip(values.iter()).enumerate() {
        let bx = chart_x + (i as f32 / categories.len() as f32) * chart_w + gap;
        let bar_h = (val / max_val) * chart_h;
        let by = chart_y + chart_h - bar_h;

        let t = val / max_val;
        elems.push(SvgElement::Rect {
            x: bx,
            y: by,
            width: bar_width,
            height: bar_h,
            fill: rgba_to_css(0.2, 0.4 + 0.4 * t, 0.8, 0.85),
            stroke: Some(rgba_to_css(0.1, 0.2, 0.5, 1.0)),
            stroke_width: Some(1.0),
            rx: Some(2.0),
        });

        // Value label above bar
        elems.push(SvgElement::Text {
            x: bx + bar_width / 2.0,
            y: by - 8.0,
            content: format!("{val:.0}"),
            font_family: "sans-serif".to_string(),
            font_size: 11.0,
            text_anchor: "middle".to_string(),
            dominant_baseline: "alphabetic".to_string(),
            fill: rgba_to_css(0.2, 0.2, 0.2, 1.0),
            font_weight: None,
        });

        // Category label below
        elems.push(SvgElement::Text {
            x: bx + bar_width / 2.0,
            y: chart_y + chart_h + 20.0,
            content: label.to_string(),
            font_family: "sans-serif".to_string(),
            font_size: 12.0,
            text_anchor: "middle".to_string(),
            dominant_baseline: "hanging".to_string(),
            fill: rgba_to_css(0.3, 0.3, 0.3, 1.0),
            font_weight: None,
        });
    }
    elems
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("PDF Export Example");
    println!("==================");

    let data = sample_data();

    // Data range for mapping
    let x_min = 0.0_f32;
    let x_max = 11.0_f32;
    let y_min = 30.0_f32;
    let y_max = 100.0_f32;

    // -----------------------------------------------------------------------
    // 1. Single-chart export using the ComposedChart convenience method
    // -----------------------------------------------------------------------

    let config = ChartConfig {
        title_config: Some(
            TitleConfig::new("Study Hours vs Test Score")
                .with_alignment(TitleAlignment::Center)
                .with_subtitle("Single-chart PDF export"),
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

        let selection = gup::selection::Selection::<DataPoint, gup::Circle>::new(vec![], context)?;

        let chart = ComposedChart::new(selection, config.clone()).with_default_axes();

        // Single-chart PDF export
        chart.export_pdf_with_marks("output_single.pdf", PdfOptions::a4(), &data_marks)?;
        println!("Single-chart PDF written to output_single.pdf");

        Ok::<(), gup::error::GupError>(())
    })?;

    // -----------------------------------------------------------------------
    // 2. Multi-page PDF using PdfDocument builder
    // -----------------------------------------------------------------------

    let mut doc = PdfDocument::new(PdfOptions::a4());

    // Page 1: Scatter plot (portrait A4)
    let mut page1_elements: Vec<SvgElement> = Vec::new();
    // Background
    page1_elements.push(SvgElement::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
        fill: rgba_to_css(1.0, 1.0, 1.0, 1.0),
        stroke: None,
        stroke_width: None,
        rx: None,
    });
    // Title
    page1_elements.push(SvgElement::Text {
        x: 400.0,
        y: 30.0,
        content: "Page 1: Scatter Plot".to_string(),
        font_family: "sans-serif".to_string(),
        font_size: 20.0,
        text_anchor: "middle".to_string(),
        dominant_baseline: "central".to_string(),
        fill: rgba_to_css(0.1, 0.1, 0.1, 1.0),
        font_weight: Some("bold".to_string()),
    });
    page1_elements.extend(data_marks.clone());

    doc.add_page_from_elements("Scatter Plot", &page1_elements, 800.0, 600.0)?;
    println!("Added page 1: Scatter Plot");

    // Page 2: Bar chart
    let mut page2_elements: Vec<SvgElement> = Vec::new();
    page2_elements.push(SvgElement::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
        fill: rgba_to_css(0.98, 0.98, 1.0, 1.0),
        stroke: None,
        stroke_width: None,
        rx: None,
    });
    page2_elements.push(SvgElement::Text {
        x: 400.0,
        y: 30.0,
        content: "Page 2: Weekly Summary".to_string(),
        font_family: "sans-serif".to_string(),
        font_size: 20.0,
        text_anchor: "middle".to_string(),
        dominant_baseline: "central".to_string(),
        fill: rgba_to_css(0.1, 0.1, 0.1, 1.0),
        font_weight: Some("bold".to_string()),
    });
    page2_elements.extend(bar_chart_elements(80.0, 80.0, 640.0, 440.0));

    doc.add_page_from_elements("Weekly Summary", &page2_elements, 800.0, 600.0)?;
    println!("Added page 2: Weekly Summary");

    // Page 3: A simple line chart
    let mut page3_elements: Vec<SvgElement> = Vec::new();
    page3_elements.push(SvgElement::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
        fill: rgba_to_css(1.0, 1.0, 1.0, 1.0),
        stroke: None,
        stroke_width: None,
        rx: None,
    });
    page3_elements.push(SvgElement::Text {
        x: 400.0,
        y: 30.0,
        content: "Page 3: Trend Line".to_string(),
        font_family: "sans-serif".to_string(),
        font_size: 20.0,
        text_anchor: "middle".to_string(),
        dominant_baseline: "central".to_string(),
        fill: rgba_to_css(0.1, 0.1, 0.1, 1.0),
        font_weight: Some("bold".to_string()),
    });
    // Draw a path for the trend line.
    let path_d = data
        .iter()
        .enumerate()
        .map(|(i, pt)| {
            let px = 60.0 + (pt.x - x_min) / (x_max - x_min) * 680.0;
            let py = 60.0 + 480.0 - (pt.y - y_min) / (y_max - y_min) * 480.0;
            if i == 0 {
                format!("M {px} {py}")
            } else {
                format!("L {px} {py}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    page3_elements.push(SvgElement::Path {
        d: path_d,
        fill: "none".to_string(),
        stroke: Some(rgba_to_css(0.2, 0.5, 0.8, 1.0)),
        stroke_width: Some(3.0),
    });

    doc.add_page_from_elements("Trend Line", &page3_elements, 800.0, 600.0)?;
    println!("Added page 3: Trend Line");

    assert_eq!(doc.page_count(), 3);

    doc.write("output_multi.pdf")?;
    println!(
        "\nMulti-page PDF written to output_multi.pdf ({} pages)",
        doc.page_count()
    );

    // -----------------------------------------------------------------------
    // 3. Landscape letter-size example
    // -----------------------------------------------------------------------

    let landscape_opts = PdfOptions::letter().orientation(Orientation::Landscape);
    let mut landscape_doc = PdfDocument::new(landscape_opts);
    landscape_doc.add_page_from_elements("Landscape Chart", &page1_elements, 800.0, 600.0)?;
    landscape_doc.write("output_landscape.pdf")?;
    println!("Landscape PDF written to output_landscape.pdf");

    println!("\nDone! Open the PDFs in a PDF viewer to verify.");
    Ok(())
}
