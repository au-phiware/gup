// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! SVG renderer and export options.
//!
//! The [`SvgRenderer`] traverses chart data — axis geometry, grid lines,
//! titles, and data mark instances — and produces a well-formed SVG
//! document.  The coordinate transform from GPU clip-space (Y-up,
//! `[-1, 1]`) to SVG viewport coordinates (Y-down,
//! `[0, width] × [0, height]`) is applied uniformly by the renderer so
//! individual marks and axes do not need to perform their own conversion.

use std::path::Path;

use crate::chart_builder::{ChartConfig, TitleAlignment};
use crate::error::{GupError, GupResult};
use crate::text::TextAnchor;

use super::element::{SvgElement, rgba_to_css};

// ---------------------------------------------------------------------------
// Export options
// ---------------------------------------------------------------------------

/// Options controlling SVG document generation.
///
/// # Examples
///
/// ```rust
/// use gup::export::svg::SvgExportOptions;
///
/// let opts = SvgExportOptions::new(800, 600)
///     .with_background([1.0, 1.0, 1.0, 1.0])
///     .with_css("text { font-family: sans-serif; }");
/// ```
#[derive(Debug, Clone)]
pub struct SvgExportOptions {
    /// Width of the SVG viewport in pixels.
    pub width: u32,
    /// Height of the SVG viewport in pixels.
    pub height: u32,
    /// Optional background colour (RGBA, each component 0.0–1.0).
    pub background: Option<[f32; 4]>,
    /// Optional extra CSS to embed in a `<style>` element.
    pub extra_css: Option<String>,
}

impl SvgExportOptions {
    /// Create export options with the given viewport dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            background: None,
            extra_css: None,
        }
    }

    /// Set the background colour.
    pub fn with_background(mut self, color: [f32; 4]) -> Self {
        self.background = Some(color);
        self
    }

    /// Set extra CSS to embed in the SVG.
    pub fn with_css(mut self, css: impl Into<String>) -> Self {
        self.extra_css = Some(css.into());
        self
    }
}

// ---------------------------------------------------------------------------
// Coordinate transform
// ---------------------------------------------------------------------------

/// Transform from GPU clip-space to SVG viewport coordinates.
///
/// GPU clip-space: X ∈ [-1, 1] left→right, Y ∈ [-1, 1] bottom→top.
/// SVG viewport:   X ∈ [0, width] left→right, Y ∈ [0, height] top→bottom.
///
/// ```text
/// svg_x = (clip_x + 1.0) / 2.0 * width
/// svg_y = (1.0 - clip_y) / 2.0 * height
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ClipToSvg {
    width: f32,
    height: f32,
}

impl ClipToSvg {
    /// Create a new transform for the given viewport size.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Transform a clip-space point to SVG viewport coordinates.
    #[inline]
    pub fn point(&self, clip_x: f32, clip_y: f32) -> (f32, f32) {
        let svg_x = (clip_x + 1.0) / 2.0 * self.width;
        let svg_y = (1.0 - clip_y) / 2.0 * self.height;
        (svg_x, svg_y)
    }

    /// Transform a clip-space distance along the X axis to SVG pixels.
    #[inline]
    pub fn scale_x(&self, clip_dx: f32) -> f32 {
        clip_dx / 2.0 * self.width
    }

    /// Transform a clip-space distance along the Y axis to SVG pixels.
    #[inline]
    pub fn scale_y(&self, clip_dy: f32) -> f32 {
        clip_dy / 2.0 * self.height
    }
}

// ---------------------------------------------------------------------------
// SVG Renderer
// ---------------------------------------------------------------------------

/// Renderer that produces an SVG document from chart data.
///
/// `SvgRenderer` works with [`ChartConfig`] and axis/grid/label data
/// that has already been computed (in NDC / clip-space) by the chart
/// builder.  It applies a [`ClipToSvg`] transform and serialises
/// everything into a well-formed SVG string.
///
/// # Coordinate Convention
///
/// All chart geometry produced by [`ComposedChart`](crate::chart_builder::ComposedChart)
/// uses GPU clip-space coordinates (Y-up, `[-1, 1]`).  The renderer
/// converts these to SVG viewport coordinates (Y-down,
/// `[0, width] × [0, height]`) using [`ClipToSvg`].
#[derive(Debug, Clone)]
pub struct SvgRenderer {
    options: SvgExportOptions,
}

impl SvgRenderer {
    /// Create a new SVG renderer with the given export options.
    pub fn new(options: SvgExportOptions) -> Self {
        Self { options }
    }

    /// Render chart data into a well-formed SVG document string.
    ///
    /// This method accepts the chart configuration and pre-computed
    /// geometry data, applies coordinate transforms, and produces an
    /// SVG string.
    ///
    /// # Arguments
    ///
    /// * `config` — The chart configuration (margins, title, colours, etc.).
    /// * `axis_line_vertices` — Pairs of vertices forming axis line segments (in NDC).
    /// * `tick_instances` — Per-tick instance data (in NDC).
    /// * `labels` — Axis labels with screen-space positions.
    /// * `data_elements` — Pre-built SVG elements for data marks (already in SVG coordinates).
    pub fn render(
        &self,
        config: &ChartConfig,
        axis_line_vertices: &[crate::render::Vertex],
        tick_instances: &[crate::axis::TickInstance],
        labels: &[crate::axis::AxisLabel],
        data_elements: &[SvgElement],
    ) -> GupResult<String> {
        let w = self.options.width as f32;
        let h = self.options.height as f32;
        let transform = ClipToSvg::new(w, h);

        let mut elements: Vec<SvgElement> = Vec::new();

        // 1. Background
        let bg_color = config
            .background_color
            .or(self.options.background)
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);
        elements.push(SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            fill: rgba_to_css(bg_color[0], bg_color[1], bg_color[2], bg_color[3]),
            stroke: None,
            stroke_width: None,
            rx: None,
        });

        // 2. Grid lines (rendered from tick instances that represent grid lines)
        if config.show_grid {
            let grid_elements = self.render_grid_lines(config, tick_instances, &transform);
            if !grid_elements.is_empty() {
                elements.push(SvgElement::Group {
                    class: Some("grid".to_string()),
                    transform: None,
                    children: grid_elements,
                });
            }
        }

        // 3. Data marks
        if !data_elements.is_empty() {
            elements.push(SvgElement::Group {
                class: Some("marks".to_string()),
                transform: None,
                children: data_elements.to_vec(),
            });
        }

        // 4. Axis lines
        let axis_elems = self.render_axis_lines(axis_line_vertices, config, &transform);
        if !axis_elems.is_empty() {
            elements.push(SvgElement::Group {
                class: Some("axes".to_string()),
                transform: None,
                children: axis_elems,
            });
        }

        // 5. Tick marks
        let tick_elems = self.render_ticks(tick_instances, &transform);
        if !tick_elems.is_empty() {
            elements.push(SvgElement::Group {
                class: Some("ticks".to_string()),
                transform: None,
                children: tick_elems,
            });
        }

        // 6. Axis labels
        let label_elems = self.render_labels(labels, config);
        if !label_elems.is_empty() {
            elements.push(SvgElement::Group {
                class: Some("labels".to_string()),
                transform: None,
                children: label_elems,
            });
        }

        // 7. Title
        if let Some(title_elem) = self.render_title(config) {
            elements.push(title_elem);
        }

        // Assemble document
        self.assemble_document(&elements)
    }

    /// Render a chart configuration without data marks — useful for
    /// charts where the caller provides only the config and axis geometry.
    pub fn render_config(
        &self,
        config: &ChartConfig,
        axis_line_vertices: &[crate::render::Vertex],
        tick_instances: &[crate::axis::TickInstance],
        labels: &[crate::axis::AxisLabel],
    ) -> GupResult<String> {
        self.render(config, axis_line_vertices, tick_instances, labels, &[])
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Render axis line segments.
    fn render_axis_lines(
        &self,
        vertices: &[crate::render::Vertex],
        config: &ChartConfig,
        transform: &ClipToSvg,
    ) -> Vec<SvgElement> {
        let _ = config; // reserved for future style overrides
        let mut elems = Vec::new();
        // Vertices come in pairs (LineList topology)
        for pair in vertices.chunks_exact(2) {
            let (x1, y1) = transform.point(pair[0].position[0], pair[0].position[1]);
            let (x2, y2) = transform.point(pair[1].position[0], pair[1].position[1]);
            let color = rgba_to_css(
                pair[0].color[0],
                pair[0].color[1],
                pair[0].color[2],
                pair[0].color[3],
            );
            elems.push(SvgElement::Line {
                x1,
                y1,
                x2,
                y2,
                stroke: color,
                stroke_width: 1.0,
                stroke_dasharray: None,
            });
        }
        elems
    }

    /// Render tick mark instances as line elements.
    fn render_ticks(
        &self,
        instances: &[crate::axis::TickInstance],
        transform: &ClipToSvg,
    ) -> Vec<SvgElement> {
        let mut elems = Vec::new();
        for inst in instances {
            let (x1, y1) = transform.point(inst.position[0], inst.position[1]);
            let (x2, y2) = transform.point(
                inst.position[0] + inst.tick_vector[0],
                inst.position[1] + inst.tick_vector[1],
            );
            let color = rgba_to_css(inst.color[0], inst.color[1], inst.color[2], inst.color[3]);
            elems.push(SvgElement::Line {
                x1,
                y1,
                x2,
                y2,
                stroke: color,
                stroke_width: 1.0,
                stroke_dasharray: None,
            });
        }
        elems
    }

    /// Render grid lines based on the chart's grid configuration.
    ///
    /// Grid lines extend across the chart area at the same positions as
    /// tick marks. The styling (colour, width, dash pattern) comes from
    /// the [`GridConfiguration`](crate::grid::GridConfiguration).
    fn render_grid_lines(
        &self,
        config: &ChartConfig,
        tick_instances: &[crate::axis::TickInstance],
        transform: &ClipToSvg,
    ) -> Vec<SvgElement> {
        let grid = &config.grid_config;
        let major = &grid.major_grid;
        if !major.enabled {
            return Vec::new();
        }

        let color = rgba_to_css(
            major.color[0],
            major.color[1],
            major.color[2],
            major.color[3] * major.opacity,
        );
        let dash = major.dash_pattern.as_ref().map(|p| {
            p.iter()
                .map(|v| format!("{v}"))
                .collect::<Vec<_>>()
                .join(" ")
        });

        let w = self.options.width as f32;
        let h = self.options.height as f32;

        let mut elems = Vec::new();

        // For each tick, extend a line across the chart area
        for inst in tick_instances {
            let (px, py) = transform.point(inst.position[0], inst.position[1]);

            // Determine orientation from the tick vector direction:
            // If tick_vector is primarily vertical, this is on a horizontal axis → vertical grid line
            // If tick_vector is primarily horizontal, this is on a vertical axis → horizontal grid line
            let dx = inst.tick_vector[0].abs();
            let dy = inst.tick_vector[1].abs();

            if dy > dx && grid.show_vertical {
                // Tick on horizontal axis (bottom/top) → vertical grid line
                elems.push(SvgElement::Line {
                    x1: px,
                    y1: 0.0,
                    x2: px,
                    y2: h,
                    stroke: color.clone(),
                    stroke_width: major.line_width,
                    stroke_dasharray: dash.clone(),
                });
            } else if dx > dy && grid.show_horizontal {
                // Tick on vertical axis (left/right) → horizontal grid line
                elems.push(SvgElement::Line {
                    x1: 0.0,
                    y1: py,
                    x2: w,
                    y2: py,
                    stroke: color.clone(),
                    stroke_width: major.line_width,
                    stroke_dasharray: dash.clone(),
                });
            }
        }

        elems
    }

    /// Render axis labels as `<text>` elements.
    ///
    /// Labels have `screen_position` in pixel coordinates already computed
    /// by the axis renderer, so no clip-space transform is needed.
    fn render_labels(
        &self,
        labels: &[crate::axis::AxisLabel],
        config: &ChartConfig,
    ) -> Vec<SvgElement> {
        labels
            .iter()
            .map(|label| {
                let style = config.label_style.clone();
                let font_family = style
                    .font_family
                    .as_deref()
                    .unwrap_or("sans-serif")
                    .to_string();
                let (text_anchor, dominant_baseline) = text_anchor_to_svg(&label.anchor);

                SvgElement::Text {
                    x: label.screen_position.x,
                    y: label.screen_position.y,
                    content: label.text.clone(),
                    font_family,
                    font_size: style.font_size,
                    text_anchor: text_anchor.to_string(),
                    dominant_baseline: dominant_baseline.to_string(),
                    fill: rgba_to_css(style.color.x, style.color.y, style.color.z, style.color.w),
                    font_weight: None,
                }
            })
            .collect()
    }

    /// Render the chart title (and optional subtitle) as `<text>` elements.
    fn render_title(&self, config: &ChartConfig) -> Option<SvgElement> {
        let title_config = config.title_config.as_ref()?;

        let w = self.options.width as f32;
        let margins = &config.margins;

        // Horizontal position based on alignment
        let x = match title_config.alignment {
            TitleAlignment::Left => margins.left,
            TitleAlignment::Center => w / 2.0,
            TitleAlignment::Right => w - margins.right,
        };

        let text_anchor = match title_config.alignment {
            TitleAlignment::Left => "start",
            TitleAlignment::Center => "middle",
            TitleAlignment::Right => "end",
        };

        // Vertical position: default to center of top margin
        let y = title_config.y_offset.unwrap_or(margins.top / 2.0);

        let style = &config.title_style;
        let font_family = style
            .font_family
            .as_deref()
            .unwrap_or("sans-serif")
            .to_string();

        let mut children = vec![SvgElement::Text {
            x,
            y,
            content: title_config.text.clone(),
            font_family: font_family.clone(),
            font_size: style.font_size,
            text_anchor: text_anchor.to_string(),
            dominant_baseline: "central".to_string(),
            fill: rgba_to_css(style.color.x, style.color.y, style.color.z, style.color.w),
            font_weight: if style.weight > 0.7 {
                Some("bold".to_string())
            } else {
                None
            },
        }];

        // Subtitle
        if let Some(subtitle) = &title_config.subtitle {
            let sub_style = &title_config.subtitle_style;
            let sub_y = y + style.font_size * title_config.line_spacing;
            let sub_font = sub_style
                .font_family
                .as_deref()
                .unwrap_or("sans-serif")
                .to_string();
            children.push(SvgElement::Text {
                x,
                y: sub_y,
                content: subtitle.clone(),
                font_family: sub_font,
                font_size: sub_style.font_size,
                text_anchor: text_anchor.to_string(),
                dominant_baseline: "central".to_string(),
                fill: rgba_to_css(
                    sub_style.color.x,
                    sub_style.color.y,
                    sub_style.color.z,
                    sub_style.color.w,
                ),
                font_weight: None,
            });
        }

        Some(SvgElement::Group {
            class: Some("title".to_string()),
            transform: None,
            children,
        })
    }

    /// Assemble the final SVG document from a list of elements.
    fn assemble_document(&self, elements: &[SvgElement]) -> GupResult<String> {
        let w = self.options.width;
        let h = self.options.height;

        let mut doc = String::with_capacity(4096);
        doc.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        doc.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n"
        ));

        // Embed optional CSS
        if let Some(css) = &self.options.extra_css {
            doc.push_str("  <style>\n    ");
            doc.push_str(css);
            doc.push_str("\n  </style>\n");
        }

        for elem in elements {
            doc.push_str(&elem.to_svg_string(1));
            doc.push('\n');
        }

        doc.push_str("</svg>\n");
        Ok(doc)
    }
}

// ---------------------------------------------------------------------------
// Convenience: export_svg on ChartConfig
// ---------------------------------------------------------------------------

/// Write an SVG document to a file.
///
/// This is a standalone helper; for ergonomic use, prefer
/// [`ComposedChart::export_svg`](crate::chart_builder::ComposedChart::export_svg).
pub fn write_svg_to_file(svg: &str, path: impl AsRef<Path>) -> GupResult<()> {
    std::fs::write(path.as_ref(), svg).map_err(|e| GupError::FileError {
        path: path.as_ref().display().to_string(),
        error: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a Gup [`TextAnchor`] to SVG `text-anchor` and `dominant-baseline`
/// attribute values.
fn text_anchor_to_svg(anchor: &TextAnchor) -> (&'static str, &'static str) {
    match anchor {
        TextAnchor::TopLeft => ("start", "hanging"),
        TextAnchor::TopCenter => ("middle", "hanging"),
        TextAnchor::TopRight => ("end", "hanging"),
        TextAnchor::CenterLeft => ("start", "central"),
        TextAnchor::Center => ("middle", "central"),
        TextAnchor::CenterRight => ("end", "central"),
        TextAnchor::BottomLeft => ("start", "alphabetic"),
        TextAnchor::BottomCenter => ("middle", "alphabetic"),
        TextAnchor::BottomRight => ("end", "alphabetic"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Coordinate transform tests (AC2) ----------------------------------

    #[test]
    fn test_clip_to_svg_corners_800x600() {
        let t = ClipToSvg::new(800.0, 600.0);

        // Bottom-left in clip space → top-right in SVG? No:
        // (-1, -1) clip → (0, 600) SVG  (left, bottom in clip → left, bottom in SVG Y-down)
        let (x, y) = t.point(-1.0, -1.0);
        assert!((x - 0.0).abs() < 0.01, "x={x}");
        assert!((y - 600.0).abs() < 0.01, "y={y}");

        // (1, 1) clip → (800, 0) SVG
        let (x, y) = t.point(1.0, 1.0);
        assert!((x - 800.0).abs() < 0.01, "x={x}");
        assert!((y - 0.0).abs() < 0.01, "y={y}");

        // (-1, 1) clip → (0, 0) SVG  (top-left)
        let (x, y) = t.point(-1.0, 1.0);
        assert!((x - 0.0).abs() < 0.01, "x={x}");
        assert!((y - 0.0).abs() < 0.01, "y={y}");

        // (1, -1) clip → (800, 600) SVG  (bottom-right)
        let (x, y) = t.point(1.0, -1.0);
        assert!((x - 800.0).abs() < 0.01, "x={x}");
        assert!((y - 600.0).abs() < 0.01, "y={y}");
    }

    #[test]
    fn test_clip_to_svg_corners_1920x1080() {
        let t = ClipToSvg::new(1920.0, 1080.0);

        let (x, y) = t.point(-1.0, -1.0);
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 1080.0).abs() < 0.01);

        let (x, y) = t.point(1.0, 1.0);
        assert!((x - 1920.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_clip_to_svg_centre() {
        let t = ClipToSvg::new(800.0, 600.0);
        let (x, y) = t.point(0.0, 0.0);
        assert!((x - 400.0).abs() < 0.01);
        assert!((y - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_clip_to_svg_corners_400x400() {
        let t = ClipToSvg::new(400.0, 400.0);

        let (x, y) = t.point(-1.0, -1.0);
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 400.0).abs() < 0.01);

        let (x, y) = t.point(1.0, 1.0);
        assert!((x - 400.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
    }

    // -- SVG export options tests -------------------------------------------

    #[test]
    fn test_export_options_defaults() {
        let opts = SvgExportOptions::new(800, 600);
        assert_eq!(opts.width, 800);
        assert_eq!(opts.height, 600);
        assert!(opts.background.is_none());
        assert!(opts.extra_css.is_none());
    }

    #[test]
    fn test_export_options_builder() {
        let opts = SvgExportOptions::new(1024, 768)
            .with_background([1.0, 1.0, 1.0, 1.0])
            .with_css("text { font-family: serif; }");
        assert!(opts.background.is_some());
        assert_eq!(
            opts.extra_css.as_deref(),
            Some("text { font-family: serif; }")
        );
    }

    // -- Text anchor mapping tests ------------------------------------------

    #[test]
    fn test_text_anchor_mapping() {
        assert_eq!(
            text_anchor_to_svg(&TextAnchor::TopLeft),
            ("start", "hanging")
        );
        assert_eq!(
            text_anchor_to_svg(&TextAnchor::Center),
            ("middle", "central")
        );
        assert_eq!(
            text_anchor_to_svg(&TextAnchor::BottomRight),
            ("end", "alphabetic")
        );
        assert_eq!(
            text_anchor_to_svg(&TextAnchor::TopCenter),
            ("middle", "hanging")
        );
        assert_eq!(
            text_anchor_to_svg(&TextAnchor::CenterRight),
            ("end", "central")
        );
    }

    // -- Document assembly tests -------------------------------------------

    #[test]
    fn test_assemble_basic_document() {
        let renderer = SvgRenderer::new(SvgExportOptions::new(100, 50));
        let elements = vec![SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            fill: "white".to_string(),
            stroke: None,
            stroke_width: None,
            rx: None,
        }];
        let doc = renderer.assemble_document(&elements).unwrap();
        assert!(doc.starts_with("<?xml version=\"1.0\""));
        assert!(doc.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(doc.contains("width=\"100\""));
        assert!(doc.contains("height=\"50\""));
        assert!(doc.contains("</svg>"));
        assert!(doc.contains("<rect"));
    }

    #[test]
    fn test_assemble_document_with_css() {
        let opts = SvgExportOptions::new(100, 50).with_css("text { fill: red; }");
        let renderer = SvgRenderer::new(opts);
        let doc = renderer.assemble_document(&[]).unwrap();
        assert!(doc.contains("<style>"));
        assert!(doc.contains("text { fill: red; }"));
        assert!(doc.contains("</style>"));
    }

    // -- Full render test --------------------------------------------------

    #[test]
    fn test_render_empty_chart() {
        let opts = SvgExportOptions::new(800, 600);
        let renderer = SvgRenderer::new(opts);
        let config = ChartConfig::default();
        let svg = renderer.render_config(&config, &[], &[], &[]).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        // Should have at least the background rect
        assert!(svg.contains("<rect"));
    }

    // -- Write to file test ------------------------------------------------

    #[test]
    fn test_write_svg_to_file() {
        let dir = std::env::temp_dir().join("gup_svg_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_output.svg");

        let svg_content = "<svg></svg>";
        write_svg_to_file(svg_content, &path).unwrap();

        let read_back = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read_back, svg_content);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_write_svg_to_file_error() {
        let result = write_svg_to_file("test", "/nonexistent/dir/file.svg");
        assert!(result.is_err());
        if let Err(GupError::FileError { path, .. }) = result {
            assert!(path.contains("file.svg"));
        }
    }
}
