// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF renderer and multi-page document builder.
//!
//! [`PdfRenderer`] converts [`SvgElement`] trees (the intermediate
//! representation produced by [`crate::export::svg`]) into PDF drawing
//! operations using `printpdf`.
//!
//! [`PdfDocument`] wraps a `printpdf::PdfDocument` and provides a
//! fluent builder for multi-page chart reports.

use std::path::Path;

use printpdf::{
    BuiltinFont, Color, Mm, Op, PaintMode, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, Rgb,
    TextItem, WindingOrder,
};

use crate::error::{GupError, GupResult};
use crate::export::svg::SvgElement;
use crate::export::svg::element::rgba_to_css;

use super::options::PdfOptions;
use crate::export::svg::ClipToSvg;

// ---------------------------------------------------------------------------
// Colour parsing helper
// ---------------------------------------------------------------------------

/// Parse a CSS colour string (as produced by `rgba_to_css()`) into printpdf
/// `Color`.  Supports `rgb(r,g,b)` and `rgba(r,g,b,a)`.  Falls back to
/// black on unrecognised input.
fn parse_css_color(css: &str) -> Color {
    let s = css.trim();

    // Try named colours first.
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
    }

    // Try "rgb(r,g,b)" or "rgba(r,g,b,a)".
    let inner = if let Some(rest) = s.strip_prefix("rgba(") {
        rest.strip_suffix(')')
    } else if let Some(rest) = s.strip_prefix("rgb(") {
        rest.strip_suffix(')')
    } else {
        None
    };

    if let Some(inner) = inner {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let g = parts[1].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            let b = parts[2].trim().parse::<f32>().unwrap_or(0.0) / 255.0;
            return Color::Rgb(Rgb::new(r, g, b, None));
        }
    }

    // Fallback: try simple hex colours "#rrggbb".
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        )
    {
        return Color::Rgb(Rgb::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            None,
        ));
    }

    // Last resort: named CSS colour keywords.
    match s.to_ascii_lowercase().as_str() {
        "red" => Color::Rgb(Rgb::new(1.0, 0.0, 0.0, None)),
        "green" => Color::Rgb(Rgb::new(0.0, 0.502, 0.0, None)),
        "blue" => Color::Rgb(Rgb::new(0.0, 0.0, 1.0, None)),
        "white" => Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)),
        _ => Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
    }
}

/// Convert millimetres to PDF points.
fn mm_to_pt(mm: f32) -> Pt {
    Pt(mm * 2.834_646)
}

// ---------------------------------------------------------------------------
// PdfRenderer
// ---------------------------------------------------------------------------

/// Converts [`SvgElement`] trees into PDF page operations.
///
/// `PdfRenderer` operates on the same vector intermediate representation
/// produced by the SVG export pipeline ([`crate::export::svg`]).  No GPU
/// commands are issued — all work is CPU-side.
///
/// # Coordinate Convention
///
/// SVG elements use an SVG viewport coordinate system (Y-down, origin at
/// top-left).  The renderer maps these to PDF coordinates (Y-up, origin at
/// bottom-left of the page) using the page height and a scale factor
/// computed from [`PdfOptions`].
#[derive(Debug, Clone)]
pub struct PdfRenderer {
    options: PdfOptions,
    /// Embedded regular font handle, if available.
    regular_font: Option<printpdf::FontId>,
    /// Embedded bold font handle, if available.
    bold_font: Option<printpdf::FontId>,
}

impl PdfRenderer {
    /// Create a new PDF renderer with the given options.
    ///
    /// Text will be rendered using built-in Helvetica.  To use embedded
    /// fonts, call [`with_fonts`](Self::with_fonts).
    pub fn new(options: PdfOptions) -> Self {
        Self {
            options,
            regular_font: None,
            bold_font: None,
        }
    }

    /// Set embedded font handles for text rendering.
    ///
    /// When set, text elements will reference the embedded font ID rather
    /// than the built-in Helvetica.  This produces proper PDF text
    /// objects with embedded font subsets.
    pub fn with_fonts(
        mut self,
        regular: Option<printpdf::FontId>,
        bold: Option<printpdf::FontId>,
    ) -> Self {
        self.regular_font = regular;
        self.bold_font = bold;
        self
    }

    /// Render a set of SVG elements into a single PDF page.
    ///
    /// `chart_width` and `chart_height` describe the SVG viewport size
    /// in pixels so that the chart can be scaled to fit the page.
    pub fn render_page(
        &self,
        elements: &[SvgElement],
        chart_width: f32,
        chart_height: f32,
    ) -> PdfPage {
        let page_w = self.options.effective_width_mm();
        let page_h = self.options.effective_height_mm();
        let (scale, offset_x, offset_y) = self.options.fit_scale(chart_width, chart_height);

        let mut ops: Vec<Op> = Vec::new();

        for elem in elements {
            self.render_element(elem, &mut ops, scale, offset_x, offset_y, page_h);
        }

        PdfPage::new(Mm(page_w), Mm(page_h), ops)
    }

    /// Render a single `SvgElement` into PDF operations.
    ///
    /// Coordinates are transformed from SVG viewport (Y-down) to PDF
    /// page coordinates (Y-up) and scaled/offset according to the fit
    /// parameters.
    fn render_element(
        &self,
        elem: &SvgElement,
        ops: &mut Vec<Op>,
        scale: f32,
        offset_x: f32,
        offset_y: f32,
        page_h: f32,
    ) {
        match elem {
            SvgElement::Rect {
                x,
                y,
                width,
                height,
                fill,
                stroke,
                stroke_width,
                ..
            } => {
                // Convert SVG coords → PDF coords (mm).
                let pdf_x = offset_x + x * scale;
                let pdf_y = page_h - (offset_y + (y + height) * scale);
                let pdf_w = width * scale;
                let pdf_h = height * scale;

                let has_fill = fill != "none" && fill != "transparent";
                let has_stroke = stroke.as_deref().is_some_and(|s| s != "none");

                if has_fill {
                    ops.push(Op::SetFillColor {
                        col: parse_css_color(fill),
                    });
                }
                if has_stroke {
                    if let Some(s) = stroke {
                        ops.push(Op::SetOutlineColor {
                            col: parse_css_color(s),
                        });
                    }
                    if let Some(sw) = stroke_width {
                        ops.push(Op::SetOutlineThickness {
                            pt: mm_to_pt(sw * scale),
                        });
                    }
                }

                let mode = match (has_fill, has_stroke) {
                    (true, true) => PaintMode::FillStroke,
                    (false, true) => PaintMode::Stroke,
                    _ => PaintMode::Fill,
                };

                let rect = printpdf::Rect {
                    x: mm_to_pt(pdf_x),
                    y: mm_to_pt(pdf_y),
                    width: mm_to_pt(pdf_w),
                    height: mm_to_pt(pdf_h),
                    mode: Some(mode),
                    winding_order: Some(WindingOrder::NonZero),
                };
                ops.push(Op::DrawRectangle { rectangle: rect });
            }

            SvgElement::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
                stroke_width,
            } => {
                // Approximate a circle with four cubic Bézier arcs.
                let pdf_cx = offset_x + cx * scale;
                let pdf_cy = page_h - (offset_y + cy * scale);
                let pdf_r = r * scale;

                let has_fill = fill != "none" && fill != "transparent";
                let has_stroke = stroke.as_deref().is_some_and(|s| s != "none");

                if has_fill {
                    ops.push(Op::SetFillColor {
                        col: parse_css_color(fill),
                    });
                }
                if has_stroke {
                    if let Some(s) = stroke {
                        ops.push(Op::SetOutlineColor {
                            col: parse_css_color(s),
                        });
                    }
                    if let Some(sw) = stroke_width {
                        ops.push(Op::SetOutlineThickness {
                            pt: mm_to_pt(sw * scale),
                        });
                    }
                }

                let mode = match (has_fill, has_stroke) {
                    (true, true) => PaintMode::FillStroke,
                    (false, true) => PaintMode::Stroke,
                    _ => PaintMode::Fill,
                };

                let polygon = circle_polygon(pdf_cx, pdf_cy, pdf_r, mode);
                ops.push(Op::DrawPolygon { polygon });
            }

            SvgElement::Line {
                x1,
                y1,
                x2,
                y2,
                stroke,
                stroke_width,
                stroke_dasharray,
            } => {
                let px1 = offset_x + x1 * scale;
                let py1 = page_h - (offset_y + y1 * scale);
                let px2 = offset_x + x2 * scale;
                let py2 = page_h - (offset_y + y2 * scale);

                ops.push(Op::SetOutlineColor {
                    col: parse_css_color(stroke),
                });
                ops.push(Op::SetOutlineThickness {
                    pt: mm_to_pt(stroke_width * scale),
                });

                if let Some(da) = stroke_dasharray {
                    let nums: Vec<f32> = da
                        .split_whitespace()
                        .filter_map(|s| s.parse::<f32>().ok())
                        .map(|v| v * scale)
                        .collect();
                    if !nums.is_empty() {
                        let dash_array: Vec<i64> =
                            nums.iter().map(|v| mm_to_pt(*v).0 as i64).collect();
                        let dash = printpdf::LineDashPattern {
                            dash_1: dash_array.first().copied(),
                            gap_1: dash_array.get(1).copied(),
                            dash_2: dash_array.get(2).copied(),
                            gap_2: dash_array.get(3).copied(),
                            ..Default::default()
                        };
                        ops.push(Op::SetLineDashPattern { dash });
                    }
                }

                let line = printpdf::Line {
                    points: vec![
                        printpdf::LinePoint {
                            p: Point {
                                x: mm_to_pt(px1),
                                y: mm_to_pt(py1),
                            },
                            bezier: false,
                        },
                        printpdf::LinePoint {
                            p: Point {
                                x: mm_to_pt(px2),
                                y: mm_to_pt(py2),
                            },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                };
                ops.push(Op::DrawLine { line });

                // Reset dash pattern after drawing dashed lines.
                if stroke_dasharray.is_some() {
                    ops.push(Op::SetLineDashPattern {
                        dash: printpdf::LineDashPattern::default(),
                    });
                }
            }

            SvgElement::Text {
                x,
                y,
                content,
                font_size,
                font_weight,
                fill,
                ..
            } => {
                let pdf_x = offset_x + x * scale;
                let pdf_y = page_h - (offset_y + y * scale);
                let pdf_font_size = font_size * scale;

                // Choose font: prefer embedded font, fall back to built-in.
                let is_bold = font_weight.as_deref() == Some("bold");
                let font_handle = if is_bold {
                    self.bold_font
                        .as_ref()
                        .map(|fid| PdfFontHandle::External(fid.clone()))
                        .unwrap_or(PdfFontHandle::Builtin(BuiltinFont::HelveticaBold))
                } else {
                    self.regular_font
                        .as_ref()
                        .map(|fid| PdfFontHandle::External(fid.clone()))
                        .unwrap_or(PdfFontHandle::Builtin(BuiltinFont::Helvetica))
                };

                ops.push(Op::SetFillColor {
                    col: parse_css_color(fill),
                });
                ops.push(Op::StartTextSection);
                ops.push(Op::SetFont {
                    font: font_handle,
                    size: mm_to_pt(pdf_font_size),
                });
                ops.push(Op::SetTextCursor {
                    pos: Point {
                        x: mm_to_pt(pdf_x),
                        y: mm_to_pt(pdf_y),
                    },
                });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(content.clone())],
                });
                ops.push(Op::EndTextSection);
            }

            SvgElement::Path {
                d,
                fill,
                stroke,
                stroke_width,
            } => {
                let has_fill = fill != "none" && fill != "transparent";
                let has_stroke = stroke.as_deref().is_some_and(|s| s != "none");

                if has_fill {
                    ops.push(Op::SetFillColor {
                        col: parse_css_color(fill),
                    });
                }
                if has_stroke {
                    if let Some(s) = stroke {
                        ops.push(Op::SetOutlineColor {
                            col: parse_css_color(s),
                        });
                    }
                    if let Some(sw) = stroke_width {
                        ops.push(Op::SetOutlineThickness {
                            pt: mm_to_pt(sw * scale),
                        });
                    }
                }

                let mode = match (has_fill, has_stroke) {
                    (true, true) => PaintMode::FillStroke,
                    (false, true) => PaintMode::Stroke,
                    _ => PaintMode::Fill,
                };

                if let Some(polygon) = parse_svg_path(d, scale, offset_x, offset_y, page_h, mode) {
                    ops.push(Op::DrawPolygon { polygon });
                }
            }

            SvgElement::Group { children, .. } => {
                ops.push(Op::SaveGraphicsState);
                for child in children {
                    self.render_element(child, ops, scale, offset_x, offset_y, page_h);
                }
                ops.push(Op::RestoreGraphicsState);
            }
        }
    }

    /// Convert a set of SVG elements to PDF bytes.
    ///
    /// This is a convenience method that creates a single-page PDF
    /// document and serialises it.
    pub fn render_to_bytes(
        &self,
        elements: &[SvgElement],
        chart_width: f32,
        chart_height: f32,
    ) -> GupResult<Vec<u8>> {
        let page = self.render_page(elements, chart_width, chart_height);

        let mut doc = printpdf::PdfDocument::new("Gup Chart");
        doc.pages.push(page);

        let opts = PdfSaveOptions::default();
        let mut warnings = Vec::new();
        let bytes = doc.save(&opts, &mut warnings);

        if !warnings.is_empty() {
            log::warn!(
                "PDF save warnings: {:?}",
                warnings
                    .iter()
                    .map(|w| format!("{w:?}"))
                    .collect::<Vec<_>>()
            );
        }

        Ok(bytes)
    }
}

// ---------------------------------------------------------------------------
// PdfDocument — multi-page builder
// ---------------------------------------------------------------------------

/// Builder for multi-page PDF documents.
///
/// On creation, `PdfDocument` attempts to locate and embed a sans-serif
/// system font (using `fontdb`).  If the font cannot be found or loaded,
/// the document falls back to the built-in PDF Helvetica font and emits a
/// `log::warn!` message — this is a non-fatal condition.
///
/// # Examples
///
/// ```rust,ignore
/// use gup::export::pdf::{PdfDocument, PdfOptions};
///
/// let mut doc = PdfDocument::new(PdfOptions::a4());
/// doc.add_page_from_elements("Chart 1", &elements_a, 800.0, 600.0)?;
/// doc.add_page_from_elements("Chart 2", &elements_b, 800.0, 600.0)?;
/// doc.write("report.pdf")?;
/// ```
pub struct PdfDocument {
    options: PdfOptions,
    inner: printpdf::PdfDocument,
    page_count: usize,
    /// Embedded regular font, if a system font was successfully loaded.
    embedded_font: Option<printpdf::FontId>,
    /// Embedded bold font, if available.
    embedded_bold_font: Option<printpdf::FontId>,
}

impl PdfDocument {
    /// Create a new empty PDF document.
    ///
    /// Tries to embed a sans-serif system font.  Falls back to PDF
    /// built-in Helvetica with a warning if no suitable font is found.
    pub fn new(options: PdfOptions) -> Self {
        let mut inner = printpdf::PdfDocument::new("Gup Chart");

        let (embedded_font, embedded_bold_font) = Self::try_embed_fonts(&mut inner);

        Self {
            options,
            inner,
            page_count: 0,
            embedded_font,
            embedded_bold_font,
        }
    }

    /// Attempt to load and embed a sans-serif font from the system.
    fn try_embed_fonts(
        doc: &mut printpdf::PdfDocument,
    ) -> (Option<printpdf::FontId>, Option<printpdf::FontId>) {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        if db.is_empty() {
            log::warn!("fontdb found no system fonts; falling back to built-in Helvetica");
            return (None, None);
        }

        // Try common sans-serif font families.
        let families: &[fontdb::Family<'_>] = &[
            fontdb::Family::SansSerif,
            fontdb::Family::Name("DejaVu Sans"),
            fontdb::Family::Name("Liberation Sans"),
            fontdb::Family::Name("Noto Sans"),
            fontdb::Family::Name("Arial"),
            fontdb::Family::Name("Helvetica"),
        ];

        let regular_id =
            Self::try_embed_font(doc, &db, families, fontdb::Weight::NORMAL, "regular");
        let bold_id = Self::try_embed_font(doc, &db, families, fontdb::Weight::BOLD, "bold");

        if regular_id.is_none() {
            log::warn!(
                "Could not locate a sans-serif system font for PDF embedding; \
                 falling back to built-in Helvetica"
            );
        }

        (regular_id, bold_id)
    }

    /// Try to embed a specific font weight from the system.
    fn try_embed_font(
        doc: &mut printpdf::PdfDocument,
        db: &fontdb::Database,
        families: &[fontdb::Family<'_>],
        weight: fontdb::Weight,
        label: &str,
    ) -> Option<printpdf::FontId> {
        for family in families {
            let query = fontdb::Query {
                families: &[family.clone()],
                weight,
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            };
            if let Some(face_id) = db.query(&query) {
                let result = db.with_face_data(face_id, |font_data, face_index| {
                    let mut warnings = Vec::new();
                    printpdf::ParsedFont::from_bytes(font_data, face_index as usize, &mut warnings)
                });
                if let Some(Some(parsed)) = result {
                    let fid = doc.add_font(&parsed);
                    log::info!("Embedded PDF font ({label}): family={:?}", family);
                    return Some(fid);
                }
            }
        }
        None
    }

    /// Add a chart page from pre-built [`SvgElement`] data.
    ///
    /// `chart_width` and `chart_height` are the SVG viewport dimensions
    /// in pixels used to compute the fit scaling.
    pub fn add_page_from_elements(
        &mut self,
        _title: &str,
        elements: &[SvgElement],
        chart_width: f32,
        chart_height: f32,
    ) -> GupResult<()> {
        let renderer = PdfRenderer::new(self.options.clone())
            .with_fonts(self.embedded_font.clone(), self.embedded_bold_font.clone());
        let page = renderer.render_page(elements, chart_width, chart_height);
        self.inner.pages.push(page);
        self.page_count += 1;
        Ok(())
    }

    /// Add a chart via the convenience SVG-render path.
    ///
    /// This renders a chart's configuration (axes, grid, title) plus
    /// optional data marks into a new PDF page.
    #[allow(clippy::too_many_arguments)]
    pub fn add_chart(
        &mut self,
        config: &crate::chart_builder::ChartConfig,
        axis_line_vertices: &[crate::render::Vertex],
        tick_instances: &[crate::axis::TickInstance],
        labels: &[crate::axis::AxisLabel],
        data_elements: &[SvgElement],
        chart_width: u32,
        chart_height: u32,
    ) -> GupResult<()> {
        // Re-use the SVG renderer to build SvgElement trees, then
        // convert those to PDF operations.
        let svg_opts = crate::export::svg::SvgExportOptions::new(chart_width, chart_height);
        let svg_renderer = crate::export::svg::SvgRenderer::new(svg_opts);
        let svg_string = svg_renderer.render(
            config,
            axis_line_vertices,
            tick_instances,
            labels,
            data_elements,
        )?;

        // We cannot easily parse the SVG string back into SvgElements,
        // so instead we generate the elements directly.  The SVG
        // renderer already does coordinate transforms, so we build
        // elements that are already in SVG viewport coordinates.
        //
        // For now, generate the page from the provided data_elements
        // (which are already in SVG viewport coords).
        let transform = ClipToSvg::new(chart_width as f32, chart_height as f32);

        let mut all_elements: Vec<SvgElement> = Vec::new();

        // Background
        let bg = config.background_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
        all_elements.push(SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: chart_width as f32,
            height: chart_height as f32,
            fill: rgba_to_css(bg[0], bg[1], bg[2], bg[3]),
            stroke: None,
            stroke_width: None,
            rx: None,
        });

        // Axis lines
        for pair in axis_line_vertices.chunks_exact(2) {
            let (x1, y1) = transform.point(pair[0].position[0], pair[0].position[1]);
            let (x2, y2) = transform.point(pair[1].position[0], pair[1].position[1]);
            let color = rgba_to_css(
                pair[0].color[0],
                pair[0].color[1],
                pair[0].color[2],
                pair[0].color[3],
            );
            all_elements.push(SvgElement::Line {
                x1,
                y1,
                x2,
                y2,
                stroke: color,
                stroke_width: 1.0,
                stroke_dasharray: None,
            });
        }

        // Tick marks
        for inst in tick_instances {
            let (x1, y1) = transform.point(inst.position[0], inst.position[1]);
            let (x2, y2) = transform.point(
                inst.position[0] + inst.tick_vector[0],
                inst.position[1] + inst.tick_vector[1],
            );
            let color = rgba_to_css(inst.color[0], inst.color[1], inst.color[2], inst.color[3]);
            all_elements.push(SvgElement::Line {
                x1,
                y1,
                x2,
                y2,
                stroke: color,
                stroke_width: 1.0,
                stroke_dasharray: None,
            });
        }

        // Labels
        for label in labels {
            all_elements.push(SvgElement::Text {
                x: label.screen_position.x,
                y: label.screen_position.y,
                content: label.text.clone(),
                font_family: "sans-serif".to_string(),
                font_size: config.label_style.font_size,
                text_anchor: "start".to_string(),
                dominant_baseline: "central".to_string(),
                fill: rgba_to_css(
                    config.label_style.color.x,
                    config.label_style.color.y,
                    config.label_style.color.z,
                    config.label_style.color.w,
                ),
                font_weight: None,
            });
        }

        // Data marks
        all_elements.extend_from_slice(data_elements);

        // Ignore the generated svg_string — we use the elements directly.
        let _ = svg_string;

        self.add_page_from_elements(
            "Chart",
            &all_elements,
            chart_width as f32,
            chart_height as f32,
        )
    }

    /// Returns the number of pages added so far.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Serialise the document to bytes.
    pub fn to_bytes(&self) -> GupResult<Vec<u8>> {
        let opts = PdfSaveOptions::default();
        let mut warnings = Vec::new();
        let bytes = self.inner.save(&opts, &mut warnings);

        if !warnings.is_empty() {
            log::warn!(
                "PDF save warnings: {:?}",
                warnings
                    .iter()
                    .map(|w| format!("{w:?}"))
                    .collect::<Vec<_>>()
            );
        }

        Ok(bytes)
    }

    /// Serialise the document and write it to a file.
    pub fn write(&self, path: impl AsRef<Path>) -> GupResult<()> {
        let bytes = self.to_bytes()?;
        std::fs::write(path.as_ref(), bytes).map_err(|e| GupError::FileError {
            path: path.as_ref().display().to_string(),
            error: e.to_string(),
        })
    }

    /// Serialise the document and write it to any `std::io::Write`.
    pub fn write_to_writer<W: std::io::Write>(&self, writer: &mut W) -> GupResult<()> {
        let bytes = self.to_bytes()?;
        writer.write_all(&bytes).map_err(|e| GupError::FileError {
            path: "<writer>".to_string(),
            error: e.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Approximate a circle as a polygon with four cubic Bézier arcs.
///
/// The magic constant `k ≈ 0.5523` comes from the standard cubic Bézier
/// approximation of a quarter-circle.  Coordinates are in millimetres.
fn circle_polygon(cx: f32, cy: f32, r: f32, mode: PaintMode) -> printpdf::Polygon {
    let k: f32 = 0.552_284_8; // 4/3 * (√2 − 1)
    let kr = k * r;

    // Points for four quarter-circle Bézier arcs (clockwise from top).
    let points = vec![
        // Start: top of circle
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx),
                y: mm_to_pt(cy + r),
            },
            bezier: false,
        },
        // Control points → right
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx + kr),
                y: mm_to_pt(cy + r),
            },
            bezier: true,
        },
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx + r),
                y: mm_to_pt(cy + kr),
            },
            bezier: true,
        },
        // Right
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx + r),
                y: mm_to_pt(cy),
            },
            bezier: false,
        },
        // Control points → bottom
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx + r),
                y: mm_to_pt(cy - kr),
            },
            bezier: true,
        },
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx + kr),
                y: mm_to_pt(cy - r),
            },
            bezier: true,
        },
        // Bottom
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx),
                y: mm_to_pt(cy - r),
            },
            bezier: false,
        },
        // Control points → left
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx - kr),
                y: mm_to_pt(cy - r),
            },
            bezier: true,
        },
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx - r),
                y: mm_to_pt(cy - kr),
            },
            bezier: true,
        },
        // Left
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx - r),
                y: mm_to_pt(cy),
            },
            bezier: false,
        },
        // Control points → top (close)
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx - r),
                y: mm_to_pt(cy + kr),
            },
            bezier: true,
        },
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx - kr),
                y: mm_to_pt(cy + r),
            },
            bezier: true,
        },
        // Close to start
        printpdf::LinePoint {
            p: Point {
                x: mm_to_pt(cx),
                y: mm_to_pt(cy + r),
            },
            bezier: false,
        },
    ];

    printpdf::Polygon {
        rings: vec![printpdf::PolygonRing { points }],
        mode,
        winding_order: WindingOrder::NonZero,
    }
}

/// Parse a simple SVG path data string into a `printpdf::Polygon`.
///
/// Supports a subset of SVG path commands: `M`, `L`, `Z` (absolute).
/// Returns `None` if the path string cannot be parsed.
fn parse_svg_path(
    d: &str,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    page_h: f32,
    mode: PaintMode,
) -> Option<printpdf::Polygon> {
    let mut points: Vec<printpdf::LinePoint> = Vec::new();
    let mut chars = d.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            'M' | 'L' => {
                chars.next();
                if let Some((x, y)) = parse_two_floats(&mut chars) {
                    let px = offset_x + x * scale;
                    let py = page_h - (offset_y + y * scale);
                    points.push(printpdf::LinePoint {
                        p: Point {
                            x: mm_to_pt(px),
                            y: mm_to_pt(py),
                        },
                        bezier: false,
                    });
                }
            }
            'Z' | 'z' => {
                chars.next();
                // Close — printpdf handles this via the polygon ring.
            }
            ' ' | ',' | '\n' | '\r' | '\t' => {
                chars.next();
            }
            _ => {
                // Skip unrecognised commands.
                chars.next();
            }
        }
    }

    if points.is_empty() {
        return None;
    }

    Some(printpdf::Polygon {
        rings: vec![printpdf::PolygonRing { points }],
        mode,
        winding_order: WindingOrder::NonZero,
    })
}

/// Parse two whitespace/comma-separated floats from a char iterator.
fn parse_two_floats(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<(f32, f32)> {
    let x = parse_float(chars)?;
    skip_ws_comma(chars);
    let y = parse_float(chars)?;
    skip_ws_comma(chars);
    Some((x, y))
}

fn parse_float(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<f32> {
    skip_ws_comma(chars);
    let mut s = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' {
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    s.parse::<f32>().ok()
}

fn skip_ws_comma(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(&ch) = chars.peek() {
        if ch == ' ' || ch == ',' || ch == '\t' || ch == '\n' || ch == '\r' {
            chars.next();
        } else {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Colour parsing -----------------------------------------------------

    #[test]
    fn test_parse_rgb_color() {
        let col = parse_css_color("rgb(255,0,0)");
        match col {
            Color::Rgb(rgb) => {
                assert!((rgb.r - 1.0).abs() < 0.01);
                assert!(rgb.g.abs() < 0.01);
                assert!(rgb.b.abs() < 0.01);
            }
            _ => panic!("Expected Rgb"),
        }
    }

    #[test]
    fn test_parse_rgba_color() {
        let col = parse_css_color("rgba(0,128,255,0.50)");
        match col {
            Color::Rgb(rgb) => {
                assert!(rgb.r.abs() < 0.01);
                assert!((rgb.g - 128.0 / 255.0).abs() < 0.01);
                assert!((rgb.b - 1.0).abs() < 0.01);
            }
            _ => panic!("Expected Rgb"),
        }
    }

    #[test]
    fn test_parse_hex_color() {
        let col = parse_css_color("#ff8000");
        match col {
            Color::Rgb(rgb) => {
                assert!((rgb.r - 1.0).abs() < 0.01);
                assert!((rgb.g - 128.0 / 255.0).abs() < 0.01);
                assert!(rgb.b.abs() < 0.01);
            }
            _ => panic!("Expected Rgb"),
        }
    }

    #[test]
    fn test_parse_named_color() {
        let col = parse_css_color("white");
        match col {
            Color::Rgb(rgb) => {
                assert!((rgb.r - 1.0).abs() < 0.01);
                assert!((rgb.g - 1.0).abs() < 0.01);
                assert!((rgb.b - 1.0).abs() < 0.01);
            }
            _ => panic!("Expected Rgb"),
        }
    }

    // -- Renderer -----------------------------------------------------------

    #[test]
    fn test_render_page_produces_ops() {
        let renderer = PdfRenderer::new(PdfOptions::a4());
        let elements = vec![SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            fill: "rgb(255,255,255)".to_string(),
            stroke: None,
            stroke_width: None,
            rx: None,
        }];
        let page = renderer.render_page(&elements, 800.0, 600.0);
        assert!(!page.ops.is_empty(), "Page should have operations");
    }

    #[test]
    fn test_render_circle() {
        let renderer = PdfRenderer::new(PdfOptions::a4());
        let elements = vec![SvgElement::Circle {
            cx: 100.0,
            cy: 100.0,
            r: 10.0,
            fill: "rgb(255,0,0)".to_string(),
            stroke: None,
            stroke_width: None,
        }];
        let page = renderer.render_page(&elements, 800.0, 600.0);
        // Should have SetFillColor + DrawPolygon (circle approximation).
        let has_polygon = page
            .ops
            .iter()
            .any(|op| matches!(op, Op::DrawPolygon { .. }));
        assert!(has_polygon, "Circle should generate a DrawPolygon op");
    }

    #[test]
    fn test_render_text() {
        let renderer = PdfRenderer::new(PdfOptions::a4());
        let elements = vec![SvgElement::Text {
            x: 50.0,
            y: 50.0,
            content: "Hello".to_string(),
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
            text_anchor: "middle".to_string(),
            dominant_baseline: "central".to_string(),
            fill: "rgb(0,0,0)".to_string(),
            font_weight: None,
        }];
        let page = renderer.render_page(&elements, 800.0, 600.0);
        let has_text = page.ops.iter().any(|op| matches!(op, Op::ShowText { .. }));
        assert!(has_text, "Text element should generate ShowText op");
    }

    #[test]
    fn test_render_line() {
        let renderer = PdfRenderer::new(PdfOptions::a4());
        let elements = vec![SvgElement::Line {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
            stroke: "rgb(0,0,0)".to_string(),
            stroke_width: 1.0,
            stroke_dasharray: None,
        }];
        let page = renderer.render_page(&elements, 800.0, 600.0);
        let has_line = page.ops.iter().any(|op| matches!(op, Op::DrawLine { .. }));
        assert!(has_line, "Line should generate DrawLine op");
    }

    #[test]
    fn test_render_group() {
        let renderer = PdfRenderer::new(PdfOptions::a4());
        let elements = vec![SvgElement::Group {
            class: Some("test".to_string()),
            transform: None,
            children: vec![SvgElement::Circle {
                cx: 50.0,
                cy: 50.0,
                r: 5.0,
                fill: "red".to_string(),
                stroke: None,
                stroke_width: None,
            }],
        }];
        let page = renderer.render_page(&elements, 800.0, 600.0);
        let has_save = page
            .ops
            .iter()
            .any(|op| matches!(op, Op::SaveGraphicsState));
        let has_restore = page
            .ops
            .iter()
            .any(|op| matches!(op, Op::RestoreGraphicsState));
        assert!(has_save, "Group should push SaveGraphicsState");
        assert!(has_restore, "Group should push RestoreGraphicsState");
    }

    // -- PdfDocument --------------------------------------------------------

    #[test]
    fn test_pdf_document_page_count() {
        let mut doc = PdfDocument::new(PdfOptions::a4());
        assert_eq!(doc.page_count(), 0);

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
        doc.add_page_from_elements("Page 1", &elems, 100.0, 100.0)
            .unwrap();
        assert_eq!(doc.page_count(), 1);

        doc.add_page_from_elements("Page 2", &elems, 100.0, 100.0)
            .unwrap();
        assert_eq!(doc.page_count(), 2);
    }

    #[test]
    fn test_pdf_document_to_bytes() {
        let mut doc = PdfDocument::new(PdfOptions::a4());
        let elems = vec![SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            fill: "rgb(255,255,255)".to_string(),
            stroke: None,
            stroke_width: None,
            rx: None,
        }];
        doc.add_page_from_elements("Test", &elems, 800.0, 600.0)
            .unwrap();

        let bytes = doc.to_bytes().unwrap();
        assert!(!bytes.is_empty());
        // PDF files start with "%PDF-".
        assert!(
            bytes.starts_with(b"%PDF-"),
            "Expected PDF header, got: {:?}",
            &bytes[..5.min(bytes.len())]
        );
    }

    #[test]
    fn test_pdf_document_write_file() {
        let dir = std::env::temp_dir().join("gup_pdf_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_output.pdf");

        let mut doc = PdfDocument::new(PdfOptions::letter());
        let elems = vec![SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            fill: "white".to_string(),
            stroke: None,
            stroke_width: None,
            rx: None,
        }];
        doc.add_page_from_elements("Test", &elems, 800.0, 600.0)
            .unwrap();
        doc.write(&path).unwrap();

        let content = std::fs::read(&path).unwrap();
        assert!(content.starts_with(b"%PDF-"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_pdf_document_multi_page_bytes() {
        let mut doc = PdfDocument::new(PdfOptions::a4());

        for i in 0..3 {
            let elems = vec![SvgElement::Circle {
                cx: 100.0 + i as f32 * 50.0,
                cy: 100.0,
                r: 20.0,
                fill: "rgb(0,0,255)".to_string(),
                stroke: None,
                stroke_width: None,
            }];
            doc.add_page_from_elements(&format!("Page {}", i + 1), &elems, 800.0, 600.0)
                .unwrap();
        }

        assert_eq!(doc.page_count(), 3);

        let bytes = doc.to_bytes().unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    // -- SVG path parsing ---------------------------------------------------

    #[test]
    fn test_parse_svg_path_simple() {
        let polygon = parse_svg_path(
            "M 0 0 L 100 0 L 100 50 Z",
            1.0,
            0.0,
            0.0,
            297.0,
            PaintMode::Fill,
        );
        assert!(polygon.is_some());
        let p = polygon.unwrap();
        assert_eq!(p.rings.len(), 1);
        assert_eq!(p.rings[0].points.len(), 3);
    }

    #[test]
    fn test_parse_svg_path_empty() {
        let polygon = parse_svg_path("", 1.0, 0.0, 0.0, 297.0, PaintMode::Fill);
        assert!(polygon.is_none());
    }

    // -- Render to bytes convenience ----------------------------------------

    #[test]
    fn test_render_to_bytes() {
        let renderer = PdfRenderer::new(PdfOptions::a4());
        let elements = vec![
            SvgElement::Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
                fill: "white".to_string(),
                stroke: None,
                stroke_width: None,
                rx: None,
            },
            SvgElement::Circle {
                cx: 400.0,
                cy: 300.0,
                r: 50.0,
                fill: "rgb(0,100,200)".to_string(),
                stroke: Some("rgb(0,0,0)".to_string()),
                stroke_width: Some(2.0),
            },
        ];
        let bytes = renderer.render_to_bytes(&elements, 800.0, 600.0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    // -- mm_to_pt conversion ------------------------------------------------

    #[test]
    fn test_mm_to_pt_conversion() {
        let pt = mm_to_pt(25.4); // 1 inch = 25.4 mm ≈ 72 pt
        assert!((pt.0 - 72.0).abs() < 0.1, "pt={}", pt.0);
    }
}
