// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box plot mark implementation for statistical distribution visualization.
//!
//! The BoxPlot mark provides GPU-accelerated rendering of statistical distributions
//! using the five-number summary (min, Q1, median, Q3, max) plus outliers. It integrates
//! with the statistical shader functions from GUP-139 for efficient quartile calculation.

use crate::mark::Mark;
use crate::selection::{AttrValue, MarkInstanceBuilder};
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

        // BoxPlotInstance struct (must match BoxPlotInstance Rust struct layout)
        shader.push_str("struct BoxPlotInstance {\n");
        shader.push_str("    position: vec2<f32>,\n");
        shader.push_str("    whisker_min: f32,\n");
        shader.push_str("    q1: f32,\n");
        shader.push_str("    median: f32,\n");
        shader.push_str("    q3: f32,\n");
        shader.push_str("    whisker_max: f32,\n");
        shader.push_str("    width: f32,\n");
        shader.push_str("    box_fill_color: vec4<f32>,\n");
        shader.push_str("    box_stroke_color: vec4<f32>,\n");
        shader.push_str("    median_color: vec4<f32>,\n");
        shader.push_str("    whisker_color: vec4<f32>,\n");
        shader.push_str("    outlier_color: vec4<f32>,\n");
        shader.push_str("    stroke_width: f32,\n");
        shader.push_str("    outlier_radius: f32,\n");
        shader.push_str("    orientation: u32,\n");
        shader.push_str("    outlier_count: u32,\n");
        shader.push_str("    notched: u32,\n");
        shader.push_str("    notch_width: f32,\n");
        shader.push_str("    _pad_notch: vec2<f32>,\n");
        shader.push_str("    outliers: array<vec4<f32>, 8>,\n");
        shader.push_str("}\n\n");

        shader.push_str(
            "@group(0) @binding(0) var<storage, read> instances: array<BoxPlotInstance>;\n\n",
        );

        shader.push_str("struct VertexInput {\n");
        shader.push_str("    @location(0) position: vec2<f32>,\n");
        shader.push_str("    @builtin(instance_index) instance_index: u32,\n");
        shader.push_str("}\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) @interpolate(flat) instance_index: u32,\n");
        shader.push_str("}\n\n");

        // Add shader function definitions
        let pipeline_functions = pipeline.generate_vertex_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        shader.push_str("@vertex\n");
        shader.push_str("fn vs_main(input: VertexInput) -> VertexOutput {\n");
        shader.push_str("    let inst = instances[input.instance_index];\n");
        shader.push_str("    var val_min = inst.whisker_min;\n");
        shader.push_str("    var val_max = inst.whisker_max;\n");
        shader.push_str("    for (var i = 0u; i < inst.outlier_count; i++) {\n");
        shader.push_str("        let v = inst.outliers[i / 4u][i % 4u];\n");
        shader.push_str("        val_min = min(val_min, v);\n");
        shader.push_str("        val_max = max(val_max, v);\n");
        shader.push_str("    }\n");
        shader.push_str("    let margin = max(inst.outlier_radius, inst.stroke_width) + 0.005;\n");
        shader.push_str("    val_min -= margin;\n");
        shader.push_str("    val_max += margin;\n");
        shader.push_str("    let half_w = inst.width * 0.5 + margin;\n");
        shader.push_str("    var world_pos: vec2<f32>;\n");
        shader.push_str("    if (inst.orientation == 0u) {\n");
        shader.push_str("        world_pos = vec2<f32>(\n");
        shader.push_str("            inst.position.x + input.position.x * half_w * 2.0,\n");
        shader.push_str("            val_min + (input.position.y + 0.5) * (val_max - val_min),\n");
        shader.push_str("        );\n");
        shader.push_str("    } else {\n");
        shader.push_str("        world_pos = vec2<f32>(\n");
        shader.push_str("            val_min + (input.position.x + 0.5) * (val_max - val_min),\n");
        shader.push_str("            inst.position.y + input.position.y * half_w * 2.0,\n");
        shader.push_str("        );\n");
        shader.push_str("    }\n");
        shader.push_str("    var output: VertexOutput;\n");
        shader.push_str("    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);\n");
        shader.push_str("    output.world_position = world_pos;\n");
        shader.push_str("    output.instance_index = input.instance_index;\n");
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

        // Same BoxPlotInstance struct for storage buffer access
        shader.push_str("struct BoxPlotInstance {\n");
        shader.push_str("    position: vec2<f32>,\n");
        shader.push_str("    whisker_min: f32,\n");
        shader.push_str("    q1: f32,\n");
        shader.push_str("    median: f32,\n");
        shader.push_str("    q3: f32,\n");
        shader.push_str("    whisker_max: f32,\n");
        shader.push_str("    width: f32,\n");
        shader.push_str("    box_fill_color: vec4<f32>,\n");
        shader.push_str("    box_stroke_color: vec4<f32>,\n");
        shader.push_str("    median_color: vec4<f32>,\n");
        shader.push_str("    whisker_color: vec4<f32>,\n");
        shader.push_str("    outlier_color: vec4<f32>,\n");
        shader.push_str("    stroke_width: f32,\n");
        shader.push_str("    outlier_radius: f32,\n");
        shader.push_str("    orientation: u32,\n");
        shader.push_str("    outlier_count: u32,\n");
        shader.push_str("    notched: u32,\n");
        shader.push_str("    notch_width: f32,\n");
        shader.push_str("    _pad_notch: vec2<f32>,\n");
        shader.push_str("    outliers: array<vec4<f32>, 8>,\n");
        shader.push_str("}\n\n");

        shader.push_str(
            "@group(0) @binding(0) var<storage, read> instances: array<BoxPlotInstance>;\n\n",
        );

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) @interpolate(flat) instance_index: u32,\n");
        shader.push_str("}\n\n");

        shader.push_str("@fragment\n");
        shader.push_str("fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n");
        shader.push_str("    let inst = instances[input.instance_index];\n");
        shader.push_str("    return inst.box_fill_color;\n");
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

/// Maximum number of outliers per box plot instance.
///
/// Outlier values are packed into a fixed-size array for GPU upload.
/// Additional outliers beyond this limit are silently dropped.
pub const MAX_OUTLIERS: usize = 32;

/// GPU-ready instance data for box plot rendering.
///
/// This struct matches the WGSL `BoxPlotInstance` layout in `boxplot.vert.wgsl`
/// and is suitable for upload to a storage buffer. Fields are aligned to
/// satisfy WGSL storage buffer alignment rules (vec4 → 16-byte aligned).
///
/// The struct packs all box plot data — statistical values, colours, style
/// parameters, and up to 32 outlier values — so that the entire box plot
/// (box, median, whiskers, caps, outliers) can be rendered in a single
/// instanced draw call.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoxPlotInstance {
    /// Center position in clip space (category axis position)
    pub position: [f32; 2],
    /// Lower whisker end in clip space
    pub whisker_min: f32,
    /// First quartile (bottom of box) in clip space
    pub q1: f32,
    /// Median line position in clip space
    pub median: f32,
    /// Third quartile (top of box) in clip space
    pub q3: f32,
    /// Upper whisker end in clip space
    pub whisker_max: f32,
    /// Box width in clip space units
    pub width: f32,
    /// Box fill colour (RGBA)
    pub box_fill_color: [f32; 4],
    /// Box stroke colour (RGBA)
    pub box_stroke_color: [f32; 4],
    /// Median line colour (RGBA)
    pub median_color: [f32; 4],
    /// Whisker line colour (RGBA)
    pub whisker_color: [f32; 4],
    /// Outlier circle colour (RGBA)
    pub outlier_color: [f32; 4],
    /// Stroke width in clip space units
    pub stroke_width: f32,
    /// Outlier circle radius in clip space units
    pub outlier_radius: f32,
    /// Orientation: 0 = vertical, 1 = horizontal
    pub orientation: u32,
    /// Number of valid outlier values in the `outliers` array
    pub outlier_count: u32,
    /// Whether to render a notch at the median (0 = no, 1 = yes)
    pub notched: u32,
    /// Notch width as fraction of box width (e.g. 0.5 = narrows to 50%)
    pub notch_width: f32,
    /// Padding to align the outliers array to 16 bytes (WGSL requirement)
    pub _pad_notch: [f32; 2],
    /// Outlier values packed into vec4s (up to 32 values, 4 per vec4)
    pub outliers: [[f32; 4]; 8],
}

impl From<&BoxPlotAttributes> for BoxPlotInstance {
    fn from(attrs: &BoxPlotAttributes) -> Self {
        let mut outliers = [[0.0f32; 4]; 8];
        let outlier_count = attrs.outliers.len().min(MAX_OUTLIERS) as u32;
        for (i, &val) in attrs.outliers.iter().take(MAX_OUTLIERS).enumerate() {
            outliers[i / 4][i % 4] = val;
        }

        Self {
            position: [attrs.position.x, attrs.position.y],
            whisker_min: attrs.min,
            q1: attrs.q1,
            median: attrs.median,
            q3: attrs.q3,
            whisker_max: attrs.max,
            width: attrs.width,
            box_fill_color: [
                attrs.box_fill_color.x,
                attrs.box_fill_color.y,
                attrs.box_fill_color.z,
                attrs.box_fill_color.w,
            ],
            box_stroke_color: [
                attrs.box_stroke_color.x,
                attrs.box_stroke_color.y,
                attrs.box_stroke_color.z,
                attrs.box_stroke_color.w,
            ],
            median_color: [
                attrs.median_color.x,
                attrs.median_color.y,
                attrs.median_color.z,
                attrs.median_color.w,
            ],
            whisker_color: [
                attrs.whisker_color.x,
                attrs.whisker_color.y,
                attrs.whisker_color.z,
                attrs.whisker_color.w,
            ],
            outlier_color: [
                attrs.outlier_color.x,
                attrs.outlier_color.y,
                attrs.outlier_color.z,
                attrs.outlier_color.w,
            ],
            stroke_width: attrs.stroke_width,
            outlier_radius: attrs.outlier_radius,
            orientation: match attrs.orientation {
                BoxPlotOrientation::Vertical => 0,
                BoxPlotOrientation::Horizontal => 1,
            },
            outlier_count,
            notched: if attrs.notched { 1 } else { 0 },
            notch_width: attrs.notch_width,
            _pad_notch: [0.0; 2],
            outliers,
        }
    }
}

impl From<BoxPlotAttributes> for BoxPlotInstance {
    fn from(attrs: BoxPlotAttributes) -> Self {
        Self::from(&attrs)
    }
}

impl MarkInstanceBuilder for BoxPlot {
    type Instance = BoxPlotInstance;

    fn default_instance() -> Self::Instance {
        BoxPlotInstance::from(&BoxPlotAttributes::default())
    }

    fn build_instance(attrs: &[(&str, AttrValue)]) -> Self::Instance {
        let mut instance = Self::default_instance();
        for &(name, value) in attrs {
            match name {
                "position" | "center" => {
                    if let AttrValue::Vec2(v) = value {
                        instance.position = v;
                    }
                }
                "min" | "whisker_min" => {
                    if let AttrValue::Float(v) = value {
                        instance.whisker_min = v;
                    }
                }
                "q1" => {
                    if let AttrValue::Float(v) = value {
                        instance.q1 = v;
                    }
                }
                "median" => {
                    if let AttrValue::Float(v) = value {
                        instance.median = v;
                    }
                }
                "q3" => {
                    if let AttrValue::Float(v) = value {
                        instance.q3 = v;
                    }
                }
                "max" | "whisker_max" => {
                    if let AttrValue::Float(v) = value {
                        instance.whisker_max = v;
                    }
                }
                "width" => {
                    if let AttrValue::Float(v) = value {
                        instance.width = v;
                    }
                }
                "box_fill_color" | "fill_color" | "color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.box_fill_color = v;
                    }
                }
                "box_stroke_color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.box_stroke_color = v;
                    }
                }
                "median_color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.median_color = v;
                    }
                }
                "whisker_color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.whisker_color = v;
                    }
                }
                "outlier_color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.outlier_color = v;
                    }
                }
                "stroke_width" => {
                    if let AttrValue::Float(v) = value {
                        instance.stroke_width = v;
                    }
                }
                "outlier_radius" => {
                    if let AttrValue::Float(v) = value {
                        instance.outlier_radius = v;
                    }
                }
                "notched" => {
                    if let AttrValue::Float(v) = value {
                        instance.notched = if v > 0.0 { 1 } else { 0 };
                    }
                }
                "notch_width" => {
                    if let AttrValue::Float(v) = value {
                        instance.notch_width = v;
                    }
                }
                _ => {} // Ignore unknown attributes
            }
        }
        instance
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

    #[test]
    fn test_boxplot_instance_size_and_alignment() {
        // BoxPlotInstance must be exactly 272 bytes to match the WGSL layout.
        assert_eq!(std::mem::size_of::<BoxPlotInstance>(), 272);
        // Alignment must be at least 4 (repr(C)).
        assert!(std::mem::align_of::<BoxPlotInstance>() >= 4);
    }

    #[test]
    fn test_boxplot_instance_from_attributes() {
        let attrs = BoxPlotAttributes {
            position: Vec2 { x: 0.5, y: 0.0 },
            min: 0.1,
            q1: 0.3,
            median: 0.5,
            q3: 0.7,
            max: 0.9,
            outliers: vec![-0.1, 1.1],
            width: 0.2,
            orientation: BoxPlotOrientation::Vertical,
            stroke_width: 0.01,
            outlier_radius: 0.02,
            ..Default::default()
        };

        let inst = BoxPlotInstance::from(&attrs);

        assert_eq!(inst.position, [0.5, 0.0]);
        assert_eq!(inst.whisker_min, 0.1);
        assert_eq!(inst.q1, 0.3);
        assert_eq!(inst.median, 0.5);
        assert_eq!(inst.q3, 0.7);
        assert_eq!(inst.whisker_max, 0.9);
        assert_eq!(inst.width, 0.2);
        assert_eq!(inst.orientation, 0); // Vertical
        assert_eq!(inst.outlier_count, 2);
        assert_eq!(inst.outliers[0][0], -0.1);
        assert_eq!(inst.outliers[0][1], 1.1);
        assert_eq!(inst.stroke_width, 0.01);
        assert_eq!(inst.outlier_radius, 0.02);
    }

    #[test]
    fn test_boxplot_instance_horizontal_orientation() {
        let attrs = BoxPlotAttributes {
            orientation: BoxPlotOrientation::Horizontal,
            ..Default::default()
        };
        let inst = BoxPlotInstance::from(&attrs);
        assert_eq!(inst.orientation, 1);
    }

    #[test]
    fn test_boxplot_instance_outlier_clamping() {
        // More than MAX_OUTLIERS should be clamped silently.
        let attrs = BoxPlotAttributes {
            outliers: (0..40).map(|i| i as f32).collect(),
            ..Default::default()
        };
        let inst = BoxPlotInstance::from(&attrs);
        assert_eq!(inst.outlier_count, MAX_OUTLIERS as u32);
        // Last valid packed value: index 31 → vec4 index 7, component 3
        assert_eq!(inst.outliers[7][3], 31.0);
    }

    #[test]
    fn test_boxplot_instance_from_owned() {
        let attrs = BoxPlotAttributes::default();
        let inst_ref = BoxPlotInstance::from(&attrs);
        let inst_owned = BoxPlotInstance::from(attrs);
        // Both conversions should produce identical results.
        assert_eq!(inst_ref.position, inst_owned.position);
        assert_eq!(inst_ref.median, inst_owned.median);
    }

    #[test]
    fn test_boxplot_instance_builder_default() {
        let inst = BoxPlot::default_instance();
        let default_attrs = BoxPlotAttributes::default();
        let expected = BoxPlotInstance::from(&default_attrs);
        assert_eq!(inst.position, expected.position);
        assert_eq!(inst.median, expected.median);
        assert_eq!(inst.q1, expected.q1);
        assert_eq!(inst.q3, expected.q3);
    }

    #[test]
    fn test_boxplot_instance_builder_with_attrs() {
        use crate::selection::AttrValue;

        let inst = BoxPlot::build_instance(&[
            ("position", AttrValue::Vec2([0.5, 0.0])),
            ("min", AttrValue::Float(0.1)),
            ("q1", AttrValue::Float(0.3)),
            ("median", AttrValue::Float(0.5)),
            ("q3", AttrValue::Float(0.7)),
            ("max", AttrValue::Float(0.9)),
            ("width", AttrValue::Float(0.2)),
            ("stroke_width", AttrValue::Float(0.01)),
        ]);
        assert_eq!(inst.position, [0.5, 0.0]);
        assert_eq!(inst.whisker_min, 0.1);
        assert_eq!(inst.q1, 0.3);
        assert_eq!(inst.median, 0.5);
        assert_eq!(inst.q3, 0.7);
        assert_eq!(inst.whisker_max, 0.9);
        assert_eq!(inst.width, 0.2);
        assert_eq!(inst.stroke_width, 0.01);
    }

    #[test]
    fn test_boxplot_instance_builder_color_attrs() {
        use crate::selection::AttrValue;

        let red = [1.0, 0.0, 0.0, 1.0];
        let green = [0.0, 1.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];

        let inst = BoxPlot::build_instance(&[
            ("box_fill_color", AttrValue::Vec4(red)),
            ("median_color", AttrValue::Vec4(green)),
            ("whisker_color", AttrValue::Vec4(blue)),
        ]);
        assert_eq!(inst.box_fill_color, red);
        assert_eq!(inst.median_color, green);
        assert_eq!(inst.whisker_color, blue);
    }

    #[test]
    fn test_boxplot_instance_builder_aliases() {
        use crate::selection::AttrValue;

        // "center" alias for "position"
        let inst = BoxPlot::build_instance(&[("center", AttrValue::Vec2([0.3, 0.4]))]);
        assert_eq!(inst.position, [0.3, 0.4]);

        // "fill_color" alias for "box_fill_color"
        let yellow = [1.0, 1.0, 0.0, 1.0];
        let inst = BoxPlot::build_instance(&[("fill_color", AttrValue::Vec4(yellow))]);
        assert_eq!(inst.box_fill_color, yellow);

        // "color" alias for "box_fill_color"
        let inst = BoxPlot::build_instance(&[("color", AttrValue::Vec4(yellow))]);
        assert_eq!(inst.box_fill_color, yellow);

        // "whisker_min" / "whisker_max" aliases
        let inst = BoxPlot::build_instance(&[
            ("whisker_min", AttrValue::Float(0.05)),
            ("whisker_max", AttrValue::Float(0.95)),
        ]);
        assert_eq!(inst.whisker_min, 0.05);
        assert_eq!(inst.whisker_max, 0.95);
    }

    #[test]
    fn test_boxplot_instance_builder_ignores_unknown() {
        use crate::selection::AttrValue;

        let default = BoxPlot::default_instance();
        let inst = BoxPlot::build_instance(&[("unknown_attr", AttrValue::Float(999.0))]);
        assert_eq!(inst.position, default.position);
        assert_eq!(inst.median, default.median);
    }

    #[test]
    fn test_boxplot_instance_notch_fields_default() {
        let inst = BoxPlot::default_instance();
        // Default: not notched
        assert_eq!(inst.notched, 0);
        assert_eq!(inst.notch_width, 0.5);
        assert_eq!(inst._pad_notch, [0.0; 2]);
    }

    #[test]
    fn test_boxplot_instance_notch_from_attributes() {
        let attrs = BoxPlotAttributes {
            notched: true,
            notch_width: 0.4,
            ..Default::default()
        };
        let inst = BoxPlotInstance::from(&attrs);
        assert_eq!(inst.notched, 1);
        assert_eq!(inst.notch_width, 0.4);
    }

    #[test]
    fn test_boxplot_instance_notch_disabled() {
        let attrs = BoxPlotAttributes {
            notched: false,
            notch_width: 0.3,
            ..Default::default()
        };
        let inst = BoxPlotInstance::from(&attrs);
        assert_eq!(inst.notched, 0);
        // notch_width is still stored even when not notched
        assert_eq!(inst.notch_width, 0.3);
    }

    #[test]
    fn test_boxplot_instance_builder_notch_attrs() {
        use crate::selection::AttrValue;

        let inst = BoxPlot::build_instance(&[
            ("notched", AttrValue::Float(1.0)),
            ("notch_width", AttrValue::Float(0.6)),
        ]);
        assert_eq!(inst.notched, 1);
        assert_eq!(inst.notch_width, 0.6);
    }
}
