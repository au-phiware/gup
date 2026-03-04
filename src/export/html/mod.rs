// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! HTML export for Gup charts.
//!
//! Produces a self-contained `.html` file that embeds:
//!
//! * A **WASM bundle** (Base64-inlined or referenced by URL) that renders the
//!   chart interactively in any WebGPU-capable browser.
//! * A **chart definition** serialised as JSON inside a
//!   `<script type="application/json">` block.
//! * An **SVG fallback** displayed via `<noscript>` and a JavaScript
//!   `navigator.gpu` check when WebGPU is unavailable.
//! * **Open Graph `<meta>` tags** with a PNG thumbnail so the page previews
//!   correctly when shared on social media.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use gup::export::html::{HtmlExporter, WasmStrategy};
//!
//! # use gup::prelude::*;
//! # use gup::chart_builder::ComposedChart;
//! # use std::sync::Arc;
//! # async fn example() -> GupResult<()> {
//! # let ctx = Arc::new(gup::RenderContext::new().await?);
//! # #[derive(Debug, Clone)] struct D { x: f32, y: f32 }
//! # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], ctx)?;
//! # let config = gup::chart_builder::ChartConfig::default();
//! # let mut chart = ComposedChart::new(sel, config).with_default_axes();
//! let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()))
//!     .with_title("My Chart")
//!     .with_description("A scatter plot of study hours vs test scores");
//! exporter.export(&mut chart, "chart.html")?;
//! # Ok(())
//! # }
//! ```

mod snapshot;
mod template;

pub use snapshot::{ChartSnapshot, SnapshotMargins};

use crate::error::{GupError, GupResult};
use std::path::{Path, PathBuf};

/// Strategy for embedding the WebAssembly module in the exported HTML.
///
/// # Variants
///
/// * [`Inline`](WasmStrategy::Inline) — Reads the `.wasm` file from disk,
///   Base64-encodes it, and inlines the result into the HTML.  Produces a
///   completely self-contained file at the cost of a ~33 % size increase
///   over the raw binary.
///
/// * [`Url`](WasmStrategy::Url) — Emits a `fetch(url)` call that loads the
///   WASM module from the given URL at runtime.  Produces a much smaller
///   HTML file but requires the `.wasm` to be hosted somewhere accessible.
#[derive(Debug, Clone)]
pub enum WasmStrategy {
    /// Base64-encode the WASM binary at the given path into the HTML.
    Inline(PathBuf),
    /// Reference the WASM module at this URL.
    Url(String),
}

/// Builder for producing a self-contained HTML file from a Gup chart.
///
/// Create one via [`HtmlExporter::new`], customise with the `with_*`
/// methods, then call [`export`](Self::export) or
/// [`render`](Self::render) to produce the output.
///
/// # Example
///
/// ```rust,no_run
/// use gup::export::html::{HtmlExporter, WasmStrategy};
/// # use gup::prelude::*;
/// # use gup::chart_builder::ComposedChart;
/// # use std::sync::Arc;
/// # async fn example() -> GupResult<()> {
/// # let ctx = Arc::new(gup::RenderContext::new().await?);
/// # #[derive(Debug, Clone)] struct D { x: f32, y: f32 }
/// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], ctx)?;
/// # let config = gup::chart_builder::ChartConfig::default();
/// # let mut chart = ComposedChart::new(sel, config);
/// let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()))
///     .with_title("Revenue Dashboard");
/// exporter.export(&mut chart, "dashboard.html")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct HtmlExporter {
    wasm_strategy: WasmStrategy,
    page_title: Option<String>,
    description: Option<String>,
    author: Option<String>,
}

impl HtmlExporter {
    /// Create a new exporter with the given WASM embedding strategy.
    pub fn new(wasm_strategy: WasmStrategy) -> Self {
        Self {
            wasm_strategy,
            page_title: None,
            description: None,
            author: None,
        }
    }

    /// Set the HTML page `<title>` and `og:title` meta tag.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.page_title = Some(title.into());
        self
    }

    /// Set the `og:description` meta tag content.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the `<meta name="author">` tag content.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Render the chart as a complete HTML document string.
    ///
    /// This is the core method that produces the HTML.  It:
    ///
    /// 1. Generates an SVG fallback via the chart's SVG renderer.
    /// 2. Renders a PNG thumbnail for Open Graph meta tags.
    /// 3. Serialises the chart configuration as a [`ChartSnapshot`] JSON
    ///    block.
    /// 4. Assembles the final HTML from the template.
    ///
    /// # Errors
    ///
    /// Returns an error if SVG rendering, PNG rendering, WASM file
    /// reading, or JSON serialisation fails.
    pub fn render<T, M>(
        &self,
        chart: &mut crate::chart_builder::ComposedChart<T, M>,
    ) -> GupResult<String>
    where
        T: Clone + crate::MaybeSend + crate::MaybeSync + std::fmt::Debug + 'static,
        M: crate::selection::Mark,
    {
        // 1. Build SVG fallback.
        let svg_options = crate::export::svg::SvgExportOptions::new(
            chart.config.width as u32,
            chart.config.height as u32,
        );
        let svg_fallback = chart.render_to_svg(&svg_options)?;

        // 2. Render PNG thumbnail for OG tags.
        let png_bytes =
            chart.render_to_png(chart.config.width as u32, chart.config.height as u32)?;

        // 3. Serialise chart config as JSON.
        let snapshot = ChartSnapshot::from_config(&chart.config);
        let chart_json =
            serde_json::to_string_pretty(&snapshot).map_err(|e| GupError::InvalidDataFormat {
                message: format!("Failed to serialise chart snapshot: {e}"),
            })?;

        // 4. Resolve WASM strategy.
        let wasm_script = self.wasm_bootstrap_script()?;

        // 5. Derive page title from config or explicit override.
        let page_title = self
            .page_title
            .clone()
            .or_else(|| chart.config.title_config.as_ref().map(|t| t.text.clone()))
            .unwrap_or_else(|| "Gup Chart".to_string());

        let description = self
            .description
            .clone()
            .or_else(|| {
                chart
                    .config
                    .title_config
                    .as_ref()
                    .and_then(|t| t.subtitle.clone())
            })
            .unwrap_or_default();

        // 6. Base64-encode the PNG thumbnail.
        use base64::Engine as _;
        let png_b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        let png_data_uri = format!("data:image/png;base64,{png_b64}");

        // 7. Assemble HTML.
        let html = template::render_html(
            &page_title,
            &description,
            self.author.as_deref().unwrap_or_default(),
            &png_data_uri,
            &svg_fallback,
            &chart_json,
            &wasm_script,
            chart.config.width as u32,
            chart.config.height as u32,
        );

        Ok(html)
    }

    /// Render the chart to HTML and write the result to a file.
    ///
    /// Convenience wrapper around [`render`](Self::render).
    ///
    /// # Errors
    ///
    /// Returns a [`GupError::FileError`] if the file cannot be written,
    /// or propagates any error from [`render`](Self::render).
    pub fn export<T, M>(
        &self,
        chart: &mut crate::chart_builder::ComposedChart<T, M>,
        path: impl AsRef<Path>,
    ) -> GupResult<()>
    where
        T: Clone + crate::MaybeSend + crate::MaybeSync + std::fmt::Debug + 'static,
        M: crate::selection::Mark,
    {
        let html = self.render(chart)?;
        let path = path.as_ref();
        std::fs::write(path, html.as_bytes()).map_err(|e| GupError::FileError {
            path: path.display().to_string(),
            error: e.to_string(),
        })
    }

    /// Produce the `<script>` body that bootstraps the WASM module.
    fn wasm_bootstrap_script(&self) -> GupResult<String> {
        match &self.wasm_strategy {
            WasmStrategy::Inline(path) => {
                let wasm_bytes = std::fs::read(path).map_err(|e| GupError::FileError {
                    path: path.display().to_string(),
                    error: e.to_string(),
                })?;

                use base64::Engine as _;
                let wasm_b64 = base64::engine::general_purpose::STANDARD.encode(&wasm_bytes);

                Ok(template::inline_wasm_script(&wasm_b64))
            }
            WasmStrategy::Url(url) => Ok(template::url_wasm_script(url)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_strategy_url_debug() {
        let strategy = WasmStrategy::Url("https://cdn.example.com/gup.wasm".into());
        let dbg = format!("{strategy:?}");
        assert!(dbg.contains("Url"));
    }

    #[test]
    fn wasm_strategy_inline_debug() {
        let strategy = WasmStrategy::Inline(PathBuf::from("/tmp/gup.wasm"));
        let dbg = format!("{strategy:?}");
        assert!(dbg.contains("Inline"));
    }

    #[test]
    fn html_exporter_builder() {
        let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()))
            .with_title("Test Chart")
            .with_description("A test chart")
            .with_author("Test Author");

        assert_eq!(exporter.page_title.as_deref(), Some("Test Chart"));
        assert_eq!(exporter.description.as_deref(), Some("A test chart"));
        assert_eq!(exporter.author.as_deref(), Some("Test Author"));
    }

    #[test]
    fn base64_encode_known_bytes() {
        use base64::Engine as _;
        let bytes = b"Hello, world!";
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        assert_eq!(encoded, "SGVsbG8sIHdvcmxkIQ==");

        // Verify data URI prefix for PNG.
        let data_uri = format!("data:image/png;base64,{encoded}");
        assert!(data_uri.starts_with("data:image/png;base64,"));
    }
}
