// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box plot mark implementation for statistical distribution visualization.
//!
//! The BoxPlot mark provides GPU-accelerated rendering of statistical distributions
//! using the five-number summary (min, Q1, median, Q3, max) plus outliers. It integrates
//! with the statistical shader functions from GUP-139 for efficient quartile calculation.

use crate::mark::Mark;
use crate::shader_function::{MinMax, Percentile, Vec2, Vec4};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// Box plot mark for rendering statistical distribution summaries.
///
/// This mark visualizes data distributions using:
/// - Box representing interquartile range (Q1-Q3)
/// - Median line at the 50th percentile
/// - Whiskers extending to min/max or 1.5×IQR
/// - Outlier points beyond whiskers
///
/// # Examples
///
/// ```rust
/// use gup::mark::{BoxPlot, BoxPlotAttributes, BoxPlotOrientation, Mark};
/// use gup::{vec2, vec4, Vec2, Vec4};
///
/// // Create box plot attributes
/// let attrs = BoxPlotAttributes {
///     position: vec2![100.0, 200.0],
///     min: 10.0,
///     q1: 25.0,
///     median: 50.0,
///     q3: 75.0,
///     max: 100.0,
///     outliers: vec![5.0, 105.0],
///     width: 40.0,
///     orientation: BoxPlotOrientation::Vertical,
///     box_fill_color: vec4![0.7, 0.7, 1.0, 0.8],
///     box_stroke_color: vec4![0.0, 0.0, 0.0, 1.0],
///     median_color: vec4![1.0, 0.0, 0.0, 1.0],
///     whisker_color: vec4![0.0, 0.0, 0.0, 1.0],
///     outlier_color: vec4![1.0, 0.5, 0.0, 1.0],
///     stroke_width: 1.0,
///     outlier_radius: 3.0,
///     notched: false,
///     notch_width: 0.5,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct BoxPlot;

/// Orientation of the box plot
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxPlotOrientation {
    /// Box extends vertically (value axis is vertical)
    Vertical,
    /// Box extends horizontally (value axis is horizontal)
    Horizontal,
}

/// GPU vertex data for box plot rendering.
///
/// Each vertex represents a component of the box plot (box, median, whisker, outlier).
/// The actual rendering is done using instanced quads and lines.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoxPlotVertex {
    /// Local position within the unit quad
    pub position: [f32; 2],
}

/// High-level attributes for configuring box plot appearance.
///
/// These attributes define the statistical values and visual properties of the box plot.
#[derive(Debug, Clone)]
pub struct BoxPlotAttributes {
    /// Position of the box plot center
    pub position: Vec2,
    /// Minimum value (lower whisker end, or minimum non-outlier)
    pub min: f32,
    /// First quartile (25th percentile, bottom of box)
    pub q1: f32,
    /// Median (50th percentile, middle line)
    pub median: f32,
    /// Third quartile (75th percentile, top of box)
    pub q3: f32,
    /// Maximum value (upper whisker end, or maximum non-outlier)
    pub max: f32,
    /// Outlier values beyond whiskers
    pub outliers: Vec<f32>,
    /// Width of the box
    pub width: f32,
    /// Orientation (vertical or horizontal)
    pub orientation: BoxPlotOrientation,
    /// Fill color for the box (IQR)
    pub box_fill_color: Vec4,
    /// Stroke color for the box outline
    pub box_stroke_color: Vec4,
    /// Color for the median line
    pub median_color: Vec4,
    /// Color for the whisker lines
    pub whisker_color: Vec4,
    /// Color for outlier points
    pub outlier_color: Vec4,
    /// Stroke width for box and whiskers
    pub stroke_width: f32,
    /// Radius of outlier circles
    pub outlier_radius: f32,
    /// Whether to draw notched box plot (shows confidence interval)
    pub notched: bool,
    /// Width of notch (as fraction of box width, typically 0.5)
    pub notch_width: f32,
}

impl BoxPlotAttributes {
    /// Compute box plot statistics from raw data values.
    ///
    /// Uses Percentile and MinMax from GUP-139 to calculate quartiles.
    pub fn from_data(
        values: &[f32],
        position: Vec2,
        width: f32,
        orientation: BoxPlotOrientation,
    ) -> Self {
        let min_max = MinMax::new(values.to_vec());
        let (min, max) = min_max.compute_cpu();

        let q1_calc = Percentile::new(values.to_vec(), 0.25);
        let median_calc = Percentile::new(values.to_vec(), 0.5);
        let q3_calc = Percentile::new(values.to_vec(), 0.75);

        let q1 = q1_calc.compute_cpu();
        let median = median_calc.compute_cpu();
        let q3 = q3_calc.compute_cpu();

        // Calculate IQR and whisker boundaries
        let iqr = q3 - q1;
        let lower_fence = q1 - 1.5 * iqr;
        let upper_fence = q3 + 1.5 * iqr;

        // Find whisker extents (min/max within fences) and outliers
        let mut whisker_min = max;
        let mut whisker_max = min;
        let mut outliers = Vec::new();

        for &value in values {
            if value < lower_fence || value > upper_fence {
                outliers.push(value);
            } else {
                whisker_min = whisker_min.min(value);
                whisker_max = whisker_max.max(value);
            }
        }

        Self {
            position,
            min: whisker_min,
            q1,
            median,
            q3,
            max: whisker_max,
            outliers,
            width,
            orientation,
            box_fill_color: Vec4 {
                x: 0.7,
                y: 0.7,
                z: 1.0,
                w: 0.8,
            },
            box_stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            median_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            whisker_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            outlier_color: Vec4 {
                x: 1.0,
                y: 0.5,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 1.0,
            outlier_radius: 3.0,
            notched: false,
            notch_width: 0.5,
        }
    }

    /// Get the interquartile range (IQR)
    pub fn iqr(&self) -> f32 {
        self.q3 - self.q1
    }

    /// Check if a value is an outlier
    pub fn is_outlier(&self, value: f32) -> bool {
        let iqr = self.iqr();
        let lower_fence = self.q1 - 1.5 * iqr;
        let upper_fence = self.q3 + 1.5 * iqr;
        value < lower_fence || value > upper_fence
    }
}

impl Mark for BoxPlot {
    type Vertex = BoxPlotVertex;
    type AttributeValue = BoxPlotAttributes;

    /// Hand-optimized vertex shader for box plots.
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/boxplot.vert.wgsl"));

    /// Hand-optimized fragment shader for box plots.
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/boxplot.frag.wgsl"));

    /// Pattern-enabled fragment shader for box plots.
    ///
    /// Integrates pattern rendering for accessibility support while maintaining
    /// all standard box plot features (stroke, anti-aliasing).
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/boxplot_pattern.frag.wgsl"));

    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_vertex_shader_with_functions(pipeline, &HashMap::new())
    }

    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str("// Generated BoxPlot vertex shader\n\n");

        // Data structures
        shader.push_str("struct BoxPlotInstance {\n");
        shader.push_str("    position: vec2<f32>,\n");
        shader.push_str("    min: f32,\n");
        shader.push_str("    q1: f32,\n");
        shader.push_str("    median: f32,\n");
        shader.push_str("    q3: f32,\n");
        shader.push_str("    max: f32,\n");
        shader.push_str("    width: f32,\n");
        shader.push_str("    orientation: u32,\n");
        shader.push_str("    box_fill_color: vec4<f32>,\n");
        shader.push_str("    box_stroke_color: vec4<f32>,\n");
        shader.push_str("    median_color: vec4<f32>,\n");
        shader.push_str("    whisker_color: vec4<f32>,\n");
        shader.push_str("    stroke_width: f32,\n");
        shader.push_str("    notched: u32,\n");
        shader.push_str("    notch_width: f32,\n");
        shader.push_str("}\n\n");

        shader.push_str(
            "@group(1) @binding(0) var<storage, read> instances: array<BoxPlotInstance>;\n\n",
        );

        shader.push_str("struct VertexInput {\n");
        shader.push_str("    @location(0) position: vec2<f32>,\n");
        shader.push_str("    @builtin(instance_index) instance_index: u32,\n");
        shader.push_str("}\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) box_fill_color: vec4<f32>,\n");
        shader.push_str("}\n\n");

        // Add shader function definitions
        let pipeline_functions = pipeline.generate_vertex_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        shader.push_str("@vertex\n");
        shader.push_str("fn vs_main(input: VertexInput) -> VertexOutput {\n");
        shader.push_str("    let instance = instances[input.instance_index];\n");
        shader.push_str("    var output: VertexOutput;\n");
        shader.push_str("    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);\n");
        shader.push_str("    output.world_position = input.position;\n");
        shader.push_str("    output.box_fill_color = instance.box_fill_color;\n");
        shader.push_str("    return output;\n");
        shader.push_str("}\n");

        shader
    }

    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    fn generate_fragment_shader_with_functions(
        _pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str("// Generated BoxPlot fragment shader\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) box_fill_color: vec4<f32>,\n");
        shader.push_str("}\n\n");

        shader.push_str("@fragment\n");
        shader.push_str("fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n");
        shader.push_str("    return input.box_fill_color;\n");
        shader.push_str("}\n");

        shader
    }

    fn vertex_count() -> usize {
        4 // Unit quad for instanced rendering
    }

    fn index_count() -> Option<usize> {
        Some(6) // Two triangles
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            BoxPlotVertex {
                position: [-0.5, -0.5],
            },
            BoxPlotVertex {
                position: [0.5, -0.5],
            },
            BoxPlotVertex {
                position: [0.5, 0.5],
            },
            BoxPlotVertex {
                position: [-0.5, 0.5],
            },
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "position" => Ok("vec2<f32>"),
            "min" | "q1" | "median" | "q3" | "max" | "width" | "stroke_width"
            | "outlier_radius" | "notch_width" => Ok("f32"),
            "box_fill_color" | "box_stroke_color" | "median_color" | "whisker_color"
            | "outlier_color" => Ok("vec4<f32>"),
            "orientation" => Ok("u32"),
            "notched" => Ok("bool"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown BoxPlot attribute: {attribute_name}"
            ))),
        }
    }

    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        match Self::get_attribute_type(attribute_name) {
            Ok(expected_type) => expected_type == output_type,
            Err(_) => false,
        }
    }
}

impl Default for BoxPlotAttributes {
    fn default() -> Self {
        Self {
            position: Vec2 { x: 0.0, y: 0.0 },
            min: 0.0,
            q1: 25.0,
            median: 50.0,
            q3: 75.0,
            max: 100.0,
            outliers: Vec::new(),
            width: 40.0,
            orientation: BoxPlotOrientation::Vertical,
            box_fill_color: Vec4 {
                x: 0.7,
                y: 0.7,
                z: 1.0,
                w: 0.8,
            },
            box_stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            median_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            whisker_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            outlier_color: Vec4 {
                x: 1.0,
                y: 0.5,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 1.0,
            outlier_radius: 3.0,
            notched: false,
            notch_width: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec2;

    #[test]
    fn test_boxplot_mark_implementation() {
        assert_eq!(BoxPlot::vertex_count(), 4);
        assert_eq!(BoxPlot::index_count(), Some(6));

        let vertices = BoxPlot::generate_vertices();
        assert_eq!(vertices.len(), 4);

        let indices = BoxPlot::generate_indices().unwrap();
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_boxplot_from_data() {
        let values = vec![
            10.0, 15.0, 20.0, 25.0, 30.0, // Q1 region
            35.0, 40.0, 45.0, 50.0, // Median region
            55.0, 60.0, 65.0, 70.0, 75.0, // Q3 region
            80.0, 85.0, // Upper whisker
            5.0, 95.0, // Outliers
        ];

        let attrs = BoxPlotAttributes::from_data(
            &values,
            vec2![100.0, 200.0],
            40.0,
            BoxPlotOrientation::Vertical,
        );

        // Verify statistical values are reasonable
        assert!(attrs.q1 < attrs.median);
        assert!(attrs.median < attrs.q3);
        assert!(attrs.min <= attrs.q1);
        assert!(attrs.max >= attrs.q3);

        // Verify IQR calculation
        let iqr = attrs.iqr();
        assert!(iqr > 0.0);
        assert_eq!(iqr, attrs.q3 - attrs.q1);
    }

    #[test]
    fn test_boxplot_outlier_detection() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 1000.0];
        let attrs = BoxPlotAttributes::from_data(
            &values,
            vec2![0.0, 0.0],
            40.0,
            BoxPlotOrientation::Vertical,
        );

        // The value 1000.0 should be an outlier
        assert!(attrs.is_outlier(1000.0));

        // Values within IQR should not be outliers
        assert!(!attrs.is_outlier(attrs.median));
        assert!(!attrs.is_outlier(attrs.q1));
        assert!(!attrs.is_outlier(attrs.q3));
    }

    #[test]
    fn test_boxplot_shaders() {
        assert!(BoxPlot::VERTEX_SHADER.is_some());
        assert!(BoxPlot::FRAGMENT_SHADER.is_some());
    }

    #[test]
    fn test_boxplot_attributes_default() {
        let default_attrs = BoxPlotAttributes::default();
        assert_eq!(default_attrs.position.x, 0.0);
        assert_eq!(default_attrs.width, 40.0);
        assert_eq!(default_attrs.median, 50.0);
        assert!(matches!(
            default_attrs.orientation,
            BoxPlotOrientation::Vertical
        ));
    }

    #[test]
    fn test_boxplot_attribute_type_validation() {
        assert_eq!(
            BoxPlot::get_attribute_type("position").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(BoxPlot::get_attribute_type("min").unwrap(), "f32");
        assert_eq!(BoxPlot::get_attribute_type("q1").unwrap(), "f32");
        assert_eq!(BoxPlot::get_attribute_type("median").unwrap(), "f32");
        assert_eq!(BoxPlot::get_attribute_type("q3").unwrap(), "f32");
        assert_eq!(BoxPlot::get_attribute_type("max").unwrap(), "f32");
        assert_eq!(
            BoxPlot::get_attribute_type("box_fill_color").unwrap(),
            "vec4<f32>"
        );
        assert!(BoxPlot::get_attribute_type("unknown").is_err());
    }

    #[test]
    fn test_boxplot_orientation() {
        let vertical = BoxPlotOrientation::Vertical;
        let horizontal = BoxPlotOrientation::Horizontal;

        assert_ne!(vertical, horizontal);
        assert_eq!(vertical, BoxPlotOrientation::Vertical);
    }
}
