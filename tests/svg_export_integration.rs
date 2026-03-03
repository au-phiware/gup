// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for SVG export.
//!
//! These tests exercise the full SVG export pipeline: constructing a
//! `ComposedChart`, generating axis geometry, and verifying that the
//! resulting SVG document is well-formed and contains the expected
//! elements.

use gup::chart_builder::{ChartConfig, ComposedChart, TitleConfig};
use gup::export::svg::element::rgba_to_css;
use gup::export::svg::{SvgElement, SvgExportOptions};
use gup::prelude::*;
use std::sync::Arc;

/// Minimal data type for test selections.
#[derive(Debug, Clone)]
struct TestData {
    x: f32,
    y: f32,
}

/// Helper: create a headless render context and an empty selection.
async fn test_chart(config: ChartConfig) -> ComposedChart<TestData, gup::Circle> {
    let ctx = Arc::new(gup::RenderContext::new().await.unwrap());
    let sel = gup::selection::Selection::<TestData, gup::Circle>::new(vec![], ctx).unwrap();
    ComposedChart::new(sel, config).with_default_axes()
}

// -----------------------------------------------------------------------
// SVG well-formedness
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_document_is_well_formed() {
    let config = ChartConfig::default()
        .with_title("Test Chart")
        .with_x_scale(LinearScale::new(0.0, 10.0, -1.0, 1.0))
        .with_y_scale(LinearScale::new(0.0, 100.0, -1.0, 1.0));
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    // XML prologue
    assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));

    // Opening and closing SVG tags
    assert!(svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("</svg>"));

    // All opened elements are closed (basic check: count tags)
    let open_g = svg.matches("<g").count();
    let close_g = svg.matches("</g>").count();
    assert_eq!(
        open_g, close_g,
        "Mismatched <g> tags: {open_g} opens, {close_g} closes"
    );
}

// -----------------------------------------------------------------------
// Background rect
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_contains_background_rect() {
    let config = ChartConfig {
        background_color: Some([0.95, 0.95, 0.95, 1.0]),
        ..ChartConfig::default()
    };
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    // There should be at least one <rect> for the background
    assert!(svg.contains("<rect"), "Missing background rect");
    assert!(svg.contains("width=\"800\""));
    assert!(svg.contains("height=\"600\""));
}

// -----------------------------------------------------------------------
// Axis lines and ticks
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_contains_axis_lines() {
    let config = ChartConfig::default()
        .with_x_scale(LinearScale::new(0.0, 10.0, -1.0, 1.0))
        .with_y_scale(LinearScale::new(0.0, 100.0, -1.0, 1.0));
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    // Axis lines are <line> elements inside <g class="axes">
    assert!(svg.contains("class=\"axes\""), "Missing axes group");
    let line_count = svg.matches("<line").count();
    // At minimum we should have axis lines and ticks
    assert!(
        line_count >= 2,
        "Expected at least 2 <line> elements for axes, found {line_count}"
    );
}

#[tokio::test]
async fn svg_contains_tick_marks() {
    let config = ChartConfig::default()
        .with_x_scale(LinearScale::new(0.0, 10.0, -1.0, 1.0))
        .with_y_scale(LinearScale::new(0.0, 100.0, -1.0, 1.0));
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    // Ticks group
    assert!(svg.contains("class=\"ticks\""), "Missing ticks group");
}

// -----------------------------------------------------------------------
// Axis labels
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_contains_axis_labels_as_text() {
    let config = ChartConfig::default()
        .with_x_scale(LinearScale::new(0.0, 10.0, -1.0, 1.0))
        .with_y_scale(LinearScale::new(0.0, 100.0, -1.0, 1.0));
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    // Labels are <text> elements inside <g class="labels">
    assert!(svg.contains("class=\"labels\""), "Missing labels group");
    let text_count = svg.matches("<text").count();
    assert!(
        text_count >= 4,
        "Expected at least 4 <text> elements for axis labels, found {text_count}"
    );

    // Text elements have font attributes
    assert!(svg.contains("font-family="));
    assert!(svg.contains("font-size="));
    assert!(svg.contains("text-anchor="));
    assert!(svg.contains("dominant-baseline="));
}

// -----------------------------------------------------------------------
// Title
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_contains_title() {
    let config = ChartConfig::default().with_title("My Chart Title");
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    assert!(svg.contains("class=\"title\""), "Missing title group");
    assert!(svg.contains("My Chart Title"), "Title text missing");
}

#[tokio::test]
async fn svg_contains_subtitle() {
    let config = ChartConfig::default()
        .with_title_config(TitleConfig::new("Main Title").with_subtitle("Sub Title"));
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    assert!(svg.contains("Main Title"));
    assert!(svg.contains("Sub Title"));
}

// -----------------------------------------------------------------------
// Grid lines
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_contains_grid_lines_when_enabled() {
    let config = ChartConfig::default()
        .with_grid()
        .with_x_scale(LinearScale::new(0.0, 10.0, -1.0, 1.0))
        .with_y_scale(LinearScale::new(0.0, 100.0, -1.0, 1.0));
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    assert!(svg.contains("class=\"grid\""), "Missing grid group");
}

#[tokio::test]
async fn svg_omits_grid_lines_when_disabled() {
    let config = ChartConfig::default().without_grid();
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);
    let svg = chart.render_to_svg(&opts).unwrap();

    assert!(
        !svg.contains("class=\"grid\""),
        "Grid group should not be present"
    );
}

// -----------------------------------------------------------------------
// Data marks
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_includes_provided_data_marks() {
    let config = ChartConfig::default();
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);

    let marks = vec![
        SvgElement::Circle {
            cx: 100.0,
            cy: 200.0,
            r: 5.0,
            fill: rgba_to_css(1.0, 0.0, 0.0, 1.0),
            stroke: None,
            stroke_width: None,
        },
        SvgElement::Circle {
            cx: 300.0,
            cy: 400.0,
            r: 5.0,
            fill: rgba_to_css(0.0, 0.0, 1.0, 1.0),
            stroke: None,
            stroke_width: None,
        },
    ];
    let svg = chart.export_svg_with_marks(&opts, &marks).unwrap();

    assert!(svg.contains("class=\"marks\""), "Missing marks group");
    let circle_count = svg.matches("<circle").count();
    assert_eq!(circle_count, 2, "Expected 2 circles, found {circle_count}");
}

#[tokio::test]
async fn svg_includes_rect_data_marks() {
    let config = ChartConfig::default();
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);

    let marks = vec![
        SvgElement::Rect {
            x: 100.0,
            y: 100.0,
            width: 50.0,
            height: 200.0,
            fill: rgba_to_css(0.2, 0.5, 0.8, 1.0),
            stroke: None,
            stroke_width: None,
            rx: None,
        },
        SvgElement::Rect {
            x: 200.0,
            y: 150.0,
            width: 50.0,
            height: 150.0,
            fill: rgba_to_css(0.8, 0.2, 0.2, 1.0),
            stroke: None,
            stroke_width: None,
            rx: None,
        },
    ];
    let svg = chart.export_svg_with_marks(&opts, &marks).unwrap();

    // Background rect + 2 data rects
    let rect_count = svg.matches("<rect").count();
    assert!(
        rect_count >= 3,
        "Expected at least 3 rects, found {rect_count}"
    );
}

// -----------------------------------------------------------------------
// Export to file
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_export_to_file() {
    let config = ChartConfig::default().with_title("File Export Test");
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600);

    let dir = std::env::temp_dir().join("gup_svg_integration_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_chart.svg");

    chart.export_svg(&path, &opts).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("<svg"));
    assert!(content.contains("File Export Test"));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

// -----------------------------------------------------------------------
// Custom CSS
// -----------------------------------------------------------------------

#[tokio::test]
async fn svg_embeds_custom_css() {
    let config = ChartConfig::default();
    let chart = test_chart(config).await;
    let opts = SvgExportOptions::new(800, 600).with_css("circle:hover { stroke-width: 3; }");
    let svg = chart.render_to_svg(&opts).unwrap();

    assert!(svg.contains("<style>"));
    assert!(svg.contains("circle:hover"));
    assert!(svg.contains("</style>"));
}

// -----------------------------------------------------------------------
// Mark trait svg_element()
// -----------------------------------------------------------------------

#[test]
fn mark_svg_element_circle() {
    let circle = gup::Circle;
    let elem = circle.svg_element();
    assert!(elem.is_some());
    if let Some(SvgElement::Circle { .. }) = elem {
        // correct variant
    } else {
        panic!("Expected SvgElement::Circle");
    }
}

#[test]
fn mark_svg_element_rectangle() {
    let rect = gup::Rectangle;
    let elem = rect.svg_element();
    assert!(elem.is_some());
    if let Some(SvgElement::Rect { .. }) = elem {
        // correct variant
    } else {
        panic!("Expected SvgElement::Rect");
    }
}

#[test]
fn mark_svg_element_line() {
    let line = gup::Line;
    let elem = line.svg_element();
    assert!(elem.is_some());
    if let Some(SvgElement::Line { .. }) = elem {
        // correct variant
    } else {
        panic!("Expected SvgElement::Line");
    }
}

#[test]
fn mark_svg_element_text() {
    use gup::mark::Text;
    let text = Text;
    let elem = text.svg_element();
    assert!(elem.is_some());
    if let Some(SvgElement::Text { .. }) = elem {
        // correct variant
    } else {
        panic!("Expected SvgElement::Text");
    }
}

#[test]
fn mark_svg_element_path() {
    use gup::mark::Path;
    let path = Path;
    let elem = path.svg_element();
    assert!(elem.is_some());
    if let Some(SvgElement::Path { .. }) = elem {
        // correct variant
    } else {
        panic!("Expected SvgElement::Path");
    }
}

#[test]
fn mark_svg_element_boxplot() {
    use gup::mark::BoxPlot;
    let bp = BoxPlot;
    let elem = bp.svg_element();
    assert!(elem.is_some());
    if let Some(SvgElement::Group { class, .. }) = elem {
        assert_eq!(class.as_deref(), Some("boxplot"));
    } else {
        panic!("Expected SvgElement::Group");
    }
}
