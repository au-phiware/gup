// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the HTML export pipeline.
//!
//! These tests build a full `ComposedChart`, export it to HTML via
//! [`HtmlExporter`], and validate that the resulting document contains
//! all the expected structural elements.

use gup::chart_builder::{ChartConfig, ComposedChart, Margins, TitleAlignment, TitleConfig};
use gup::export::html::{ChartSnapshot, HtmlExporter, WasmStrategy};
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Pt {
    x: f32,
    y: f32,
}

/// Helper: build a chart and export to HTML with the given exporter.
fn export_chart_html(exporter: &HtmlExporter) -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let ctx = Arc::new(gup::RenderContext::new().await.unwrap());
        let sel = gup::selection::Selection::<Pt, gup::Circle>::new(
            vec![Pt { x: 1.0, y: 2.0 }, Pt { x: 3.0, y: 4.0 }],
            ctx,
        )
        .unwrap();

        let config = ChartConfig {
            title_config: Some(
                TitleConfig::new("Integration Test")
                    .with_alignment(TitleAlignment::Center)
                    .with_subtitle("subtitle text"),
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
        };

        let mut chart = ComposedChart::new(sel, config).with_default_axes();
        exporter.render(&mut chart).unwrap()
    })
}

// -----------------------------------------------------------------------
// HTML structure tests
// -----------------------------------------------------------------------

#[test]
fn html_export_is_valid_utf8_html() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    assert!(html.starts_with("<!DOCTYPE html>"), "missing DOCTYPE");
    assert!(html.contains("<html"), "missing <html> tag");
    assert!(html.contains("</html>"), "missing closing </html>");
}

#[test]
fn html_export_contains_og_meta_tags() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()))
        .with_title("OG Test Title")
        .with_description("OG Test Description");
    let html = export_chart_html(&exporter);

    assert!(
        html.contains(r#"<meta property="og:title" content="OG Test Title">"#),
        "missing og:title"
    );
    assert!(
        html.contains(r#"<meta property="og:description" content="OG Test Description">"#),
        "missing og:description"
    );
    assert!(
        html.contains(r#"<meta property="og:image" content="data:image/png;base64,"#),
        "missing og:image with PNG data URI"
    );
    assert!(
        html.contains(r#"<meta name="twitter:image""#),
        "missing twitter:image"
    );
}

#[test]
fn html_export_contains_noscript_svg_fallback() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    assert!(html.contains("<noscript>"), "missing <noscript> block");
    // The SVG fallback should appear inside the noscript block.
    let noscript_idx = html.find("<noscript>").unwrap();
    let after_noscript = &html[noscript_idx..];
    assert!(
        after_noscript.contains("<svg"),
        "noscript block should contain SVG"
    );
}

#[test]
fn html_export_contains_svg_fallback_div() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    // The JS-toggled SVG fallback div.
    assert!(
        html.contains(r#"id="gup-svg-fallback""#),
        "missing gup-svg-fallback div"
    );
}

#[test]
fn html_export_contains_json_data_block() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    assert!(
        html.contains(r#"<script type="application/json" id="gup-chart-data">"#),
        "missing JSON data block"
    );

    // Extract and parse the JSON.
    let start_marker = r#"id="gup-chart-data">"#;
    let start = html.find(start_marker).unwrap() + start_marker.len();
    let end = html[start..].find("</script>").unwrap() + start;
    let json_str = html[start..end].trim();

    let snapshot: ChartSnapshot =
        serde_json::from_str(json_str).expect("embedded JSON should parse as ChartSnapshot");

    assert_eq!(snapshot.title.as_deref(), Some("Integration Test"));
    assert_eq!(snapshot.width, 800.0);
    assert_eq!(snapshot.height, 600.0);
    assert!(snapshot.show_grid);
}

#[test]
fn html_export_contains_webgpu_detection() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    assert!(
        html.contains("navigator.gpu"),
        "missing navigator.gpu check"
    );
    assert!(
        html.contains("gup-no-webgpu"),
        "missing gup-no-webgpu CSS class toggle"
    );
}

#[test]
fn html_export_contains_canvas() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    assert!(
        html.contains(r#"<canvas id="gup-canvas""#),
        "missing canvas element"
    );
    assert!(
        html.contains(r#"width="800""#),
        "canvas should have correct width"
    );
    assert!(
        html.contains(r#"height="600""#),
        "canvas should have correct height"
    );
}

#[test]
fn html_export_url_strategy_contains_fetch() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("https://cdn.example.com/gup.wasm".into()));
    let html = export_chart_html(&exporter);

    assert!(html.contains("fetch("), "URL strategy should use fetch()");
    assert!(
        html.contains("https://cdn.example.com/gup.wasm"),
        "URL should be embedded"
    );
}

#[test]
fn html_export_write_to_file() {
    let exporter =
        HtmlExporter::new(WasmStrategy::Url("gup.wasm".into())).with_title("File Write Test");

    let dir = std::env::temp_dir().join("gup_html_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_chart.html");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let ctx = Arc::new(gup::RenderContext::new().await.unwrap());
        let sel = gup::selection::Selection::<Pt, gup::Circle>::new(vec![], ctx).unwrap();
        let config = ChartConfig::default();
        let mut chart = ComposedChart::new(sel, config).with_default_axes();
        exporter.export(&mut chart, &path).unwrap();
    });

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.starts_with("<!DOCTYPE html>"));
    assert!(contents.contains("og:image"));
    assert!(contents.contains("<noscript>"));
    assert!(contents.contains("application/json"));

    // Clean up.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn html_export_convenience_method() {
    let dir = std::env::temp_dir().join("gup_html_convenience");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("convenience.html");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let ctx = Arc::new(gup::RenderContext::new().await.unwrap());
        let sel = gup::selection::Selection::<Pt, gup::Circle>::new(vec![], ctx).unwrap();
        let config = ChartConfig::default();
        let mut chart = ComposedChart::new(sel, config).with_default_axes();
        chart.export_html(&path).unwrap();
    });

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.starts_with("<!DOCTYPE html>"));

    // Clean up.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn html_export_author_metadata() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into())).with_author("Jane Doe");
    let html = export_chart_html(&exporter);

    assert!(
        html.contains(r#"<meta name="author" content="Jane Doe">"#),
        "author meta tag should be present"
    );
}

// -----------------------------------------------------------------------
// JSON round-trip test
// -----------------------------------------------------------------------

#[test]
fn chart_snapshot_round_trip_from_html() {
    let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
    let html = export_chart_html(&exporter);

    // Extract JSON from the HTML.
    let start_marker = r#"id="gup-chart-data">"#;
    let start = html.find(start_marker).unwrap() + start_marker.len();
    let end = html[start..].find("</script>").unwrap() + start;
    let json_str = html[start..end].trim();

    // Deserialise.
    let snapshot: ChartSnapshot = serde_json::from_str(json_str).unwrap();

    // Re-serialise and deserialise again.
    let json2 = serde_json::to_string_pretty(&snapshot).unwrap();
    let snapshot2: ChartSnapshot = serde_json::from_str(&json2).unwrap();

    assert_eq!(snapshot, snapshot2, "round-trip should be lossless");
}

// -----------------------------------------------------------------------
// Inline WASM strategy test
// -----------------------------------------------------------------------

#[test]
fn html_export_inline_wasm_strategy() {
    // Create a temporary fake WASM file.
    let dir = std::env::temp_dir().join("gup_html_inline_test");
    std::fs::create_dir_all(&dir).unwrap();
    let wasm_path = dir.join("fake.wasm");
    // A minimal valid WASM magic header + version.
    let fake_wasm = b"\x00asm\x01\x00\x00\x00";
    std::fs::write(&wasm_path, fake_wasm).unwrap();

    let exporter = HtmlExporter::new(WasmStrategy::Inline(wasm_path.clone()));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let html = rt.block_on(async {
        let ctx = Arc::new(gup::RenderContext::new().await.unwrap());
        let sel = gup::selection::Selection::<Pt, gup::Circle>::new(vec![], ctx).unwrap();
        let config = ChartConfig::default();
        let mut chart = ComposedChart::new(sel, config).with_default_axes();
        exporter.render(&mut chart).unwrap()
    });

    // The inline strategy should use atob() for Base64 decoding.
    assert!(html.contains("atob("), "inline strategy should use atob()");
    assert!(
        html.contains("WebAssembly.instantiate"),
        "should instantiate WebAssembly"
    );
    // Should NOT contain fetch().
    assert!(
        !html.contains("fetch("),
        "inline strategy should not use fetch()"
    );

    // Clean up.
    let _ = std::fs::remove_dir_all(&dir);
}
