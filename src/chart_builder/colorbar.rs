// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Colorbar legend renderer for colour-scale visualisations.
//!
//! Provides [`ColorbarRenderer`] which draws a thin gradient-filled strip
//! adjacent to the plot area with tick marks and numeric labels.  The
//! gradient uses the same [`ColorScale`](crate::shader_function::ColorScale)
//! as the chart cells, ensuring visual consistency.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use gup::chart_builder::colorbar::{ColorbarRenderer, ColorbarConfig, ColorbarOrientation};
//! use gup::shader_function::ColorScale;
//!
//! let color_scale = ColorScale::viridis(0.0, 100.0);
//! let config = ColorbarConfig::default();
//! let renderer = ColorbarRenderer::new(color_scale, config);
//!
//! // Generate geometry for a 600-px tall chart area at x = 0.85 in NDC
//! let geom = renderer.generate_geometry(
//!     0.85,  // x_ndc: right edge of the colourbar strip
//!     -0.8,  // y_min_ndc: bottom of chart area
//!     0.8,   // y_max_ndc: top of chart area
//!     (800.0, 600.0),
//! );
//! ```

use crate::axis::{
    AxisBounds, AxisConfiguration, AxisLabel, AxisPosition, AxisRenderer, TickInstance,
};
use crate::render::Vertex;
use crate::shader_function::{ColorScale, Vec2};
use crate::tick_generator;

// ── Configuration ────────────────────────────────────────────────────────

/// Orientation of the colorbar strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ColorbarOrientation {
    /// Vertical strip (default) — placed to the right of the chart.
    #[default]
    Vertical,
    /// Horizontal strip — placed below the chart.
    Horizontal,
}


/// Configuration for the colorbar legend.
#[derive(Debug, Clone)]
pub struct ColorbarConfig {
    /// Orientation of the gradient strip.
    pub orientation: ColorbarOrientation,
    /// Width of the gradient strip in NDC units.
    pub strip_width_ndc: f32,
    /// Number of discrete segments used to approximate the gradient.
    pub segment_count: usize,
    /// Target number of tick marks along the colorbar.
    pub tick_count: Option<usize>,
    /// Axis configuration for tick marks and labels.
    pub axis_config: AxisConfiguration,
}

impl Default for ColorbarConfig {
    fn default() -> Self {
        Self {
            orientation: ColorbarOrientation::Vertical,
            strip_width_ndc: 0.03,
            segment_count: 64,
            tick_count: Some(5),
            axis_config: AxisConfiguration {
                show_line: false,
                show_major_ticks: true,
                show_minor_ticks: false,
                major_tick_length: 4.0,
                minor_tick_length: 2.0,
                line_color: [0.2, 0.2, 0.2, 1.0],
                line_width: 1.0,
                target_tick_count: Some(5),
                minor_tick_subdivisions: 5,
                label_style: None,
            },
        }
    }
}

// ── Output ───────────────────────────────────────────────────────────────

/// Geometry produced by [`ColorbarRenderer::generate_geometry`].
#[derive(Debug, Clone)]
pub struct ColorbarGeometry {
    /// Triangle-list vertices for the gradient strip (position + colour).
    pub gradient_vertices: Vec<Vertex>,
    /// Tick instance data (same format as axis ticks).
    pub tick_instances: Vec<TickInstance>,
    /// Axis-line vertices for the colorbar outline (LineList pairs).
    pub line_vertices: Vec<Vertex>,
    /// Labels for tick values.
    pub labels: Vec<AxisLabel>,
}

// ── Renderer ─────────────────────────────────────────────────────────────

/// Renders a colour-scale legend (colorbar) as a gradient strip with ticks
/// and labels.
///
/// `ColorbarRenderer` is intentionally decoupled from `ComposedChart` so
/// that it can be reused by any chart type that exposes a `ColorScale`.
#[derive(Debug, Clone)]
pub struct ColorbarRenderer {
    /// The colour scale used for the gradient.
    color_scale: ColorScale,
    /// Configuration controlling appearance and layout.
    config: ColorbarConfig,
}

impl ColorbarRenderer {
    /// Create a new colorbar renderer.
    pub fn new(color_scale: ColorScale, config: ColorbarConfig) -> Self {
        Self {
            color_scale,
            config,
        }
    }

    /// Create a colorbar renderer with default configuration.
    pub fn with_defaults(color_scale: ColorScale) -> Self {
        Self::new(color_scale, ColorbarConfig::default())
    }

    /// Reference to the underlying colour scale.
    pub fn color_scale(&self) -> &ColorScale {
        &self.color_scale
    }

    /// Reference to the configuration.
    pub fn config(&self) -> &ColorbarConfig {
        &self.config
    }

    /// Generate all colorbar geometry in NDC coordinates.
    ///
    /// # Parameters
    ///
    /// * `x_ndc` — The left edge of the colourbar strip in NDC.
    /// * `y_min_ndc` — Bottom of the strip (NDC, typically negative).
    /// * `y_max_ndc` — Top of the strip (NDC, typically positive).
    /// * `viewport_size` — `(width, height)` of the viewport in pixels.
    ///
    /// For horizontal orientation the parameters are reinterpreted as
    /// `(y_ndc, x_min_ndc, x_max_ndc)`.
    pub fn generate_geometry(
        &self,
        x_ndc: f32,
        y_min_ndc: f32,
        y_max_ndc: f32,
        viewport_size: (f32, f32),
    ) -> ColorbarGeometry {
        let gradient_vertices = self.generate_gradient_strip(x_ndc, y_min_ndc, y_max_ndc);
        let line_vertices = self.generate_outline(x_ndc, y_min_ndc, y_max_ndc);

        // Build a tick scale from the color scale domain.
        let tick_scale = tick_generator::LinearScale::new(
            self.color_scale.domain_min as f64,
            self.color_scale.domain_max as f64,
        );

        // The axis runs along the right edge of the strip (for vertical)
        // from bottom to top (start = bottom, end = top so ticks point right).
        let strip_right = x_ndc + self.config.strip_width_ndc;
        let axis_bounds = AxisBounds::new(
            Vec2 {
                x: strip_right,
                y: y_min_ndc,
            },
            Vec2 {
                x: strip_right,
                y: y_max_ndc,
            },
            20.0, // margin for labels
        );

        let renderer = AxisRenderer::new();

        let mut axis_config = self.config.axis_config.clone();
        axis_config.target_tick_count = self.config.tick_count;

        let tick_instances = renderer.generate_tick_instances(
            &axis_bounds,
            &axis_config,
            AxisPosition::Right,
            Some(&tick_scale),
            viewport_size,
        );

        let labels = renderer.generate_label_data(
            &axis_bounds,
            &axis_config,
            AxisPosition::Right,
            Some(&tick_scale),
            viewport_size,
            None, // default NumericFormatter
        );

        ColorbarGeometry {
            gradient_vertices,
            tick_instances,
            line_vertices,
            labels,
        }
    }

    /// Generate triangle-list vertices for the gradient strip.
    ///
    /// The strip is split into `segment_count` horizontal bands. Each band
    /// is a quad (two triangles) coloured by linearly sampling the gradient
    /// at the band's top and bottom edges.
    fn generate_gradient_strip(&self, x_ndc: f32, y_min_ndc: f32, y_max_ndc: f32) -> Vec<Vertex> {
        let n = self.config.segment_count.max(1);
        let strip_w = self.config.strip_width_ndc;
        let left = x_ndc;
        let right = x_ndc + strip_w;
        let height = y_max_ndc - y_min_ndc;

        let mut vertices = Vec::with_capacity(n * 6);

        for i in 0..n {
            // Normalized parameter t ∈ [0, 1] from bottom to top.
            let t_bot = i as f32 / n as f32;
            let t_top = (i + 1) as f32 / n as f32;

            let y_bot = y_min_ndc + t_bot * height;
            let y_top = y_min_ndc + t_top * height;

            let c_bot = self.sample_color(t_bot);
            let c_top = self.sample_color(t_top);

            // Two triangles forming a quad:
            //  top-left --- top-right
            //  |          / |
            //  |        /   |
            //  bot-left --- bot-right

            // Triangle 1: bot-left, bot-right, top-left
            vertices.push(Vertex {
                position: [left, y_bot],
                color: c_bot,
            });
            vertices.push(Vertex {
                position: [right, y_bot],
                color: c_bot,
            });
            vertices.push(Vertex {
                position: [left, y_top],
                color: c_top,
            });

            // Triangle 2: bot-right, top-right, top-left
            vertices.push(Vertex {
                position: [right, y_bot],
                color: c_bot,
            });
            vertices.push(Vertex {
                position: [right, y_top],
                color: c_top,
            });
            vertices.push(Vertex {
                position: [left, y_top],
                color: c_top,
            });
        }

        vertices
    }

    /// Generate outline line vertices around the gradient strip.
    fn generate_outline(&self, x_ndc: f32, y_min_ndc: f32, y_max_ndc: f32) -> Vec<Vertex> {
        let strip_w = self.config.strip_width_ndc;
        let left = x_ndc;
        let right = x_ndc + strip_w;
        let color = self.config.axis_config.line_color;

        // Four edges as LineList pairs (8 vertices).
        vec![
            // Left edge
            Vertex {
                position: [left, y_min_ndc],
                color,
            },
            Vertex {
                position: [left, y_max_ndc],
                color,
            },
            // Right edge
            Vertex {
                position: [right, y_min_ndc],
                color,
            },
            Vertex {
                position: [right, y_max_ndc],
                color,
            },
            // Bottom edge
            Vertex {
                position: [left, y_min_ndc],
                color,
            },
            Vertex {
                position: [right, y_min_ndc],
                color,
            },
            // Top edge
            Vertex {
                position: [left, y_max_ndc],
                color,
            },
            Vertex {
                position: [right, y_max_ndc],
                color,
            },
        ]
    }

    /// Sample the gradient colour at a normalized position `t ∈ [0, 1]`.
    ///
    /// Performs CPU-side linear interpolation over the colour stops,
    /// matching the GPU shader's behaviour.
    fn sample_color(&self, t: f32) -> [f32; 4] {
        let t = t.clamp(0.0, 1.0);
        let stops = &self.color_scale.gradient.stops;
        let colors = &self.color_scale.gradient.colors;

        if colors.is_empty() {
            return [0.0, 0.0, 0.0, 1.0];
        }
        if colors.len() == 1 || t <= stops[0] {
            let c = &colors[0];
            return [c.x, c.y, c.z, c.w];
        }
        if t >= *stops.last().unwrap() {
            let c = colors.last().unwrap();
            return [c.x, c.y, c.z, c.w];
        }

        // Binary search for the surrounding stops.
        for i in 0..stops.len() - 1 {
            if t >= stops[i] && t <= stops[i + 1] {
                let range = stops[i + 1] - stops[i];
                let frac = if range > 0.0 {
                    (t - stops[i]) / range
                } else {
                    0.0
                };
                let a = &colors[i];
                let b = &colors[i + 1];
                return [
                    a.x + (b.x - a.x) * frac,
                    a.y + (b.y - a.y) * frac,
                    a.z + (b.z - a.z) * frac,
                    a.w + (b.w - a.w) * frac,
                ];
            }
        }

        // Fallback (should not reach here with valid stops).
        let c = colors.last().unwrap();
        [c.x, c.y, c.z, c.w]
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::ColorScale;

    #[test]
    fn test_default_config() {
        let config = ColorbarConfig::default();
        assert_eq!(config.orientation, ColorbarOrientation::Vertical);
        assert_eq!(config.segment_count, 64);
        assert_eq!(config.tick_count, Some(5));
        assert!(!config.axis_config.show_line);
        assert!(config.axis_config.show_major_ticks);
    }

    #[test]
    fn test_sample_color_endpoints() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let renderer = ColorbarRenderer::with_defaults(scale);

        let c0 = renderer.sample_color(0.0);
        let c1 = renderer.sample_color(1.0);

        // Viridis starts dark purple (~0.27, 0.00, 0.33) and ends yellow
        assert!(c0[0] < 0.4, "start R should be low: {}", c0[0]);
        assert!(c1[0] > 0.8, "end R should be high: {}", c1[0]);
    }

    #[test]
    fn test_sample_color_midpoint() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let renderer = ColorbarRenderer::with_defaults(scale);

        let c_mid = renderer.sample_color(0.5);
        // Viridis midpoint is a teal-ish green
        assert!(c_mid[1] > 0.3, "mid G should be > 0.3: {}", c_mid[1]);
    }

    #[test]
    fn test_sample_color_clamping() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let renderer = ColorbarRenderer::with_defaults(scale);

        // Out-of-range values should clamp
        let c_neg = renderer.sample_color(-0.5);
        let c_zero = renderer.sample_color(0.0);
        assert_eq!(c_neg, c_zero);

        let c_over = renderer.sample_color(1.5);
        let c_one = renderer.sample_color(1.0);
        assert_eq!(c_over, c_one);
    }

    #[test]
    fn test_gradient_strip_vertex_count() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let config = ColorbarConfig {
            segment_count: 10,
            ..Default::default()
        };
        let renderer = ColorbarRenderer::new(scale, config);

        let verts = renderer.generate_gradient_strip(-0.8, -0.8, 0.8);
        // 10 segments × 6 vertices per quad = 60
        assert_eq!(verts.len(), 60);
    }

    #[test]
    fn test_gradient_strip_spans_range() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let config = ColorbarConfig {
            segment_count: 4,
            strip_width_ndc: 0.05,
            ..Default::default()
        };
        let renderer = ColorbarRenderer::new(scale, config);

        let verts = renderer.generate_gradient_strip(0.8, -0.7, 0.7);

        // Check first triangle bottom-left vertex
        assert!((verts[0].position[0] - 0.8).abs() < 1e-5);
        assert!((verts[0].position[1] - (-0.7)).abs() < 1e-5);

        // Check last triangle top-right vertex (index 4 in last quad)
        let last_quad_start = (4 - 1) * 6; // quad index 3, first vertex
        let top_right = &verts[last_quad_start + 4]; // top-right of last quad
        assert!((top_right.position[0] - 0.85).abs() < 1e-5);
        assert!((top_right.position[1] - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_outline_vertices() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let renderer = ColorbarRenderer::with_defaults(scale);

        let outline = renderer.generate_outline(0.8, -0.7, 0.7);
        // 4 edges × 2 vertices = 8
        assert_eq!(outline.len(), 8);
    }

    #[test]
    fn test_generate_geometry_produces_all_parts() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let renderer = ColorbarRenderer::with_defaults(scale);

        let geom = renderer.generate_geometry(0.85, -0.8, 0.8, (800.0, 600.0));

        // Gradient strip
        assert!(!geom.gradient_vertices.is_empty());
        assert_eq!(geom.gradient_vertices.len() % 6, 0); // triangles

        // Outline
        assert_eq!(geom.line_vertices.len(), 8);

        // Ticks and labels should be generated
        assert!(!geom.tick_instances.is_empty());
        assert!(!geom.labels.is_empty());
    }

    #[test]
    fn test_custom_tick_count() {
        let scale = ColorScale::viridis(0.0, 100.0);
        let config = ColorbarConfig {
            tick_count: Some(3),
            ..Default::default()
        };
        let renderer = ColorbarRenderer::new(scale, config);

        let geom = renderer.generate_geometry(0.85, -0.8, 0.8, (800.0, 600.0));

        // Should have approximately 3 ticks (generator may choose slightly different count)
        assert!(
            geom.tick_instances.len() <= 6,
            "Expected ≤6 ticks, got {}",
            geom.tick_instances.len()
        );
        assert!(!geom.tick_instances.is_empty(), "Expected at least 1 tick");
    }

    #[test]
    fn test_different_palettes() {
        for make_scale in [
            ColorScale::viridis as fn(f32, f32) -> ColorScale,
            ColorScale::plasma,
            ColorScale::inferno,
            ColorScale::magma,
        ] {
            let scale = make_scale(0.0, 50.0);
            let renderer = ColorbarRenderer::with_defaults(scale);
            let geom = renderer.generate_geometry(0.8, -0.7, 0.7, (800.0, 600.0));
            assert!(
                !geom.gradient_vertices.is_empty(),
                "gradient should have vertices"
            );
        }
    }

    #[test]
    fn test_domain_range_affects_labels() {
        let scale_small = ColorScale::viridis(0.0, 10.0);
        let scale_large = ColorScale::viridis(0.0, 1000.0);

        let renderer_small = ColorbarRenderer::with_defaults(scale_small);
        let renderer_large = ColorbarRenderer::with_defaults(scale_large);

        let geom_small = renderer_small.generate_geometry(0.85, -0.8, 0.8, (800.0, 600.0));
        let geom_large = renderer_large.generate_geometry(0.85, -0.8, 0.8, (800.0, 600.0));

        // Both should produce labels
        assert!(!geom_small.labels.is_empty());
        assert!(!geom_large.labels.is_empty());

        // Labels should contain different formatted values
        let small_texts: Vec<&str> = geom_small.labels.iter().map(|l| l.text.as_str()).collect();
        let large_texts: Vec<&str> = geom_large.labels.iter().map(|l| l.text.as_str()).collect();
        assert_ne!(small_texts, large_texts);
    }
}
