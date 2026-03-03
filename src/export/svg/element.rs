// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightweight SVG element representation.
//!
//! [`SvgElement`] is a small enum that covers the SVG element types
//! needed by Gup's built-in marks: `<circle>`, `<rect>`, `<line>`,
//! `<path>`, `<text>`, and `<g>` (group).  Each variant carries the
//! minimal set of attributes required to faithfully reproduce the
//! visual output of the corresponding GPU mark.

/// A lightweight representation of an SVG element.
///
/// This enum describes the SVG primitives needed by Gup's built-in
/// mark types.  Instances are produced by [`super::SvgRenderer`]
/// during chart export and serialised into the final SVG document.
#[derive(Debug, Clone, PartialEq)]
pub enum SvgElement {
    /// An SVG `<circle>` element.
    Circle {
        /// Centre X coordinate (SVG viewport pixels).
        cx: f32,
        /// Centre Y coordinate (SVG viewport pixels).
        cy: f32,
        /// Radius in SVG viewport pixels.
        r: f32,
        /// Fill colour as a CSS colour string (e.g. `"rgba(255,0,0,1)"`).
        fill: String,
        /// Optional stroke colour.
        stroke: Option<String>,
        /// Optional stroke width.
        stroke_width: Option<f32>,
    },

    /// An SVG `<rect>` element.
    Rect {
        /// Top-left X coordinate.
        x: f32,
        /// Top-left Y coordinate.
        y: f32,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
        height: f32,
        /// Fill colour as a CSS colour string.
        fill: String,
        /// Optional stroke colour.
        stroke: Option<String>,
        /// Optional stroke width.
        stroke_width: Option<f32>,
        /// Optional corner radius for rounded rectangles.
        rx: Option<f32>,
    },

    /// An SVG `<line>` element.
    Line {
        /// Start X coordinate.
        x1: f32,
        /// Start Y coordinate.
        y1: f32,
        /// End X coordinate.
        x2: f32,
        /// End Y coordinate.
        y2: f32,
        /// Stroke colour as a CSS colour string.
        stroke: String,
        /// Stroke width in pixels.
        stroke_width: f32,
        /// Optional dash array (e.g. `"4 2"` for dashed lines).
        stroke_dasharray: Option<String>,
    },

    /// An SVG `<path>` element.
    Path {
        /// SVG path data string (e.g. `"M 0 0 L 100 100"`).
        d: String,
        /// Fill colour (use `"none"` for unfilled paths).
        fill: String,
        /// Optional stroke colour.
        stroke: Option<String>,
        /// Optional stroke width.
        stroke_width: Option<f32>,
    },

    /// An SVG `<text>` element.
    ///
    /// Text marks are exported as `<text>` elements (not path
    /// approximations) so that the output remains editable, searchable,
    /// and accessible.
    Text {
        /// X coordinate of the text anchor point.
        x: f32,
        /// Y coordinate of the text anchor point.
        y: f32,
        /// The text content.
        content: String,
        /// CSS `font-family` value.
        font_family: String,
        /// Font size in pixels.
        font_size: f32,
        /// SVG `text-anchor` attribute (`"start"`, `"middle"`, or `"end"`).
        text_anchor: String,
        /// SVG `dominant-baseline` attribute.
        dominant_baseline: String,
        /// Fill colour as a CSS colour string.
        fill: String,
        /// Optional font weight (e.g. `"bold"`, `"normal"`).
        font_weight: Option<String>,
    },

    /// An SVG `<g>` (group) element that contains child elements.
    Group {
        /// Optional CSS class name for the group.
        class: Option<String>,
        /// Optional `transform` attribute value.
        transform: Option<String>,
        /// Child elements within the group.
        children: Vec<SvgElement>,
    },
}

impl SvgElement {
    /// Serialise this element to an SVG string fragment.
    ///
    /// The returned string is a well-formed SVG element (or tree of
    /// elements for [`SvgElement::Group`]).  Indentation is controlled
    /// by `indent_level` (each level adds two spaces).
    pub fn to_svg_string(&self, indent_level: usize) -> String {
        let indent = "  ".repeat(indent_level);
        match self {
            SvgElement::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
                stroke_width,
            } => {
                let mut attrs =
                    format!(r#"{indent}<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}""#,);
                if let Some(s) = stroke {
                    attrs.push_str(&format!(r#" stroke="{s}""#));
                }
                if let Some(sw) = stroke_width {
                    attrs.push_str(&format!(r#" stroke-width="{sw}""#));
                }
                attrs.push_str("/>");
                attrs
            }
            SvgElement::Rect {
                x,
                y,
                width,
                height,
                fill,
                stroke,
                stroke_width,
                rx,
            } => {
                let mut attrs = format!(
                    r#"{indent}<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{fill}""#,
                );
                if let Some(r) = rx {
                    attrs.push_str(&format!(r#" rx="{r}""#));
                }
                if let Some(s) = stroke {
                    attrs.push_str(&format!(r#" stroke="{s}""#));
                }
                if let Some(sw) = stroke_width {
                    attrs.push_str(&format!(r#" stroke-width="{sw}""#));
                }
                attrs.push_str("/>");
                attrs
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
                let mut attrs = format!(
                    r#"{indent}<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{stroke_width}""#,
                );
                if let Some(da) = stroke_dasharray {
                    attrs.push_str(&format!(r#" stroke-dasharray="{da}""#));
                }
                attrs.push_str("/>");
                attrs
            }
            SvgElement::Path {
                d,
                fill,
                stroke,
                stroke_width,
            } => {
                let mut attrs = format!(r#"{indent}<path d="{d}" fill="{fill}""#,);
                if let Some(s) = stroke {
                    attrs.push_str(&format!(r#" stroke="{s}""#));
                }
                if let Some(sw) = stroke_width {
                    attrs.push_str(&format!(r#" stroke-width="{sw}""#));
                }
                attrs.push_str("/>");
                attrs
            }
            SvgElement::Text {
                x,
                y,
                content,
                font_family,
                font_size,
                text_anchor,
                dominant_baseline,
                fill,
                font_weight,
            } => {
                let mut attrs = format!(
                    r#"{indent}<text x="{x}" y="{y}" font-family="{font_family}" font-size="{font_size}" text-anchor="{text_anchor}" dominant-baseline="{dominant_baseline}" fill="{fill}""#,
                );
                if let Some(fw) = font_weight {
                    attrs.push_str(&format!(r#" font-weight="{fw}""#));
                }
                attrs.push_str(&format!(">{}</text>", xml_escape(content)));
                attrs
            }
            SvgElement::Group {
                class,
                transform,
                children,
            } => {
                let mut s = format!("{indent}<g");
                if let Some(c) = class {
                    s.push_str(&format!(r#" class="{c}""#));
                }
                if let Some(t) = transform {
                    s.push_str(&format!(r#" transform="{t}""#));
                }
                s.push('>');
                for child in children {
                    s.push('\n');
                    s.push_str(&child.to_svg_string(indent_level + 1));
                }
                s.push('\n');
                s.push_str(&indent);
                s.push_str("</g>");
                s
            }
        }
    }
}

/// Escape XML special characters in text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Format an RGBA colour (each component 0.0–1.0) as a CSS `rgba(…)` string.
pub(crate) fn rgba_to_css(r: f32, g: f32, b: f32, a: f32) -> String {
    let ri = (r * 255.0).round() as u8;
    let gi = (g * 255.0).round() as u8;
    let bi = (b * 255.0).round() as u8;
    if (a - 1.0).abs() < f32::EPSILON {
        format!("rgb({ri},{gi},{bi})")
    } else {
        format!("rgba({ri},{gi},{bi},{a:.2})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_svg() {
        let elem = SvgElement::Circle {
            cx: 100.0,
            cy: 200.0,
            r: 5.0,
            fill: "rgb(255,0,0)".to_string(),
            stroke: None,
            stroke_width: None,
        };
        let svg = elem.to_svg_string(0);
        assert!(svg.contains("cx=\"100\""));
        assert!(svg.contains("cy=\"200\""));
        assert!(svg.contains("r=\"5\""));
        assert!(svg.contains("fill=\"rgb(255,0,0)\""));
        assert!(svg.starts_with("<circle"));
        assert!(svg.ends_with("/>"));
    }

    #[test]
    fn test_rect_svg() {
        let elem = SvgElement::Rect {
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 80.0,
            fill: "rgb(0,128,255)".to_string(),
            stroke: Some("rgb(0,0,0)".to_string()),
            stroke_width: Some(1.0),
            rx: Some(3.0),
        };
        let svg = elem.to_svg_string(0);
        assert!(svg.contains("x=\"10\""));
        assert!(svg.contains("width=\"50\""));
        assert!(svg.contains("rx=\"3\""));
        assert!(svg.contains("stroke=\"rgb(0,0,0)\""));
    }

    #[test]
    fn test_line_svg() {
        let elem = SvgElement::Line {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
            stroke: "rgb(0,0,0)".to_string(),
            stroke_width: 1.0,
            stroke_dasharray: Some("4 2".to_string()),
        };
        let svg = elem.to_svg_string(0);
        assert!(svg.contains("x1=\"0\""));
        assert!(svg.contains("x2=\"100\""));
        assert!(svg.contains("stroke-dasharray=\"4 2\""));
    }

    #[test]
    fn test_text_svg() {
        let elem = SvgElement::Text {
            x: 50.0,
            y: 50.0,
            content: "Hello & <World>".to_string(),
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
            text_anchor: "middle".to_string(),
            dominant_baseline: "central".to_string(),
            fill: "rgb(0,0,0)".to_string(),
            font_weight: Some("bold".to_string()),
        };
        let svg = elem.to_svg_string(0);
        assert!(svg.contains("font-family=\"sans-serif\""));
        assert!(svg.contains("font-size=\"14\""));
        assert!(svg.contains("font-weight=\"bold\""));
        // Verify XML escaping
        assert!(svg.contains("Hello &amp; &lt;World&gt;"));
        assert!(!svg.contains("Hello & <World>"));
    }

    #[test]
    fn test_group_svg() {
        let group = SvgElement::Group {
            class: Some("marks".to_string()),
            transform: None,
            children: vec![SvgElement::Circle {
                cx: 10.0,
                cy: 20.0,
                r: 3.0,
                fill: "red".to_string(),
                stroke: None,
                stroke_width: None,
            }],
        };
        let svg = group.to_svg_string(0);
        assert!(svg.contains("<g class=\"marks\">"));
        assert!(svg.contains("</g>"));
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn test_path_svg() {
        let elem = SvgElement::Path {
            d: "M 0 0 L 100 50 L 100 100 Z".to_string(),
            fill: "rgba(0,128,255,0.50)".to_string(),
            stroke: Some("rgb(0,0,255)".to_string()),
            stroke_width: Some(2.0),
        };
        let svg = elem.to_svg_string(0);
        assert!(svg.contains("d=\"M 0 0 L 100 50 L 100 100 Z\""));
        assert!(svg.contains("fill=\"rgba(0,128,255,0.50)\""));
    }

    #[test]
    fn test_rgba_to_css_opaque() {
        let css = rgba_to_css(1.0, 0.0, 0.0, 1.0);
        assert_eq!(css, "rgb(255,0,0)");
    }

    #[test]
    fn test_rgba_to_css_translucent() {
        let css = rgba_to_css(0.0, 0.5, 1.0, 0.75);
        assert_eq!(css, "rgba(0,128,255,0.75)");
    }
}
