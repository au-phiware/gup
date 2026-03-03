// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the PDF export module.
//!
//! These tests verify end-to-end PDF generation: building SVG elements,
//! converting them through `PdfRenderer` / `PdfDocument`, and asserting
//! that the output starts with the `%PDF-` magic bytes and reports the
//! correct page count.

#![cfg(feature = "pdf")]

use gup::export::pdf::{Orientation, PdfDocument, PdfOptions, PdfRenderer};
use gup::export::svg::SvgElement;
use gup::export::svg::element::rgba_to_css;

// ---------------------------------------------------------------------------
// Helper: build a small chart worth of SVG elements
// ---------------------------------------------------------------------------

fn sample_scatter_elements() -> Vec<SvgElement> {
    let data = [
        (100.0, 200.0),
        (250.0, 150.0),
        (400.0, 350.0),
        (550.0, 100.0),
    ];
    let mut elems = Vec::new();

    // Background
    elems.push(SvgElement::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
        fill: "rgb(255,255,255)".to_string(),
        stroke: None,
        stroke_width: None,
        rx: None,
    });

    // Title
    elems.push(SvgElement::Text {
        x: 400.0,
        y: 30.0,
        content: "Test Scatter Plot".to_string(),
        font_family: "sans-serif".to_string(),
        font_size: 18.0,
        text_anchor: "middle".to_string(),
        dominant_baseline: "central".to_string(),
        fill: "rgb(0,0,0)".to_string(),
        font_weight: Some("bold".to_string()),
    });

    // Data circles
    for (cx, cy) in data {
        elems.push(SvgElement::Circle {
            cx,
            cy,
            r: 8.0,
            fill: rgba_to_css(0.2, 0.5, 0.9, 0.8),
            stroke: Some("rgb(0,0,0)".to_string()),
            stroke_width: Some(1.0),
        });
    }

    // Axis lines
    elems.push(SvgElement::Line {
        x1: 60.0,
        y1: 550.0,
        x2: 740.0,
        y2: 550.0,
        stroke: "rgb(0,0,0)".to_string(),
        stroke_width: 1.5,
        stroke_dasharray: None,
    });
    elems.push(SvgElement::Line {
        x1: 60.0,
        y1: 50.0,
        x2: 60.0,
        y2: 550.0,
        stroke: "rgb(0,0,0)".to_string(),
        stroke_width: 1.5,
        stroke_dasharray: None,
    });

    elems
}

fn sample_bar_elements() -> Vec<SvgElement> {
    let mut elems = Vec::new();
    elems.push(SvgElement::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
        fill: "rgb(250,250,255)".to_string(),
        stroke: None,
        stroke_width: None,
        rx: None,
    });
    elems.push(SvgElement::Text {
        x: 400.0,
        y: 30.0,
        content: "Test Bar Chart".to_string(),
        font_family: "sans-serif".to_string(),
        font_size: 18.0,
        text_anchor: "middle".to_string(),
        dominant_baseline: "central".to_string(),
        fill: "rgb(0,0,0)".to_string(),
        font_weight: Some("bold".to_string()),
    });
    let values = [40.0, 70.0, 55.0, 85.0];
    for (i, &val) in values.iter().enumerate() {
        let bx = 100.0 + i as f32 * 150.0;
        let bar_h = val / 100.0 * 400.0;
        elems.push(SvgElement::Rect {
            x: bx,
            y: 500.0 - bar_h,
            width: 100.0,
            height: bar_h,
            fill: rgba_to_css(0.3, 0.6, 0.9, 0.85),
            stroke: Some("rgb(20,40,80)".to_string()),
            stroke_width: Some(1.0),
            rx: Some(3.0),
        });
    }
    elems
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_single_page_pdf_has_magic_bytes() {
    let elems = sample_scatter_elements();
    let renderer = PdfRenderer::new(PdfOptions::a4());
    let bytes = renderer.render_to_bytes(&elems, 800.0, 600.0).unwrap();
    assert!(
        bytes.starts_with(b"%PDF-"),
        "PDF should start with %PDF- magic bytes"
    );
}

#[test]
fn test_single_page_pdf_is_non_trivial() {
    let elems = sample_scatter_elements();
    let renderer = PdfRenderer::new(PdfOptions::a4());
    let bytes = renderer.render_to_bytes(&elems, 800.0, 600.0).unwrap();
    // A PDF with actual content should be more than a few hundred bytes.
    assert!(
        bytes.len() > 500,
        "PDF is suspiciously small: {} bytes",
        bytes.len()
    );
}

#[test]
fn test_multi_page_pdf_page_count() {
    let scatter = sample_scatter_elements();
    let bars = sample_bar_elements();

    let mut doc = PdfDocument::new(PdfOptions::a4());
    doc.add_page_from_elements("Scatter", &scatter, 800.0, 600.0)
        .unwrap();
    doc.add_page_from_elements("Bars", &bars, 800.0, 600.0)
        .unwrap();

    assert_eq!(doc.page_count(), 2);

    let bytes = doc.to_bytes().unwrap();
    assert!(bytes.starts_with(b"%PDF-"));

    // The multi-page PDF should be larger than a single-page one.
    let single_renderer = PdfRenderer::new(PdfOptions::a4());
    let single_bytes = single_renderer
        .render_to_bytes(&scatter, 800.0, 600.0)
        .unwrap();
    assert!(
        bytes.len() > single_bytes.len(),
        "Multi-page ({} bytes) should be larger than single-page ({} bytes)",
        bytes.len(),
        single_bytes.len()
    );
}

#[test]
fn test_pdf_document_write_and_read_back() {
    let dir = std::env::temp_dir().join("gup_pdf_integration");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("integration_output.pdf");

    let elems = sample_scatter_elements();
    let mut doc = PdfDocument::new(PdfOptions::letter());
    doc.add_page_from_elements("Test", &elems, 800.0, 600.0)
        .unwrap();
    doc.write(&path).unwrap();

    let content = std::fs::read(&path).unwrap();
    assert!(content.starts_with(b"%PDF-"));
    assert!(content.len() > 500);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn test_pdf_write_error_on_bad_path() {
    let elems = sample_scatter_elements();
    let mut doc = PdfDocument::new(PdfOptions::a4());
    doc.add_page_from_elements("Test", &elems, 800.0, 600.0)
        .unwrap();

    let result = doc.write("/nonexistent/dir/file.pdf");
    assert!(result.is_err());
}

#[test]
fn test_landscape_produces_wider_page() {
    let elems = sample_scatter_elements();

    let portrait = PdfRenderer::new(PdfOptions::a4());
    let landscape = PdfRenderer::new(PdfOptions::a4().orientation(Orientation::Landscape));

    let p_page = portrait.render_page(&elems, 800.0, 600.0);
    let l_page = landscape.render_page(&elems, 800.0, 600.0);

    // Landscape page should be wider than tall; portrait should be taller.
    assert!(
        p_page.media_box.height > p_page.media_box.width,
        "Portrait page should be taller than wide"
    );
    assert!(
        l_page.media_box.width > l_page.media_box.height,
        "Landscape page should be wider than tall"
    );
}

#[test]
fn test_custom_page_size() {
    let opts = PdfOptions::custom(150.0, 150.0).margin_mm(5.0);
    let renderer = PdfRenderer::new(opts);
    let elems = vec![SvgElement::Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        fill: "white".to_string(),
        stroke: None,
        stroke_width: None,
        rx: None,
    }];
    let page = renderer.render_page(&elems, 100.0, 100.0);
    // 150mm ≈ 425.2pt
    assert!(
        (page.media_box.width.0 - 150.0 * 2.834_646).abs() < 1.0,
        "width = {} pt",
        page.media_box.width.0
    );
}

#[test]
fn test_pdf_with_groups() {
    let elems = vec![SvgElement::Group {
        class: Some("data".to_string()),
        transform: None,
        children: vec![
            SvgElement::Circle {
                cx: 100.0,
                cy: 100.0,
                r: 10.0,
                fill: "red".to_string(),
                stroke: None,
                stroke_width: None,
            },
            SvgElement::Circle {
                cx: 200.0,
                cy: 200.0,
                r: 10.0,
                fill: "blue".to_string(),
                stroke: None,
                stroke_width: None,
            },
        ],
    }];

    let renderer = PdfRenderer::new(PdfOptions::a4());
    let bytes = renderer.render_to_bytes(&elems, 400.0, 400.0).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn test_pdf_with_path_elements() {
    let elems = vec![SvgElement::Path {
        d: "M 10 10 L 100 10 L 100 100 Z".to_string(),
        fill: "rgba(0,128,255,0.50)".to_string(),
        stroke: Some("rgb(0,0,0)".to_string()),
        stroke_width: Some(2.0),
    }];

    let renderer = PdfRenderer::new(PdfOptions::a4());
    let bytes = renderer.render_to_bytes(&elems, 200.0, 200.0).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn test_pdf_with_dashed_lines() {
    let elems = vec![SvgElement::Line {
        x1: 0.0,
        y1: 50.0,
        x2: 200.0,
        y2: 50.0,
        stroke: "rgb(128,128,128)".to_string(),
        stroke_width: 1.0,
        stroke_dasharray: Some("4 2".to_string()),
    }];

    let renderer = PdfRenderer::new(PdfOptions::a4());
    let bytes = renderer.render_to_bytes(&elems, 200.0, 100.0).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn test_pdf_write_to_writer() {
    let elems = sample_scatter_elements();
    let mut doc = PdfDocument::new(PdfOptions::a4());
    doc.add_page_from_elements("Test", &elems, 800.0, 600.0)
        .unwrap();

    let mut buffer = Vec::new();
    doc.write_to_writer(&mut buffer).unwrap();
    assert!(buffer.starts_with(b"%PDF-"));
    assert!(buffer.len() > 500);
}

#[test]
fn test_empty_document_produces_valid_pdf() {
    // A document with no pages should still produce valid PDF bytes.
    let doc = PdfDocument::new(PdfOptions::a4());
    let bytes = doc.to_bytes().unwrap();
    assert!(
        bytes.starts_with(b"%PDF-"),
        "Empty document should produce valid PDF"
    );
}

#[test]
fn test_pdf_options_presets() {
    // Verify all preset constructors work.
    let a4 = PdfOptions::a4();
    assert!((a4.effective_width_mm() - 210.0).abs() < 0.1);
    assert!((a4.effective_height_mm() - 297.0).abs() < 0.1);

    let letter = PdfOptions::letter();
    assert!((letter.effective_width_mm() - 215.9).abs() < 0.1);
    assert!((letter.effective_height_mm() - 279.4).abs() < 0.1);

    let custom = PdfOptions::custom(100.0, 200.0);
    assert!((custom.effective_width_mm() - 100.0).abs() < 0.1);
    assert!((custom.effective_height_mm() - 200.0).abs() < 0.1);

    let landscape = PdfOptions::a4().orientation(Orientation::Landscape);
    assert!((landscape.effective_width_mm() - 297.0).abs() < 0.1);
    assert!((landscape.effective_height_mm() - 210.0).abs() < 0.1);
}

#[test]
fn test_feature_flag_no_leakage() {
    // This test exists to document that `cargo check` without `--features pdf`
    // must compile cleanly.  The actual verification is done by CI running
    // `cargo check` without the feature.  Here we just verify our types exist
    // when the feature is enabled.
    let _opts = PdfOptions::a4();
    let _renderer = PdfRenderer::new(PdfOptions::a4());
    let _doc = PdfDocument::new(PdfOptions::a4());
}
