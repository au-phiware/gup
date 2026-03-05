// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! HTML template generation for the Gup chart exporter.
//!
//! The template produces a single self-contained HTML page that:
//!
//! * Bootstraps a WASM module (inline or via URL) to render the chart
//!   interactively with WebGPU.
//! * Falls back to an embedded SVG when JavaScript is disabled or WebGPU
//!   is unavailable.
//! * Embeds Open Graph and Twitter Card meta tags for rich link previews.
//! * Stores the chart definition as a `<script type="application/json">`
//!   block for later deserialisation.

use std::fmt::Write as _;

/// Render the complete HTML document.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_html(
    page_title: &str,
    description: &str,
    author: &str,
    png_data_uri: &str,
    svg_fallback: &str,
    chart_json: &str,
    wasm_script: &str,
    width: u32,
    height: u32,
) -> String {
    // Pre-allocate a reasonable buffer.
    let mut html =
        String::with_capacity(svg_fallback.len() + chart_json.len() + wasm_script.len() + 4096);

    // Escape text for safe HTML embedding.
    let title_escaped = html_escape(page_title);
    let desc_escaped = html_escape(description);
    let author_escaped = html_escape(author);

    // --- Document start ---------------------------------------------------
    let _ = write!(
        html,
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title_escaped}</title>
"##
    );

    // --- Open Graph / Twitter Card meta tags ------------------------------
    let _ = write!(
        html,
        r#"<meta property="og:title" content="{title_escaped}">
<meta property="og:description" content="{desc_escaped}">
<meta property="og:image" content="{png_data_uri}">
<meta property="og:type" content="website">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:image" content="{png_data_uri}">
"#
    );

    if !author.is_empty() {
        let _ = write!(html, r#"<meta name="author" content="{author_escaped}">"#);
        html.push('\n');
    }

    // --- Inline CSS -------------------------------------------------------
    let _ = write!(
        html,
        r#"<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ display: flex; justify-content: center; align-items: center; min-height: 100vh; background: #f5f5f5; }}
#gup-canvas {{ width: {width}px; height: {height}px; max-width: 100vw; max-height: 100vh; }}
#gup-svg-fallback {{ display: none; }}
.gup-no-webgpu #gup-svg-fallback {{ display: block; }}
.gup-no-webgpu #gup-canvas {{ display: none; }}
noscript svg {{ max-width: 100vw; max-height: 100vh; }}
</style>
"#
    );

    // --- Close head, open body -------------------------------------------
    html.push_str("</head>\n<body>\n");

    // --- Canvas (interactive target) -------------------------------------
    let _ = write!(
        html,
        r#"<canvas id="gup-canvas" width="{width}" height="{height}"></canvas>
"#
    );

    // --- SVG fallback (noscript + JS runtime check) ----------------------
    let _ = write!(
        html,
        r#"<div id="gup-svg-fallback">
{svg_fallback}
</div>
<noscript>
{svg_fallback}
</noscript>
"#
    );

    // --- Chart data as JSON -----------------------------------------------
    let _ = write!(
        html,
        r#"<script type="application/json" id="gup-chart-data">
{chart_json}
</script>
"#
    );

    // --- WASM bootstrap script --------------------------------------------
    let _ = write!(
        html,
        r#"<script>
// Feature-detect WebGPU; show SVG fallback if unavailable.
if (!navigator.gpu) {{
  document.body.classList.add('gup-no-webgpu');
}} else {{
{wasm_script}
}}
</script>
"#
    );

    // --- Close body and document -----------------------------------------
    html.push_str("</body>\n</html>\n");

    html
}

/// Produce the JavaScript snippet that decodes a Base64-inlined WASM
/// module and instantiates it.
///
/// The generated script reads the embedded chart data from
/// `#gup-chart-data` and stores it in `window.__GUP_CHART_DATA__` so that
/// the WASM module's init function can access it.
pub(crate) fn inline_wasm_script(wasm_b64: &str) -> String {
    format!(
        r#"  // Read embedded chart data for the WASM module.
  const chartDataEl = document.getElementById('gup-chart-data');
  if (chartDataEl) {{
    window.__GUP_CHART_DATA__ = chartDataEl.textContent;
  }}

  // Decode the Base64-inlined WASM module.
  const wasmB64 = "{wasm_b64}";
  const wasmBin = Uint8Array.from(atob(wasmB64), c => c.charCodeAt(0));
  WebAssembly.instantiate(wasmBin).then(result => {{
    const {{ instance }} = result;
    if (instance.exports._start) instance.exports._start();
  }}).catch(err => {{
    console.error('Gup WASM initialisation failed:', err);
    document.body.classList.add('gup-no-webgpu');
  }});"#
    )
}

/// Produce the JavaScript snippet that fetches a WASM module from a URL
/// and instantiates it.
///
/// Like [`inline_wasm_script`], the generated script reads the chart data
/// from `#gup-chart-data` and stores it in `window.__GUP_CHART_DATA__`
/// before fetching the WASM module.
pub(crate) fn url_wasm_script(url: &str) -> String {
    let url_escaped = js_string_escape(url);
    format!(
        r#"  // Read embedded chart data for the WASM module.
  const chartDataEl = document.getElementById('gup-chart-data');
  if (chartDataEl) {{
    window.__GUP_CHART_DATA__ = chartDataEl.textContent;
  }}

  fetch("{url_escaped}")
    .then(r => r.arrayBuffer())
    .then(buf => WebAssembly.instantiate(buf))
    .then(result => {{
      const {{ instance }} = result;
      if (instance.exports._start) instance.exports._start();
    }}).catch(err => {{
      console.error('Gup WASM initialisation failed:', err);
      document.body.classList.add('gup-no-webgpu');
    }});"#
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal HTML attribute/content escaping.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Escape a string for embedding inside a JavaScript double-quoted string.
fn js_string_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_html_contains_doctype() {
        let html = render_html(
            "Test",
            "desc",
            "author",
            "data:image/png;base64,AAAA",
            "<svg></svg>",
            r#"{"title":"Test"}"#,
            "// wasm bootstrap",
            800,
            600,
        );
        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn render_html_contains_og_tags() {
        let html = render_html(
            "OG Test",
            "OG description",
            "",
            "data:image/png;base64,XXXX",
            "<svg></svg>",
            "{}",
            "",
            800,
            600,
        );
        assert!(html.contains(r#"<meta property="og:title" content="OG Test">"#));
        assert!(html.contains(r#"<meta property="og:description" content="OG description">"#));
        assert!(
            html.contains(r#"<meta property="og:image" content="data:image/png;base64,XXXX">"#)
        );
        assert!(html.contains(r#"<meta name="twitter:image""#));
    }

    #[test]
    fn render_html_contains_noscript() {
        let svg = "<svg><circle cx=\"10\" cy=\"10\" r=\"5\"/></svg>";
        let html = render_html("T", "", "", "x", svg, "{}", "", 400, 300);
        assert!(html.contains("<noscript>"));
        assert!(html.contains(svg));
    }

    #[test]
    fn render_html_contains_json_block() {
        let json = r#"{"width":800}"#;
        let html = render_html("T", "", "", "x", "<svg/>", json, "", 800, 600);
        assert!(html.contains(r#"<script type="application/json" id="gup-chart-data">"#));
        assert!(html.contains(json));
    }

    #[test]
    fn render_html_contains_webgpu_check() {
        let html = render_html("T", "", "", "x", "<svg/>", "{}", "", 800, 600);
        assert!(html.contains("navigator.gpu"));
        assert!(html.contains("gup-no-webgpu"));
    }

    #[test]
    fn render_html_contains_canvas() {
        let html = render_html("T", "", "", "x", "<svg/>", "{}", "", 1024, 768);
        assert!(html.contains(r#"<canvas id="gup-canvas" width="1024" height="768""#));
    }

    #[test]
    fn inline_wasm_script_contains_base64() {
        let script = inline_wasm_script("AQIDBA==");
        assert!(script.contains("AQIDBA=="));
        assert!(script.contains("atob"));
        assert!(script.contains("WebAssembly.instantiate"));
    }

    #[test]
    fn inline_wasm_script_reads_chart_data() {
        let script = inline_wasm_script("AQIDBA==");
        assert!(
            script.contains("gup-chart-data"),
            "should read the chart data element"
        );
        assert!(
            script.contains("__GUP_CHART_DATA__"),
            "should store chart data in global"
        );
    }

    #[test]
    fn url_wasm_script_contains_fetch() {
        let script = url_wasm_script("https://cdn.example.com/gup.wasm");
        assert!(script.contains("fetch("));
        assert!(script.contains("https://cdn.example.com/gup.wasm"));
        assert!(script.contains("WebAssembly.instantiate"));
    }

    #[test]
    fn url_wasm_script_reads_chart_data() {
        let script = url_wasm_script("https://cdn.example.com/gup.wasm");
        assert!(
            script.contains("gup-chart-data"),
            "should read the chart data element"
        );
        assert!(
            script.contains("__GUP_CHART_DATA__"),
            "should store chart data in global"
        );
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(
            html_escape("<b>\"A&B\"</b>"),
            "&lt;b&gt;&quot;A&amp;B&quot;&lt;/b&gt;"
        );
    }

    #[test]
    fn js_string_escape_quotes() {
        assert_eq!(js_string_escape(r#"say "hello""#), r#"say \"hello\""#);
    }

    #[test]
    fn render_html_author_present() {
        let html = render_html("T", "", "Jane Doe", "x", "<svg/>", "{}", "", 800, 600);
        assert!(html.contains(r#"<meta name="author" content="Jane Doe">"#));
    }

    #[test]
    fn render_html_author_absent() {
        let html = render_html("T", "", "", "x", "<svg/>", "{}", "", 800, 600);
        assert!(!html.contains(r#"<meta name="author""#));
    }
}
