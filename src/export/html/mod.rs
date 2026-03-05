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
//! # WASM strategies
//!
//! The [`WasmStrategy`] enum controls how the WASM module is embedded:
//!
//! | Strategy                              | Description                         |
//! |---------------------------------------|-------------------------------------|
//! | [`Inline(path)`](WasmStrategy::Inline) | Base64-encode a `.wasm` file.      |
//! | [`Url(url)`](WasmStrategy::Url)        | Fetch from a URL at runtime.       |
//! | [`Auto(root)`](WasmStrategy::Auto)     | Auto-discover from `pkg/` output.  |
//!
//! ## Auto-discovery
//!
//! [`WasmStrategy::Auto`] searches for the `wasm-pack` output artifact
//! (`*_bg.wasm`) inside the `pkg/` subdirectory of the workspace root
//! (or the current directory when `None` is passed).  This is the
//! recommended strategy for projects that use `wasm-pack build` as part
//! of their workflow.
//!
//! If the `pkg/` directory is missing, empty, or contains multiple
//! `*_bg.wasm` files (ambiguous), a clear error message is returned
//! with guidance on falling back to [`WasmStrategy::Inline`].
//!
//! You can also call [`discover_wasm_artifact`] directly to locate the
//! WASM file without building the full exporter.
//!
//! # JavaScript↔WASM data passing
//!
//! The generated JavaScript bootstrap reads the embedded chart data from
//! the `<script id="gup-chart-data">` element and stores it as
//! `window.__GUP_CHART_DATA__` before instantiating the WASM module.
//! The Gup WASM entry point (see [`wasm_api::render_from_bundle`](crate::wasm_api::render_from_bundle))
//! can then parse this JSON as a [`ChartBundle`] or [`ChartSnapshot`]
//! and render the chart onto the `<canvas>`.
//!
//! # Data embedding
//!
//! When the chart's data type `T` implements [`serde::Serialize`], the
//! exporter can embed the full dataset in the JSON block alongside the
//! configuration.  This uses the [`ChartBundle`] format, which wraps a
//! [`ChartSnapshot`] with an optional `data` array:
//!
//! ```json
//! {
//!   "config": { "title": "…", "width": 800, … },
//!   "data": [ { "x": 1.0, "y": 2.0 }, … ]
//! }
//! ```
//!
//! Use [`HtmlExporter::render_with_data`] / [`HtmlExporter::export_with_data`]
//! (or the convenience [`ComposedChart::export_html_with_data`](crate::chart_builder::ComposedChart::export_html_with_data)) to enable data
//! embedding.  The data-free methods ([`HtmlExporter::render`] /
//! [`HtmlExporter::export`]) continue to work as before and produce a plain
//! [`ChartSnapshot`] JSON block.
//!
//! ## Size considerations
//!
//! Embedding data increases the HTML file size proportionally to the
//! dataset.  Each data point is serialised as a JSON object, so a 10 000-row
//! dataset with a handful of fields might add several hundred kilobytes.
//! For very large datasets, consider:
//!
//! * Downsampling or aggregating before export.
//! * Using the config-only export and loading data separately at runtime.
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
//! // Auto-discover WASM from wasm-pack output:
//! let exporter = HtmlExporter::new(WasmStrategy::Auto(None))
//!     .with_title("My Chart")
//!     .with_description("A scatter plot of study hours vs test scores");
//! exporter.export(&mut chart, "chart.html")?;
//!
//! // Or use an explicit URL:
//! let exporter = HtmlExporter::new(WasmStrategy::Url("gup.wasm".into()));
//! exporter.export(&mut chart, "chart.html")?;
//! # Ok(())
//! # }
//! ```

mod snapshot;
mod template;

pub use snapshot::{ChartBundle, ChartSnapshot, SnapshotMargins};

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
///
/// * [`Auto`](WasmStrategy::Auto) — Automatically discovers the `.wasm`
///   artifact from the `wasm-pack` output directory (`pkg/`), then inlines
///   it just like [`Inline`](WasmStrategy::Inline).  Searches the current
///   directory by default, or a custom workspace root if specified.
///
///   The discovery algorithm looks for files matching `*_bg.wasm` inside a
///   `pkg/` subdirectory, which is the standard layout produced by
///   `wasm-pack build`.  If no file is found, an error is returned with
///   guidance on falling back to an explicit path.
#[derive(Debug, Clone)]
pub enum WasmStrategy {
    /// Base64-encode the WASM binary at the given path into the HTML.
    Inline(PathBuf),
    /// Reference the WASM module at this URL.
    Url(String),
    /// Auto-discover the WASM artifact from the `wasm-pack` output.
    ///
    /// Optionally specify a workspace root directory.  When `None`, the
    /// current working directory is used.
    Auto(Option<PathBuf>),
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

    /// Render the chart as a complete HTML document with data embedded.
    ///
    /// Like [`render`](Self::render), but also serialises the chart's
    /// [`Selection`](crate::selection::Selection) data items into the JSON
    /// block as a [`ChartBundle`].  The resulting HTML is fully
    /// self-contained — the WASM module can reconstruct the entire chart
    /// (configuration **and** data) from the embedded JSON alone.
    ///
    /// The JSON block uses the [`ChartBundle`] format:
    ///
    /// ```json
    /// {
    ///   "config": { /* ChartSnapshot fields */ },
    ///   "data": [ /* serialised T instances */ ]
    /// }
    /// ```
    ///
    /// # Type bounds
    ///
    /// This method requires `T: Serialize`.  If your data type does not
    /// implement `Serialize`, use [`render`](Self::render) instead — it
    /// embeds only the config snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if SVG rendering, PNG rendering, WASM file
    /// reading, or JSON serialisation fails.
    pub fn render_with_data<T, M>(
        &self,
        chart: &mut crate::chart_builder::ComposedChart<T, M>,
    ) -> GupResult<String>
    where
        T: Clone
            + crate::MaybeSend
            + crate::MaybeSync
            + std::fmt::Debug
            + serde::Serialize
            + 'static,
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

        // 3. Serialise chart config + data as a ChartBundle.
        let snapshot = ChartSnapshot::from_config(&chart.config);
        let data_values: Vec<serde_json::Value> = chart
            .visualization
            .data()
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| GupError::InvalidDataFormat {
                message: format!("Failed to serialise data item: {e}"),
            })?;

        let bundle = ChartBundle::with_data(snapshot, data_values);
        let chart_json =
            serde_json::to_string_pretty(&bundle).map_err(|e| GupError::InvalidDataFormat {
                message: format!("Failed to serialise chart bundle: {e}"),
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

    /// Render the chart to HTML with data and write the result to a file.
    ///
    /// Convenience wrapper around [`render_with_data`](Self::render_with_data).
    ///
    /// # Errors
    ///
    /// Returns a [`GupError::FileError`] if the file cannot be written,
    /// or propagates any error from [`render_with_data`](Self::render_with_data).
    pub fn export_with_data<T, M>(
        &self,
        chart: &mut crate::chart_builder::ComposedChart<T, M>,
        path: impl AsRef<Path>,
    ) -> GupResult<()>
    where
        T: Clone
            + crate::MaybeSend
            + crate::MaybeSync
            + std::fmt::Debug
            + serde::Serialize
            + 'static,
        M: crate::selection::Mark,
    {
        let html = self.render_with_data(chart)?;
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
            WasmStrategy::Auto(root) => {
                let wasm_path = discover_wasm_artifact(root.as_deref())?;
                let wasm_bytes = std::fs::read(&wasm_path).map_err(|e| GupError::FileError {
                    path: wasm_path.display().to_string(),
                    error: e.to_string(),
                })?;

                use base64::Engine as _;
                let wasm_b64 = base64::engine::general_purpose::STANDARD.encode(&wasm_bytes);

                Ok(template::inline_wasm_script(&wasm_b64))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-discovery
// ---------------------------------------------------------------------------

/// Search for a `wasm-pack` output artifact.
///
/// Looks for `*_bg.wasm` inside the `pkg/` subdirectory of `root`.  When
/// `root` is `None`, the current working directory is used.
///
/// # Errors
///
/// Returns [`GupError::FileError`] if:
///
/// * The `pkg/` directory does not exist.
/// * No `*_bg.wasm` file is found inside it.
/// * Multiple `*_bg.wasm` files are found (ambiguous).
pub fn discover_wasm_artifact(root: Option<&Path>) -> GupResult<PathBuf> {
    let base = match root {
        Some(r) => r.to_path_buf(),
        None => std::env::current_dir().map_err(|e| GupError::FileError {
            path: ".".into(),
            error: format!("Failed to determine current directory: {e}"),
        })?,
    };

    let pkg_dir = base.join("pkg");
    if !pkg_dir.is_dir() {
        return Err(GupError::FileError {
            path: pkg_dir.display().to_string(),
            error: "pkg/ directory not found. Run `wasm-pack build` first, \
                    or use WasmStrategy::Inline(path) to specify the WASM file explicitly."
                .into(),
        });
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(&pkg_dir).map_err(|e| GupError::FileError {
        path: pkg_dir.display().to_string(),
        error: format!("Failed to read pkg/ directory: {e}"),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| GupError::FileError {
            path: pkg_dir.display().to_string(),
            error: format!("Failed to read directory entry: {e}"),
        })?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with("_bg.wasm") {
            candidates.push(entry.path());
        }
    }

    match candidates.len() {
        0 => Err(GupError::FileError {
            path: pkg_dir.display().to_string(),
            error: "No *_bg.wasm file found in pkg/. Run `wasm-pack build` first, \
                    or use WasmStrategy::Inline(path) to specify the WASM file explicitly."
                .into(),
        }),
        1 => Ok(candidates.into_iter().next().unwrap()),
        n => {
            let names: Vec<String> = candidates
                .iter()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            Err(GupError::FileError {
                path: pkg_dir.display().to_string(),
                error: format!(
                    "Found {n} *_bg.wasm files ({}) — ambiguous. \
                     Use WasmStrategy::Inline(path) to specify which one.",
                    names.join(", ")
                ),
            })
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
    fn wasm_strategy_auto_debug() {
        let strategy = WasmStrategy::Auto(None);
        let dbg = format!("{strategy:?}");
        assert!(dbg.contains("Auto"));
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

    // -- Auto-discovery tests -----------------------------------------------

    #[test]
    fn discover_wasm_artifact_finds_single_file() {
        let dir = std::env::temp_dir().join("gup_wasm_discover_single");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let wasm_path = pkg.join("gup_bg.wasm");
        std::fs::write(&wasm_path, b"\x00asm\x01\x00\x00\x00").unwrap();

        let result = discover_wasm_artifact(Some(&dir));
        assert!(result.is_ok(), "should find the single WASM file");
        assert_eq!(result.unwrap(), wasm_path);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_wasm_artifact_errors_when_pkg_missing() {
        let dir = std::env::temp_dir().join("gup_wasm_discover_nopkg");
        // Make sure it doesn't exist.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = discover_wasm_artifact(Some(&dir));
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("not found"),
            "should mention pkg/ not found: {err_msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_wasm_artifact_errors_when_no_wasm_file() {
        let dir = std::env::temp_dir().join("gup_wasm_discover_empty");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        // Create a non-matching file.
        std::fs::write(pkg.join("readme.txt"), b"hello").unwrap();

        let result = discover_wasm_artifact(Some(&dir));
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("No *_bg.wasm"),
            "should mention no WASM found: {err_msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_wasm_artifact_errors_when_ambiguous() {
        let dir = std::env::temp_dir().join("gup_wasm_discover_ambig");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("foo_bg.wasm"), b"\x00asm").unwrap();
        std::fs::write(pkg.join("bar_bg.wasm"), b"\x00asm").unwrap();

        let result = discover_wasm_artifact(Some(&dir));
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("ambiguous"),
            "should mention ambiguity: {err_msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_wasm_artifact_ignores_non_bg_wasm() {
        let dir = std::env::temp_dir().join("gup_wasm_discover_nonbg");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        // Only the _bg.wasm file should match; plain .wasm should not.
        std::fs::write(pkg.join("gup_bg.wasm"), b"\x00asm\x01\x00\x00\x00").unwrap();
        std::fs::write(pkg.join("gup.wasm"), b"\x00asm").unwrap();

        let result = discover_wasm_artifact(Some(&dir));
        assert!(result.is_ok());
        let found = result.unwrap();
        assert!(
            found
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("_bg.wasm"),
            "should pick the _bg.wasm file"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
