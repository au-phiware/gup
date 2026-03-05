// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Observable Plot-style chart builders for GPU-accelerated visualizations.
//!
//! This module provides high-level, fluent APIs for creating common chart types
//! while maintaining full GPU performance and seamless interoperability with
//! the low-level Selection system.
//!
//! # Key Features
//!
//! * **One-line chart creation** with Observable Plot compatibility
//! * **Type-safe accessor functions** with compile-time validation
//! * **Zero-cost abstractions** over Phase 1 Selection primitives
//! * **Seamless conversion** to low-level APIs for advanced customization
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::prelude::*;
//!
//! #[derive(Debug, Clone)]
//! struct SalesPoint {
//!     revenue: f32,
//!     profit: f32,
//!     region: String,
//! }
//!
//! # async fn example() -> GupResult<()> {
//! let sales_data = vec![
//!     SalesPoint { revenue: 100.0, profit: 20.0, region: "North".to_string() },
//!     SalesPoint { revenue: 200.0, profit: 45.0, region: "South".to_string() },
//! ];
//!
//! // Observable Plot-style API
//! let chart = gup::plot()
//!     .data(sales_data)
//!     .scatter(x("revenue"), y("profit"))
//!     .color(color("region"));
//! # Ok(())
//! # }
//! ```

pub mod accessor;
pub mod builders;
pub mod colorbar;
pub mod labels;
pub mod optimized_accessor;
pub mod pipeline_cache;
pub mod plot_api;
pub mod shader_specialization;

pub use accessor::*;
pub use builders::*;
pub use colorbar::*;
pub use labels::*;
pub use optimized_accessor::*;
pub use pipeline_cache::*;
pub use plot_api::*;
pub use shader_specialization::*;

use crate::RenderContext;
use crate::axis::{
    Axis, AxisBounds, AxisConfiguration, AxisLabel, AxisLinePipeline, AxisPosition, AxisRenderer,
    LinearAxis, TickInstance, TickPipeline,
};
use crate::error::{GupError, GupResult};
use crate::grid::GridConfiguration;
use crate::label::{
    AxisInfo, LabelConstraints, LabelFormatter, LabelLayout, LabelPosition, LabelPositioner,
};
use crate::render::Vertex;
use crate::selection::Selection;
use crate::shader_function::{BandScale, ColorScale, LinearScale, LogScale, PointScale, Vec2};
use crate::text::TextStyle;
use crate::text::hover_reveal::{ClippedTextRegistry, HoverRevealState, TooltipConfig};
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

/// Rich axis geometry output containing line vertices and instanced tick data.
///
/// Instead of combining axis lines and ticks into a single `Vec<Vertex>`,
/// this struct separates them so that ticks can be rendered via GPU
/// instancing (one draw call per tick type) while axis lines continue to
/// use the standard `LineList` vertex-pair path.
///
/// Use [`ComposedChart::generate_axis_geometry_instanced`] to obtain this
/// struct.
#[derive(Debug, Clone)]
pub struct AxisGeometry {
    /// Vertex pairs for axis lines only (rendered with a `LineList` pipeline).
    pub line_vertices: Vec<Vertex>,
    /// Per-tick instance data for GPU-instanced rendering via [`crate::axis::TickPipeline`].
    pub tick_instances: Vec<TickInstance>,
    /// Label data for text rendering.
    pub labels: Vec<AxisLabel>,
}

impl AxisGeometry {
    /// Flatten into the legacy `(Vec<Vertex>, Vec<AxisLabel>)` format.
    ///
    /// This is a convenience for callers that still use a single `LineList`
    /// draw call for both axis lines and ticks.  Each [`TickInstance`] is
    /// expanded into two [`Vertex`] entries (the two endpoints of the tick
    /// line segment).
    pub fn into_legacy(self) -> (Vec<Vertex>, Vec<AxisLabel>) {
        let mut vertices = self.line_vertices;
        for inst in &self.tick_instances {
            // Start point: the on-axis position
            vertices.push(Vertex {
                position: inst.position,
                color: inst.color,
            });
            // End point: position + tick_vector
            vertices.push(Vertex {
                position: [
                    inst.position[0] + inst.tick_vector[0],
                    inst.position[1] + inst.tick_vector[1],
                ],
                color: inst.color,
            });
        }
        (vertices, self.labels)
    }

    /// Return the total number of tick instances.
    pub fn tick_count(&self) -> usize {
        self.tick_instances.len()
    }
}

/// Uploaded GPU buffers for instanced tick rendering.
///
/// Created by [`ComposedChart::render`] and consumed by
/// [`ComposedChart::draw_ticks`].
#[derive(Debug)]
struct TickBuffers {
    base_buf: wgpu::Buffer,
    inst_buf: wgpu::Buffer,
    instance_count: u32,
}

/// Uploaded GPU buffer for axis-line vertex rendering.
///
/// Created by [`ComposedChart::render`] and consumed by
/// [`ComposedChart::draw_axis_lines`].
#[derive(Debug)]
struct AxisLineBuffers {
    vertex_buf: wgpu::Buffer,
    vertex_count: u32,
}

/// GPU pipeline for rendering the colorbar gradient strip using TriangleList
/// topology with per-vertex colours.
///
/// Uses the same `basic.wgsl` shader as axis lines but with triangle
/// topology instead of line-list.
struct GradientStripPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl std::fmt::Debug for GradientStripPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GradientStripPipeline")
            .field("pipeline", &"<wgpu::RenderPipeline>")
            .finish()
    }
}

impl GradientStripPipeline {
    /// Create a new gradient-strip pipeline for the given surface format.
    fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient_strip_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/basic.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gradient_strip_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gradient_strip_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self { pipeline }
    }

    /// Upload gradient vertices to a GPU buffer.
    fn upload(&self, device: &wgpu::Device, vertices: &[Vertex]) -> wgpu::Buffer {
        use wgpu::util::{BufferInitDescriptor, DeviceExt};

        device.create_buffer_init(&BufferInitDescriptor {
            label: Some("gradient_strip_vertex_buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    /// Record gradient draw commands into an active render pass.
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        vertex_buf: &'a wgpu::Buffer,
        vertex_count: u32,
    ) {
        if vertex_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, vertex_buf.slice(..));
        render_pass.draw(0..vertex_count, 0..1);
    }
}

/// Uploaded GPU buffer for colorbar gradient rendering.
#[derive(Debug)]
struct GradientStripBuffers {
    vertex_buf: wgpu::Buffer,
    vertex_count: u32,
}

/// Core trait for all chart builders providing fluent API construction.
///
/// This trait enables type-safe chart building with compile-time validation
/// and automatic conversion to GPU-accelerated Selection instances.
///
/// # Type Parameters
///
/// * `T` - The data type for chart elements
/// * `Output` - The rendered chart type (typically a Selection)
///
/// # Design Philosophy
///
/// Chart builders are **zero-cost abstractions** that compile down to
/// efficient Selection operations. The fluent API provides Observable Plot
/// compatibility while maintaining full GPU performance.
pub trait ChartBuilder<T>: Sized {
    /// The output type after building (typically Selection<T, M>)
    type Output;

    /// Build the chart with the provided data and context.
    ///
    /// This method transforms the high-level chart configuration into
    /// GPU-accelerated Selection primitives.
    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output>;

    /// Convenience method to build and render in one step.
    fn render_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output>
    where
        Self::Output: crate::Mixable,
    {
        let output = self.build_with_data(data, context)?;
        // The render method is called during the mixable render process
        Ok(output)
    }
}

/// A chart builder that has been bound to specific data.
///
/// This intermediate type enables method chaining while preserving type
/// information about the data and chart configuration.
pub struct BoundChartBuilder<B, T>
where
    B: ChartBuilder<T>,
{
    pub(crate) builder: B,
    pub(crate) data: Vec<T>,
    pub(crate) context: Arc<RenderContext>,
    pub(crate) _phantom: PhantomData<T>,
}

impl<B, T> BoundChartBuilder<B, T>
where
    B: ChartBuilder<T>,
{
    /// Create a new bound chart builder.
    pub fn new(builder: B, data: Vec<T>, context: Arc<RenderContext>) -> Self {
        Self {
            builder,
            data,
            context,
            _phantom: PhantomData,
        }
    }

    /// Build the chart using the bound data and context.
    pub fn build(self) -> GupResult<B::Output> {
        self.builder.build_with_data(self.data, self.context)
    }

    /// Build and render the chart in one step.
    pub fn render(self) -> GupResult<B::Output>
    where
        B::Output: crate::Mixable,
    {
        self.builder.render_with_data(self.data, self.context)
    }

    // Convert this builder to a low-level Selection for advanced customization.
    //
    // This enables seamless transition from high-level builder APIs to
    // low-level Selection operations when needed.
    //
    // TODO: Disabled until Selection type is fully implemented
    /*
    pub fn into_selection<M>(self) -> GupResult<Selection<T, M>>
    where
        T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
        M: crate::selection::Mark,
        M::AttributeValue: Default + Clone,
    {
        Selection::new(self.data, self.context)
    }
    */

    /// Access the underlying data for inspection.
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// Get the number of data points.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the dataset is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Horizontal alignment for chart title text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleAlignment {
    /// Align the title to the left edge of the chart.
    Left,
    /// Center the title horizontally (default).
    #[default]
    Center,
    /// Align the title to the right edge of the chart.
    Right,
}

/// Configuration for chart title and optional subtitle.
///
/// Provides fine-grained control over title text, subtitle text,
/// alignment, and vertical positioning. When omitted from a
/// [`ChartConfig`], no title is rendered (backward compatible with
/// the pre-`TitleConfig` behaviour of `title: None`).
///
/// # Examples
///
/// ```rust
/// use gup::chart_builder::{TitleConfig, TitleAlignment};
/// use gup::text::TextStyle;
///
/// let title = TitleConfig::new("Revenue by Quarter")
///     .with_alignment(TitleAlignment::Left)
///     .with_subtitle("FY 2024")
///     .with_subtitle_style(TextStyle::new(14.0).with_rgba(0.5, 0.5, 0.5, 1.0));
/// ```
#[derive(Debug, Clone)]
pub struct TitleConfig {
    /// Primary title text.
    pub text: String,

    /// Horizontal alignment within the chart area.
    pub alignment: TitleAlignment,

    /// Vertical offset from the top edge of the chart (in pixels).
    ///
    /// When `None`, the title is positioned at `margins.top / 2.0`
    /// (the vertical centre of the top margin).
    pub y_offset: Option<f32>,

    /// Optional subtitle, rendered below the main title.
    pub subtitle: Option<String>,

    /// Text style for the subtitle.
    ///
    /// Defaults to a smaller, lighter variant when not explicitly set.
    pub subtitle_style: TextStyle,

    /// Line spacing multiplier between lines in a multi-line title.
    ///
    /// Applied to both the title and the gap between title and
    /// subtitle. Defaults to 1.2.
    pub line_spacing: f32,
}

impl TitleConfig {
    /// Create a new title configuration with the given text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            alignment: TitleAlignment::default(),
            y_offset: None,
            subtitle: None,
            subtitle_style: TextStyle::new(14.0).with_rgba(0.4, 0.4, 0.4, 1.0),
            line_spacing: 1.2,
        }
    }

    /// Set horizontal alignment.
    pub fn with_alignment(mut self, alignment: TitleAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the vertical offset from the top edge (in pixels).
    pub fn with_y_offset(mut self, offset: f32) -> Self {
        self.y_offset = Some(offset);
        self
    }

    /// Set subtitle text.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set the text style for the subtitle.
    pub fn with_subtitle_style(mut self, style: TextStyle) -> Self {
        self.subtitle_style = style;
        self
    }

    /// Set the line spacing multiplier for multi-line titles.
    pub fn with_line_spacing(mut self, spacing: f32) -> Self {
        self.line_spacing = spacing.max(0.1);
        self
    }
}

/// Axis scale variant, supporting different scale types for chart axes.
///
/// This enum wraps the shader-function scale types ([`LinearScale`],
/// [`LogScale`], [`BandScale`], and [`PointScale`]) so that
/// `ChartConfig::x_scale` / `ChartConfig::y_scale` can accept any kind.
#[derive(Debug, Clone)]
pub enum AxisScale {
    /// Linear interpolation between domain and range.
    Linear(LinearScale),
    /// Logarithmic interpolation between domain and range.
    Log(LogScale),
    /// Band scale for categorical data (maps index to band centre).
    Band(BandScale),
    /// Point scale for categorical data (maps index to point position).
    Point(PointScale),
}

impl AxisScale {
    /// Return the domain minimum value.
    pub fn domain_min(&self) -> f32 {
        match self {
            AxisScale::Linear(s) => s.domain_min,
            AxisScale::Log(s) => s.domain_min,
            AxisScale::Band(_) | AxisScale::Point(_) => 0.0,
        }
    }

    /// Return the domain maximum value.
    pub fn domain_max(&self) -> f32 {
        match self {
            AxisScale::Linear(s) => s.domain_max,
            AxisScale::Log(s) => s.domain_max,
            AxisScale::Band(s) => s.category_count.saturating_sub(1) as f32,
            AxisScale::Point(s) => s.category_count.saturating_sub(1) as f32,
        }
    }

    /// Map a data-domain value to the output range.
    ///
    /// For linear scales this is a simple affine interpolation.
    /// For log scales the input is log-transformed first.
    /// Band/point scales map an integer index to the band/point position.
    pub fn scale_value(&self, value: f32) -> f32 {
        match self {
            AxisScale::Linear(s) => {
                let span = s.domain_max - s.domain_min;
                if span.abs() < f32::EPSILON {
                    return s.range_min;
                }
                let t = (value - s.domain_min) / span;
                s.range_min + t * (s.range_max - s.range_min)
            }
            AxisScale::Log(s) => s.apply(value),
            AxisScale::Band(s) => s.apply(value as u32),
            AxisScale::Point(s) => s.apply(value as u32),
        }
    }

    /// Create a tick-generator [`Scale`](crate::tick_generator::Scale) trait
    /// object matching this axis scale's type and domain.
    pub fn to_tick_scale(&self) -> Box<dyn crate::tick_generator::Scale> {
        match self {
            AxisScale::Linear(s) => Box::new(crate::tick_generator::LinearScale::new(
                s.domain_min as f64,
                s.domain_max as f64,
            )),
            AxisScale::Log(s) => Box::new(crate::tick_generator::LogarithmicScale::base_10(
                s.domain_min as f64,
                s.domain_max as f64,
            )),
            AxisScale::Band(s) => Box::new(crate::tick_generator::LinearScale::new(
                0.0,
                s.category_count.saturating_sub(1) as f64,
            )),
            AxisScale::Point(s) => Box::new(crate::tick_generator::LinearScale::new(
                0.0,
                s.category_count.saturating_sub(1) as f64,
            )),
        }
    }
}

impl From<LinearScale> for AxisScale {
    fn from(s: LinearScale) -> Self {
        AxisScale::Linear(s)
    }
}

impl From<LogScale> for AxisScale {
    fn from(s: LogScale) -> Self {
        AxisScale::Log(s)
    }
}

impl From<BandScale> for AxisScale {
    fn from(s: BandScale) -> Self {
        AxisScale::Band(s)
    }
}

impl From<PointScale> for AxisScale {
    fn from(s: PointScale) -> Self {
        AxisScale::Point(s)
    }
}

/// Chart configuration that applies to all chart types.
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// Chart title configuration (optional).
    ///
    /// Use [`ChartConfig::with_title`] for simple titles or
    /// [`ChartConfig::with_title_config`] for full layout control
    /// (alignment, subtitle, y-offset).
    pub title_config: Option<TitleConfig>,

    /// Chart width in pixels
    pub width: f32,

    /// Chart height in pixels
    pub height: f32,

    /// Chart margins
    pub margins: Margins,

    /// Background color (optional)
    pub background_color: Option<[f32; 4]>,

    /// Whether to show axes
    pub show_axes: bool,

    /// Whether to show grid lines
    pub show_grid: bool,

    /// Grid system configuration
    pub grid_config: GridConfiguration,

    /// Text style for axis labels.
    ///
    /// When `font_family` is set, the chart text rendering methods
    /// will use `FontAtlasManager` to resolve the correct font atlas
    /// automatically.
    pub label_style: TextStyle,

    /// Text style for the chart title.
    ///
    /// When `font_family` is set, the chart text rendering methods
    /// will use `FontAtlasManager` to resolve the correct font atlas
    /// automatically.
    pub title_style: TextStyle,

    /// Whether hover reveal is enabled for clipped text.
    ///
    /// When `true`, truncated axis labels and titles automatically
    /// register with a [`ClippedTextRegistry`] so that hovering over
    /// them reveals the full text in a tooltip.
    pub hover_reveal: bool,

    /// Tooltip configuration for hover reveal.
    ///
    /// Controls appearance (colours, padding, font size) and timing
    /// (show delay, fade durations) of the tooltip shown when hovering
    /// over clipped text. Only used when [`hover_reveal`](Self::hover_reveal)
    /// is `true`.
    pub tooltip_config: TooltipConfig,

    /// Optional X-axis shader-function scale.
    ///
    /// When set, the scale's domain is used to auto-configure axis tick
    /// generation and the corresponding WGSL function is included in the
    /// shader pipeline.  Accepts [`LinearScale`] or [`LogScale`] via the
    /// [`AxisScale`] enum.
    pub x_scale: Option<AxisScale>,

    /// Optional Y-axis shader-function scale.
    ///
    /// When set, the scale's domain is used to auto-configure axis tick
    /// generation and the corresponding WGSL function is included in the
    /// shader pipeline.  Accepts [`LinearScale`] or [`LogScale`] via the
    /// [`AxisScale`] enum.
    pub y_scale: Option<AxisScale>,

    /// Optional colour-scale shader function.
    ///
    /// When set, the [`ColorScale`] is wired into the chart's shader
    /// pipeline so that a numeric data value is mapped to an RGBA colour
    /// on the GPU.
    pub color_scale: Option<ColorScale>,

    /// Whether to show a colorbar legend for the colour scale.
    ///
    /// When `true` and [`color_scale`](Self::color_scale) is set, a
    /// gradient strip with tick marks and labels is rendered adjacent to
    /// the plot area.
    pub show_colorbar: bool,

    /// Optional custom X-axis tick label formatter.
    ///
    /// When set, axis tick labels for the bottom/top axis are formatted
    /// using this formatter instead of the default [`NumericFormatter`].
    ///
    /// [`NumericFormatter`]: crate::label::NumericFormatter
    pub x_label_formatter: Option<Arc<dyn LabelFormatter>>,

    /// Optional custom Y-axis tick label formatter.
    ///
    /// When set, axis tick labels for the left/right axis are formatted
    /// using this formatter instead of the default [`NumericFormatter`].
    ///
    /// [`NumericFormatter`]: crate::label::NumericFormatter
    pub y_label_formatter: Option<Arc<dyn LabelFormatter>>,
}

/// Chart margin specification.
#[derive(Debug, Clone, Copy)]
pub struct Margins {
    /// Top margin in pixels.
    pub top: f32,
    /// Right margin in pixels.
    pub right: f32,
    /// Bottom margin in pixels.
    pub bottom: f32,
    /// Left margin in pixels.
    pub left: f32,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            title_config: None,
            width: 800.0,
            height: 600.0,
            margins: Margins::default(),
            background_color: None,
            show_axes: true,
            show_grid: false,
            grid_config: GridConfiguration::default(),
            label_style: TextStyle::new(14.0),
            title_style: TextStyle::new(18.0).bold(),
            hover_reveal: false,
            tooltip_config: TooltipConfig::default(),
            x_scale: None,
            y_scale: None,
            color_scale: None,
            show_colorbar: false,
            x_label_formatter: None,
            y_label_formatter: None,
        }
    }
}

impl ChartConfig {
    /// Enable grid rendering with default configuration.
    pub fn with_grid(mut self) -> Self {
        self.show_grid = true;
        self
    }

    /// Enable grid rendering with custom configuration.
    pub fn with_grid_config(mut self, config: GridConfiguration) -> Self {
        self.show_grid = true;
        self.grid_config = config;
        self
    }

    /// Disable grid rendering.
    pub fn without_grid(mut self) -> Self {
        self.show_grid = false;
        self
    }

    /// Set the text style for axis labels.
    ///
    /// Use [`TextStyle::with_font_family`] to specify a font; the chart's
    /// multi-font rendering methods will resolve it through a
    /// [`FontAtlasManager`](crate::text::FontAtlasManager) automatically.
    pub fn with_label_style(mut self, style: TextStyle) -> Self {
        self.label_style = style;
        self
    }

    /// Set the text style for the chart title.
    ///
    /// Use [`TextStyle::with_font_family`] to specify a font; the chart's
    /// multi-font rendering methods will resolve it through a
    /// [`FontAtlasManager`](crate::text::FontAtlasManager) automatically.
    pub fn with_title_style(mut self, style: TextStyle) -> Self {
        self.title_style = style;
        self
    }

    /// Set the chart title (simple, centred by default).
    ///
    /// This is a convenience method that creates a [`TitleConfig`] with
    /// centre alignment and default settings. For full control over
    /// alignment, subtitle, and offset use [`with_title_config`](Self::with_title_config).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title_config = Some(TitleConfig::new(title));
        self
    }

    /// Set full title configuration (alignment, subtitle, y-offset).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gup::chart_builder::{ChartConfig, TitleConfig, TitleAlignment};
    ///
    /// let config = ChartConfig::default()
    ///     .with_title_config(
    ///         TitleConfig::new("Sales Report")
    ///             .with_alignment(TitleAlignment::Left)
    ///             .with_subtitle("Q4 2024"),
    ///     );
    /// ```
    pub fn with_title_config(mut self, config: TitleConfig) -> Self {
        self.title_config = Some(config);
        self
    }

    /// Return the title text, if configured.
    ///
    /// Convenience accessor that reads through [`TitleConfig`].
    pub fn title(&self) -> Option<&str> {
        self.title_config.as_ref().map(|c| c.text.as_str())
    }

    /// Enable or disable hover reveal for clipped text.
    ///
    /// When enabled, axis labels and chart titles that are truncated
    /// due to limited space automatically show a tooltip with the full
    /// text when the user hovers over them.
    pub fn with_hover_reveal(mut self, enabled: bool) -> Self {
        self.hover_reveal = enabled;
        self
    }

    /// Set the tooltip configuration for hover reveal.
    ///
    /// Implicitly enables hover reveal. Use this to customise tooltip
    /// appearance and timing.
    pub fn with_tooltip_config(mut self, config: TooltipConfig) -> Self {
        self.tooltip_config = config;
        self.hover_reveal = true;
        self
    }

    /// Set the X-axis scale.
    ///
    /// Accepts any scale type that implements `Into<AxisScale>`, including
    /// [`LinearScale`] and [`LogScale`].  When set, the scale's domain is
    /// used to auto-configure axis tick generation.
    pub fn with_x_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.x_scale = Some(scale.into());
        self
    }

    /// Set the Y-axis scale.
    ///
    /// Accepts any scale type that implements `Into<AxisScale>`, including
    /// [`LinearScale`] and [`LogScale`].  When set, the scale's domain is
    /// used to auto-configure axis tick generation.
    pub fn with_y_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.y_scale = Some(scale.into());
        self
    }

    /// Set the colour scale for value-to-colour mapping.
    ///
    /// When set, the [`ColorScale`] shader function is wired into the
    /// chart's shader pipeline so that a numeric data value is mapped to an
    /// RGBA colour entirely on the GPU.
    ///
    /// Built-in palettes such as [`ColorScale::viridis`],
    /// [`ColorScale::plasma`], and [`ColorScale::rd_bu`] are accepted
    /// directly.
    pub fn with_color_scale(mut self, scale: impl Into<ColorScale>) -> Self {
        self.color_scale = Some(scale.into());
        self
    }

    /// Enable or disable the colorbar legend.
    ///
    /// The colorbar is only rendered when both `show_colorbar` is `true`
    /// **and** a [`ColorScale`] is configured via
    /// [`with_color_scale`](Self::with_color_scale).
    pub fn with_colorbar(mut self, show: bool) -> Self {
        self.show_colorbar = show;
        self
    }

    /// Set a custom X-axis tick label formatter.
    ///
    /// The formatter is used for bottom and top axes. Pass any type
    /// that implements [`LabelFormatter`] (e.g. [`PercentFormatter`],
    /// [`NumericFormatter`], [`DateTimeFormatter`]).
    ///
    /// [`PercentFormatter`]: crate::label::PercentFormatter
    /// [`NumericFormatter`]: crate::label::NumericFormatter
    /// [`DateTimeFormatter`]: crate::label::DateTimeFormatter
    pub fn with_x_label_formatter(mut self, formatter: impl LabelFormatter) -> Self {
        self.x_label_formatter = Some(Arc::new(formatter));
        self
    }

    /// Set a custom Y-axis tick label formatter.
    ///
    /// The formatter is used for left and right axes. Pass any type
    /// that implements [`LabelFormatter`] (e.g. [`PercentFormatter`],
    /// [`NumericFormatter`], [`DateTimeFormatter`]).
    ///
    /// [`PercentFormatter`]: crate::label::PercentFormatter
    /// [`NumericFormatter`]: crate::label::NumericFormatter
    /// [`DateTimeFormatter`]: crate::label::DateTimeFormatter
    pub fn with_y_label_formatter(mut self, formatter: impl LabelFormatter) -> Self {
        self.y_label_formatter = Some(Arc::new(formatter));
        self
    }

    /// Return the label formatter for a given axis position, if any.
    pub(crate) fn label_formatter_for(
        &self,
        position: AxisPosition,
    ) -> Option<&dyn LabelFormatter> {
        match position {
            AxisPosition::Bottom | AxisPosition::Top => self
                .x_label_formatter
                .as_ref()
                .map(|f| &**f as &dyn LabelFormatter),
            AxisPosition::Left | AxisPosition::Right => self
                .y_label_formatter
                .as_ref()
                .map(|f| &**f as &dyn LabelFormatter),
        }
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 40.0,
            right: 40.0,
            bottom: 60.0,
            left: 60.0,
        }
    }
}

impl Margins {
    /// Create uniform margins on all sides.
    pub fn uniform(margin: f32) -> Self {
        Self {
            top: margin,
            right: margin,
            bottom: margin,
            left: margin,
        }
    }

    /// Create symmetric margins (top/bottom and left/right).
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// A composed chart that includes both data visualization and axis rendering.
///
/// This struct represents a complete chart with both the main visualization
/// (e.g., scatter plot, line chart) and optional axis components.
#[derive(Debug)]
pub struct ComposedChart<T, M>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// The main data visualization
    pub visualization: Selection<T, M>,
    /// Bottom axis (X-axis)
    pub bottom_axis: Option<Box<dyn Axis>>,
    /// Left axis (Y-axis)
    pub left_axis: Option<Box<dyn Axis>>,
    /// Top axis (secondary X-axis)
    pub top_axis: Option<Box<dyn Axis>>,
    /// Right axis (secondary Y-axis)
    pub right_axis: Option<Box<dyn Axis>>,
    /// Chart configuration
    pub config: ChartConfig,
    /// Grid system for rendering grid lines
    pub grid_system: Option<crate::grid::GridSystem>,
    /// GPU-instanced tick rendering pipeline (lazily created on first render).
    tick_pipeline: Option<TickPipeline>,
    /// Uploaded tick buffers ready for drawing.
    tick_buffers: Option<TickBuffers>,
    /// Uploaded grid line instance buffers for instanced drawing.
    grid_buffers: Option<TickBuffers>,
    /// Cached axis-line render pipeline (lazily created on first render).
    axis_line_pipeline: Option<AxisLinePipeline>,
    /// Uploaded axis-line vertex buffer ready for drawing.
    axis_line_buffers: Option<AxisLineBuffers>,
    /// Registry of clipped/truncated text for hover reveal.
    clipped_text_registry: ClippedTextRegistry,
    /// Hover reveal state machine managing tooltip visibility.
    hover_state: HoverRevealState,
    /// Cached triangle-list pipeline for colorbar gradient rendering.
    colorbar_gradient_pipeline: Option<GradientStripPipeline>,
    /// Uploaded colorbar gradient vertex buffer.
    colorbar_gradient_buffers: Option<GradientStripBuffers>,
    /// Cached colorbar geometry (labels are stored here for text rendering).
    colorbar_geometry: Option<colorbar::ColorbarGeometry>,
}

impl<T, M> ComposedChart<T, M>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// Create a new composed chart from a visualization and configuration.
    pub fn new(visualization: Selection<T, M>, config: ChartConfig) -> Self {
        let grid_system = if config.show_grid {
            Some(crate::grid::GridSystem::new(config.grid_config.clone()))
        } else {
            None
        };

        let hover_state = HoverRevealState::new(config.tooltip_config.clone());

        Self {
            visualization,
            bottom_axis: None,
            left_axis: None,
            top_axis: None,
            right_axis: None,
            config,
            grid_system,
            tick_pipeline: None,
            tick_buffers: None,
            grid_buffers: None,
            axis_line_pipeline: None,
            axis_line_buffers: None,
            clipped_text_registry: ClippedTextRegistry::new(),
            hover_state,
            colorbar_gradient_pipeline: None,
            colorbar_gradient_buffers: None,
            colorbar_geometry: None,
        }
    }

    /// Add axes to the chart based on configuration.
    pub fn with_default_axes(mut self) -> Self {
        if self.config.show_axes {
            let axis_config = AxisConfiguration::default();

            // Add bottom axis (X-axis)
            self.bottom_axis = Some(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )));

            // Add left axis (Y-axis)
            self.left_axis = Some(Box::new(LinearAxis::new(AxisPosition::Left, axis_config)));
        }
        self
    }

    /// Add a custom bottom axis.
    pub fn with_bottom_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.bottom_axis = Some(axis);
        self
    }

    /// Add a custom left axis.
    pub fn with_left_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.left_axis = Some(axis);
        self
    }

    /// Add a custom top axis.
    pub fn with_top_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.top_axis = Some(axis);
        self
    }

    /// Add a custom right axis.
    pub fn with_right_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.right_axis = Some(axis);
        self
    }

    /// Get the number of data elements in the visualization.
    pub fn len(&self) -> usize {
        self.visualization.data().len()
    }

    /// Check if the visualization has no data elements.
    pub fn is_empty(&self) -> bool {
        self.visualization.data().is_empty()
    }

    /// Return whether hover reveal is enabled for this chart.
    pub fn hover_reveal_enabled(&self) -> bool {
        self.config.hover_reveal
    }

    /// Get a reference to the clipped text registry.
    pub fn clipped_text_registry(&self) -> &ClippedTextRegistry {
        &self.clipped_text_registry
    }

    /// Get a mutable reference to the clipped text registry.
    pub fn clipped_text_registry_mut(&mut self) -> &mut ClippedTextRegistry {
        &mut self.clipped_text_registry
    }

    /// Get a reference to the hover reveal state.
    pub fn hover_state(&self) -> &HoverRevealState {
        &self.hover_state
    }

    /// Get a mutable reference to the hover reveal state.
    pub fn hover_state_mut(&mut self) -> &mut HoverRevealState {
        &mut self.hover_state
    }

    /// Update hover reveal state with the current mouse position.
    ///
    /// Call this once per frame (or on mouse-move) with the cursor's
    /// screen-space coordinates and the frame's delta time.
    ///
    /// When hover reveal is disabled this is a no-op.
    pub fn update_hover(&mut self, mouse_x: f32, mouse_y: f32, dt: f32) {
        if self.config.hover_reveal {
            self.hover_state
                .update(&self.clipped_text_registry, mouse_x, mouse_y, dt);
        }
    }

    /// Render the complete chart including axes and grid.
    ///
    /// Axis lines are rendered via each axis's `render()` method. Tick
    /// marks and grid lines are prepared for GPU-instanced rendering
    /// through [`TickPipeline`], which is
    /// lazily created on the first call.
    ///
    /// After calling `render()`, use [`draw_grid_lines`](Self::draw_grid_lines)
    /// followed by [`draw_ticks`](Self::draw_ticks) to record the
    /// instanced draw commands into your render pass.
    pub fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Calculate chart area based on margins and axis requirements
        let chart_area = self.calculate_chart_area();

        // Phase 1: Prepare instanced grid line instances (behind everything)
        if self.config.show_grid {
            self.prepare_grid_pipeline(context, &chart_area);
        }

        // Phase 2: Render main visualization (data points, on top of grid)
        // Note: In a complete implementation, this would use the Mixable render system
        // For now, we acknowledge that the visualization is prepared for rendering

        // Phase 3: Render axes (on top of everything)
        for (position, axis_opt) in [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ] {
            if let Some(axis) = axis_opt {
                let bounds = self.calculate_axis_bounds(position, &chart_area);
                axis.render(context, bounds)?;
            }
        }

        // Phase 4: Prepare instanced tick rendering resources
        self.prepare_tick_pipeline(context);

        Ok(())
    }

    /// Lazily create the `TickPipeline` and upload tick instance data.
    ///
    /// Called automatically by [`render()`](Self::render). After this,
    /// [`draw_ticks()`](Self::draw_ticks) can record the instanced draw
    /// command into a render pass.
    fn prepare_tick_pipeline(&mut self, context: &RenderContext) {
        // Lazily create the tick pipeline
        if self.tick_pipeline.is_none() {
            self.tick_pipeline = Some(TickPipeline::new(
                context.device(),
                context.surface_format(),
            ));
        }

        let geom = self.generate_axis_geometry_instanced();
        let colorbar_geom = self.generate_colorbar_geometry();

        // Merge axis ticks with colorbar ticks.
        let mut all_tick_instances = geom.tick_instances;
        let mut all_line_vertices = geom.line_vertices;

        if let Some(ref cb_geom) = colorbar_geom {
            all_tick_instances.extend_from_slice(&cb_geom.tick_instances);
            all_line_vertices.extend_from_slice(&cb_geom.line_vertices);
        }

        if !all_tick_instances.is_empty() {
            if let Some(tick_pipeline) = &self.tick_pipeline {
                let (base_buf, inst_buf) =
                    tick_pipeline.upload(context.device(), context.queue(), &all_tick_instances);
                self.tick_buffers = Some(TickBuffers {
                    base_buf,
                    inst_buf,
                    instance_count: all_tick_instances.len() as u32,
                });
            }
        } else {
            self.tick_buffers = None;
        }

        // Prepare axis-line pipeline and buffers alongside tick data
        self.prepare_axis_line_pipeline(context, &all_line_vertices);

        // Prepare colorbar gradient pipeline
        if let Some(ref cb_geom) = colorbar_geom {
            if self.colorbar_gradient_pipeline.is_none() {
                self.colorbar_gradient_pipeline = Some(GradientStripPipeline::new(
                    context.device(),
                    context.surface_format(),
                ));
            }
            if !cb_geom.gradient_vertices.is_empty() {
                if let Some(pipeline) = &self.colorbar_gradient_pipeline {
                    let vertex_buf = pipeline.upload(context.device(), &cb_geom.gradient_vertices);
                    self.colorbar_gradient_buffers = Some(GradientStripBuffers {
                        vertex_buf,
                        vertex_count: cb_geom.gradient_vertices.len() as u32,
                    });
                }
            } else {
                self.colorbar_gradient_buffers = None;
            }
        } else {
            self.colorbar_gradient_buffers = None;
        }

        self.colorbar_geometry = colorbar_geom;
    }

    /// Lazily create the [`AxisLinePipeline`] and upload axis-line vertices.
    ///
    /// Called automatically by [`prepare_tick_pipeline()`](Self::prepare_tick_pipeline).
    /// After this, [`draw_axis_lines()`](Self::draw_axis_lines) can record the
    /// draw command into a render pass.
    fn prepare_axis_line_pipeline(&mut self, context: &RenderContext, line_vertices: &[Vertex]) {
        if self.axis_line_pipeline.is_none() {
            self.axis_line_pipeline = Some(AxisLinePipeline::new(
                context.device(),
                context.surface_format(),
            ));
        }

        if !line_vertices.is_empty() {
            if let Some(pipeline) = &self.axis_line_pipeline {
                let vertex_buf = pipeline.upload(context.device(), line_vertices);
                self.axis_line_buffers = Some(AxisLineBuffers {
                    vertex_buf,
                    vertex_count: line_vertices.len() as u32,
                });
            }
        } else {
            self.axis_line_buffers = None;
        }
    }

    /// Record instanced tick draw commands into an active render pass.
    ///
    /// Call this after [`render()`](Self::render) has prepared the tick
    /// pipeline. Returns the number of tick instances drawn (0 if there
    /// are no ticks or the pipeline is not yet initialised).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// chart.render(&mut context)?;
    /// // ... create render pass ...
    /// let ticks_drawn = chart.draw_ticks(&mut render_pass);
    /// ```
    pub fn draw_ticks<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) -> u32 {
        if let (Some(tick_pipeline), Some(bufs)) = (&self.tick_pipeline, &self.tick_buffers) {
            tick_pipeline.draw(
                render_pass,
                &bufs.base_buf,
                &bufs.inst_buf,
                bufs.instance_count,
            );
            bufs.instance_count
        } else {
            0
        }
    }

    /// Returns `true` if the tick pipeline has been initialised and there
    /// are tick instances ready to draw.
    pub fn has_tick_data(&self) -> bool {
        self.tick_buffers
            .as_ref()
            .is_some_and(|b| b.instance_count > 0)
    }

    /// Record axis-line draw commands into an active render pass.
    ///
    /// Call this after [`render()`](Self::render) has prepared the axis-line
    /// pipeline. Returns the number of vertices drawn (0 if there are no
    /// axis lines or the pipeline is not yet initialised).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// chart.render(&mut context)?;
    /// // ... create render pass ...
    /// chart.draw_axis_lines(&mut render_pass);
    /// chart.draw_ticks(&mut render_pass);
    /// ```
    pub fn draw_axis_lines<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) -> u32 {
        if let (Some(pipeline), Some(bufs)) = (&self.axis_line_pipeline, &self.axis_line_buffers) {
            pipeline.draw(render_pass, &bufs.vertex_buf, bufs.vertex_count);
            bufs.vertex_count
        } else {
            0
        }
    }

    /// Returns `true` if the axis-line pipeline has been initialised and
    /// there are axis-line vertices ready to draw.
    pub fn has_axis_line_data(&self) -> bool {
        self.axis_line_buffers
            .as_ref()
            .is_some_and(|b| b.vertex_count > 0)
    }

    // ── Colorbar methods ─────────────────────────────────────────────────

    /// Returns `true` when the colorbar should be rendered.
    ///
    /// A colorbar is shown only when `show_colorbar` is `true` in the
    /// chart config **and** a [`ColorScale`] has been configured.
    pub fn has_colorbar(&self) -> bool {
        self.config.show_colorbar && self.config.color_scale.is_some()
    }

    /// Returns `true` if the colorbar gradient pipeline has been
    /// initialised and there are vertices ready to draw.
    pub fn has_colorbar_data(&self) -> bool {
        self.colorbar_gradient_buffers
            .as_ref()
            .is_some_and(|b| b.vertex_count > 0)
    }

    /// Record colorbar gradient draw commands into an active render pass.
    ///
    /// Call this after [`prepare_draw_commands()`](Self::prepare_draw_commands)
    /// (or [`render()`](Self::render)) has prepared the colorbar pipeline.
    ///
    /// Returns the number of vertices drawn (0 if there is no colorbar).
    pub fn draw_colorbar_gradient<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) -> u32 {
        if let (Some(pipeline), Some(bufs)) = (
            &self.colorbar_gradient_pipeline,
            &self.colorbar_gradient_buffers,
        ) {
            pipeline.draw(render_pass, &bufs.vertex_buf, bufs.vertex_count);
            bufs.vertex_count
        } else {
            0
        }
    }

    /// Access the colorbar geometry (labels, outline, ticks) after
    /// [`prepare_draw_commands()`](Self::prepare_draw_commands).
    ///
    /// Use the returned [`ColorbarGeometry`] to:
    /// - Feed `labels` to a `TextRenderer`
    /// - Add `line_vertices` to the axis-line draw
    /// - Add `tick_instances` to the tick draw
    pub fn colorbar_geometry(&self) -> Option<&colorbar::ColorbarGeometry> {
        self.colorbar_geometry.as_ref()
    }

    /// Prepare the colorbar gradient pipeline and upload buffers.
    ///
    /// Called by [`prepare_draw_commands()`](Self::prepare_draw_commands)
    /// when `has_colorbar()` is true.
    fn generate_colorbar_geometry(&self) -> Option<colorbar::ColorbarGeometry> {
        if !self.has_colorbar() {
            return None;
        }

        let color_scale = self.config.color_scale.clone().unwrap();
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);

        // Position the colorbar to the right of the chart area.
        // Convert pixel coordinates to NDC.
        let w = self.config.width;
        let h = self.config.height;

        // Place the strip 10 px to the right of the chart area,
        // spanning the full chart height.
        let gap_px = 10.0;
        let x_px = chart_area.x + chart_area.width + gap_px;
        let x_ndc = (x_px / w) * 2.0 - 1.0;

        let y_top_px = chart_area.y;
        let y_bot_px = chart_area.y + chart_area.height;
        let y_top_ndc = 1.0 - (y_top_px / h) * 2.0;
        let y_bot_ndc = 1.0 - (y_bot_px / h) * 2.0;

        let renderer = colorbar::ColorbarRenderer::with_defaults(color_scale);
        Some(renderer.generate_geometry(x_ndc, y_bot_ndc, y_top_ndc, viewport_size))
    }

    /// Prepare cached axis-line and tick pipelines from the current chart
    /// geometry.
    ///
    /// Call this once per frame **before** issuing draw commands via
    /// [`draw_axis_lines()`](Self::draw_axis_lines) and
    /// [`draw_ticks()`](Self::draw_ticks). Pipelines are lazily created on
    /// the first call and reused on subsequent frames; only the vertex /
    /// instance buffers are re-uploaded.
    ///
    /// This is the public entry-point for callers that manage their own
    /// render pass (e.g. examples). The private
    /// `prepare_tick_pipeline()` is used
    /// internally by [`render()`](Self::render).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// chart.prepare_draw_commands(&device, &queue, surface_format);
    /// let mut render_pass = /* … */;
    /// chart.draw_axis_lines(&mut render_pass);
    /// chart.draw_ticks(&mut render_pass);
    /// ```
    pub fn prepare_draw_commands(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) {
        // Lazily create pipelines
        if self.tick_pipeline.is_none() {
            self.tick_pipeline = Some(TickPipeline::new(device, surface_format));
        }
        if self.axis_line_pipeline.is_none() {
            self.axis_line_pipeline = Some(AxisLinePipeline::new(device, surface_format));
        }

        let geom = self.generate_axis_geometry_instanced();

        // Generate colorbar geometry (if enabled) so its outline and ticks
        // can be merged into the axis buffers.
        let colorbar_geom = self.generate_colorbar_geometry();

        // Merge axis-line vertices with colorbar outline.
        let mut all_line_vertices = geom.line_vertices;
        let mut all_tick_instances = geom.tick_instances;

        if let Some(ref cb_geom) = colorbar_geom {
            all_line_vertices.extend_from_slice(&cb_geom.line_vertices);
            all_tick_instances.extend_from_slice(&cb_geom.tick_instances);
        }

        // Axis-line vertices (includes colorbar outline)
        if !all_line_vertices.is_empty() {
            if let Some(pipeline) = &self.axis_line_pipeline {
                let vertex_buf = pipeline.upload(device, &all_line_vertices);
                self.axis_line_buffers = Some(AxisLineBuffers {
                    vertex_buf,
                    vertex_count: all_line_vertices.len() as u32,
                });
            }
        } else {
            self.axis_line_buffers = None;
        }

        // Tick instances (includes colorbar ticks)
        if !all_tick_instances.is_empty() {
            if let Some(tick_pipeline) = &self.tick_pipeline {
                let (base_buf, inst_buf) = tick_pipeline.upload(device, queue, &all_tick_instances);
                self.tick_buffers = Some(TickBuffers {
                    base_buf,
                    inst_buf,
                    instance_count: all_tick_instances.len() as u32,
                });
            }
        } else {
            self.tick_buffers = None;
        }

        // Colorbar gradient strip (separate triangle-list pipeline)
        if let Some(ref cb_geom) = colorbar_geom {
            if self.colorbar_gradient_pipeline.is_none() {
                self.colorbar_gradient_pipeline =
                    Some(GradientStripPipeline::new(device, surface_format));
            }
            if !cb_geom.gradient_vertices.is_empty() {
                if let Some(pipeline) = &self.colorbar_gradient_pipeline {
                    let vertex_buf = pipeline.upload(device, &cb_geom.gradient_vertices);
                    self.colorbar_gradient_buffers = Some(GradientStripBuffers {
                        vertex_buf,
                        vertex_count: cb_geom.gradient_vertices.len() as u32,
                    });
                }
            } else {
                self.colorbar_gradient_buffers = None;
            }
        } else {
            self.colorbar_gradient_buffers = None;
        }

        self.colorbar_geometry = colorbar_geom;
    }

    /// Prepare grid line instance buffers for GPU-instanced rendering.
    ///
    /// Called automatically by [`render()`](Self::render) when grids are
    /// enabled. After this, [`draw_grid_lines()`](Self::draw_grid_lines)
    /// can record the instanced draw command into a render pass.
    fn prepare_grid_pipeline(&mut self, context: &RenderContext, chart_area: &ChartArea) {
        use crate::grid::ChartBounds;

        // Ensure tick pipeline exists (shared with tick marks)
        if self.tick_pipeline.is_none() {
            self.tick_pipeline = Some(TickPipeline::new(
                context.device(),
                context.surface_format(),
            ));
        }

        let chart_bounds = ChartBounds::new(
            chart_area.x,
            chart_area.x + chart_area.width,
            chart_area.y,
            chart_area.y + chart_area.height,
        );

        let horizontal_ticks = Self::generate_sample_horizontal_ticks(chart_bounds);
        let vertical_ticks = Self::generate_sample_vertical_ticks(chart_bounds);
        let horizontal_minor_ticks: Vec<f64> = Vec::new();
        let vertical_minor_ticks: Vec<f64> = Vec::new();

        if let Some(grid_system) = &mut self.grid_system {
            let instances = grid_system.generate_grid_instances(
                &horizontal_ticks,
                &vertical_ticks,
                &horizontal_minor_ticks,
                &vertical_minor_ticks,
                chart_bounds,
            );

            if !instances.is_empty() {
                if let Some(tick_pipeline) = &self.tick_pipeline {
                    let (base_buf, inst_buf) =
                        tick_pipeline.upload(context.device(), context.queue(), instances);
                    self.grid_buffers = Some(TickBuffers {
                        base_buf,
                        inst_buf,
                        instance_count: instances.len() as u32,
                    });
                }
            } else {
                self.grid_buffers = None;
            }
        }
    }

    /// Record instanced grid line draw commands into an active render pass.
    ///
    /// Call this after [`render()`](Self::render) has prepared the grid
    /// pipeline, and **before** [`draw_ticks()`](Self::draw_ticks) so that
    /// grid lines appear behind tick marks.
    ///
    /// Returns the number of grid line instances drawn.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// chart.render(&mut context)?;
    /// // ... create render pass ...
    /// chart.draw_grid_lines(&mut render_pass);
    /// chart.draw_ticks(&mut render_pass);
    /// ```
    pub fn draw_grid_lines<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) -> u32 {
        if let (Some(tick_pipeline), Some(bufs)) = (&self.tick_pipeline, &self.grid_buffers) {
            tick_pipeline.draw(
                render_pass,
                &bufs.base_buf,
                &bufs.inst_buf,
                bufs.instance_count,
            );
            bufs.instance_count
        } else {
            0
        }
    }

    /// Returns `true` if the grid pipeline has been initialised and there
    /// are grid line instances ready to draw.
    pub fn has_grid_data(&self) -> bool {
        self.grid_buffers
            .as_ref()
            .is_some_and(|b| b.instance_count > 0)
    }

    /// Render grid lines via the legacy `LineAttributes` path.
    ///
    /// Superseded by [`prepare_grid_pipeline`](Self::prepare_grid_pipeline)
    /// which uses GPU instancing.  Retained for reference and fallback.
    #[allow(dead_code)]
    fn render_grid_lines_static(
        grid_system: &mut crate::grid::GridSystem,
        context: &mut RenderContext,
        chart_area: &ChartArea,
    ) -> GupResult<()> {
        use crate::grid::ChartBounds;

        // Convert ChartArea to ChartBounds for grid system
        let chart_bounds = ChartBounds::new(
            chart_area.x,
            chart_area.x + chart_area.width,
            chart_area.y,
            chart_area.y + chart_area.height,
        );

        // Generate sample tick positions (in a complete implementation, these would come from the axes)
        let horizontal_ticks = Self::generate_sample_horizontal_ticks(chart_bounds);
        let vertical_ticks = Self::generate_sample_vertical_ticks(chart_bounds);
        let horizontal_minor_ticks = Vec::new(); // No minor ticks for now
        let vertical_minor_ticks = Vec::new();

        // Render the grid
        grid_system.render_grid(
            context,
            &horizontal_ticks,
            &vertical_ticks,
            &horizontal_minor_ticks,
            &vertical_minor_ticks,
            chart_bounds,
        )?;

        // For now, we'll skip the visual rendering of grid selections
        // In a complete implementation, this would use the passed context
        // and render the grid selections through the proper rendering pipeline
        println!(
            "Grid system generated {} total grid lines",
            grid_system.total_line_count()
        );

        Ok(())
    }

    /// Generate sample horizontal tick positions.
    fn generate_sample_horizontal_ticks(bounds: crate::grid::ChartBounds) -> Vec<f64> {
        let mut ticks = Vec::new();
        let step = bounds.width() / 5.0; // 5 major divisions
        for i in 0..=5 {
            ticks.push((bounds.left + i as f32 * step) as f64);
        }
        ticks
    }

    /// Generate sample vertical tick positions.
    fn generate_sample_vertical_ticks(bounds: crate::grid::ChartBounds) -> Vec<f64> {
        let mut ticks = Vec::new();
        let step = bounds.height() / 4.0; // 4 major divisions
        for i in 0..=4 {
            ticks.push((bounds.top + i as f32 * step) as f64);
        }
        ticks
    }

    /// Calculate the available chart area after accounting for margins and axes.
    fn calculate_chart_area(&self) -> ChartArea {
        let total_width = self.config.width;
        let total_height = self.config.height;

        let mut margins = self.config.margins;

        // Adjust margins for axes
        if let Some(axis) = &self.bottom_axis {
            margins.bottom += axis.calculate_margin(None);
        }
        if let Some(axis) = &self.left_axis {
            margins.left += axis.calculate_margin(None);
        }
        if let Some(axis) = &self.top_axis {
            margins.top += axis.calculate_margin(None);
        }
        if let Some(axis) = &self.right_axis {
            margins.right += axis.calculate_margin(None);
        }

        ChartArea {
            x: margins.left,
            y: margins.top,
            width: total_width - margins.left - margins.right,
            height: total_height - margins.top - margins.bottom,
            margins,
        }
    }

    /// Calculate axis bounds for a specific position (in pixel coordinates).
    fn calculate_axis_bounds(&self, position: AxisPosition, chart_area: &ChartArea) -> AxisBounds {
        match position {
            AxisPosition::Bottom => AxisBounds::new(
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y + chart_area.height,
                },
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y + chart_area.height,
                },
                chart_area.margins.bottom,
            ),
            AxisPosition::Left => AxisBounds::new(
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y + chart_area.height,
                },
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y,
                },
                chart_area.margins.left,
            ),
            AxisPosition::Top => AxisBounds::new(
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y,
                },
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y,
                },
                chart_area.margins.top,
            ),
            AxisPosition::Right => AxisBounds::new(
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y + chart_area.height,
                },
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y,
                },
                chart_area.margins.right,
            ),
        }
    }

    /// Convert pixel-space [`AxisBounds`] to NDC (clip space, -1.0 to 1.0).
    ///
    /// Uses the chart's configured `width` and `height` as the viewport
    /// dimensions for the conversion:
    /// - `ndc_x = (pixel_x / width) * 2.0 - 1.0`
    /// - `ndc_y = 1.0 - (pixel_y / height) * 2.0`
    fn pixel_bounds_to_ndc(&self, bounds: &AxisBounds) -> AxisBounds {
        let w = self.config.width;
        let h = self.config.height;
        AxisBounds::new(
            Vec2 {
                x: (bounds.start.x / w) * 2.0 - 1.0,
                y: 1.0 - (bounds.start.y / h) * 2.0,
            },
            Vec2 {
                x: (bounds.end.x / w) * 2.0 - 1.0,
                y: 1.0 - (bounds.end.y / h) * 2.0,
            },
            bounds.available_margin,
        )
    }

    /// Generate axis geometry (vertices and labels) for all configured axes.
    ///
    /// Returns a tuple of `(vertices, labels)` where:
    /// - `vertices`: `Vec<Vertex>` suitable for drawing with a `LineList`
    ///   pipeline. Each pair of consecutive vertices forms one line segment
    ///   (axis lines and tick marks).
    /// - `labels`: `Vec<AxisLabel>` with formatted text, screen position, and
    ///   recommended anchor for each tick label, ready for
    ///   [`TextRenderer`](crate::text::TextRenderer).
    ///
    /// The method converts the chart's pixel-space layout (from
    /// [`ChartConfig`] margins and dimensions) into NDC coordinates before
    /// delegating to [`AxisRenderer`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::axis::{LinearAxis, AxisPosition, AxisConfiguration};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// # let config = gup::chart_builder::ChartConfig::default();
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    /// let (vertices, labels) = chart.generate_axis_geometry();
    ///
    /// // Draw `vertices` with a LineList pipeline, render `labels` with TextRenderer
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_axis_geometry(&self) -> (Vec<Vertex>, Vec<AxisLabel>) {
        self.generate_axis_geometry_instanced().into_legacy()
    }

    /// Generate axis geometry with instanced tick data.
    ///
    /// Returns an [`AxisGeometry`] struct that separates axis line vertices
    /// from tick instance data.  Axis lines are in
    /// [`AxisGeometry::line_vertices`] (rendered with a `LineList` pipeline)
    /// and tick marks are in [`AxisGeometry::tick_instances`] (rendered via
    /// [`TickPipeline`]).
    ///
    /// This method produces the same visual result as
    /// [`generate_axis_geometry`](Self::generate_axis_geometry), but enables
    /// callers to use the more efficient instanced rendering path introduced
    /// in GUP-204.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::axis::{LinearAxis, AxisPosition, AxisConfiguration};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// # let config = gup::chart_builder::ChartConfig::default();
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    /// let geom = chart.generate_axis_geometry_instanced();
    ///
    /// // `geom.line_vertices` — draw with LineList pipeline
    /// // `geom.tick_instances` — draw with TickPipeline
    /// // `geom.labels` — render with TextRenderer
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_axis_geometry_instanced(&self) -> AxisGeometry {
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        // Build tick_generator Scale objects from the shader-function
        // scales when present, so axis tick generation uses the correct domain.
        // For LogScale, LogarithmicScale produces properly log-spaced ticks.
        let x_tick_scale = self.config.x_scale.as_ref().map(|s| s.to_tick_scale());
        let y_tick_scale = self.config.y_scale.as_ref().map(|s| s.to_tick_scale());

        let mut all_line_vertices = Vec::new();
        let mut all_tick_instances = Vec::new();
        let mut all_labels = Vec::new();

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                // Select the appropriate tick scale for this axis position.
                let tick_scale: Option<&dyn crate::tick_generator::Scale> = match position {
                    AxisPosition::Bottom | AxisPosition::Top => x_tick_scale
                        .as_ref()
                        .map(|s| &**s as &dyn crate::tick_generator::Scale),
                    AxisPosition::Left | AxisPosition::Right => y_tick_scale
                        .as_ref()
                        .map(|s| &**s as &dyn crate::tick_generator::Scale),
                };

                // Axis line vertices (LineList)
                let line_verts = renderer.generate_line_vertices(&ndc_bounds, config);
                all_line_vertices.extend(line_verts);

                // Tick instances (GPU instancing)
                let tick_insts = renderer.generate_tick_instances(
                    &ndc_bounds,
                    config,
                    *position,
                    tick_scale,
                    viewport_size,
                );
                all_tick_instances.extend(tick_insts);

                // Select the custom label formatter for this axis, if any.
                let label_formatter = self.config.label_formatter_for(*position);

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    tick_scale,
                    viewport_size,
                    label_formatter,
                );
                all_labels.extend(labels);
            }
        }

        AxisGeometry {
            line_vertices: all_line_vertices,
            tick_instances: all_tick_instances,
            labels: all_labels,
        }
    }

    /// Generate axis geometry with label collision resolution.
    ///
    /// Like [`generate_axis_geometry`](Self::generate_axis_geometry), but runs
    /// each axis's labels through the given [`LabelPositioner`] to resolve
    /// overlaps.  Labels that cannot be placed without collisions may be
    /// offset, rotated, or hidden depending on the positioner's strategy
    /// pipeline.
    ///
    /// The positioner accumulates state across axes, so labels from one axis
    /// will not overlap labels already placed by a previous axis.
    ///
    /// Returns `(vertices, layout)` where `layout` contains the resolved
    /// [`LabelPosition`] entries and a list of hidden label indices.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::axis::{LinearAxis, AxisPosition, AxisConfiguration};
    /// use gup::label::{LabelPositioner, LabelConstraints};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// # let config = gup::chart_builder::ChartConfig::default();
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    /// let mut positioner = LabelPositioner::new();
    /// let constraints = LabelConstraints::axis_labels();
    /// let (vertices, layout) = chart.generate_axis_geometry_resolved(
    ///     &mut positioner,
    ///     &constraints,
    /// )?;
    ///
    /// // `layout.positions` contains collision-free label placements
    /// // `layout.hidden_labels` lists indices that were hidden
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_axis_geometry_resolved(
        &self,
        positioner: &mut LabelPositioner,
        constraints: &LabelConstraints,
    ) -> GupResult<(Vec<Vertex>, LabelLayout)> {
        let (geom, layout) =
            self.generate_axis_geometry_instanced_resolved(positioner, constraints)?;
        let (vertices, _labels) = geom.into_legacy();
        Ok((vertices, layout))
    }

    /// Generate instanced axis geometry with label collision resolution.
    ///
    /// Combines [`generate_axis_geometry_instanced`](Self::generate_axis_geometry_instanced)
    /// with label collision resolution via the given [`LabelPositioner`].
    ///
    /// Returns `(geometry, layout)` where `geometry` contains separated line
    /// vertices and tick instances, and `layout` contains the resolved
    /// [`LabelPosition`] entries and a list of hidden label indices.
    pub fn generate_axis_geometry_instanced_resolved(
        &self,
        positioner: &mut LabelPositioner,
        constraints: &LabelConstraints,
    ) -> GupResult<(AxisGeometry, LabelLayout)> {
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        // Build tick_generator Scale objects from the shader-function
        // scales when present, so axis tick generation uses the correct domain.
        let x_tick_scale = self.config.x_scale.as_ref().map(|s| s.to_tick_scale());
        let y_tick_scale = self.config.y_scale.as_ref().map(|s| s.to_tick_scale());

        let mut all_line_vertices = Vec::new();
        let mut all_tick_instances = Vec::new();
        let mut all_positions: Vec<LabelPosition> = Vec::new();
        let mut all_hidden: Vec<usize> = Vec::new();
        let mut offset = 0usize;
        let mut any_rotated = false;

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                // Select the appropriate tick scale for this axis position.
                let tick_scale: Option<&dyn crate::tick_generator::Scale> = match position {
                    AxisPosition::Bottom | AxisPosition::Top => x_tick_scale
                        .as_ref()
                        .map(|s| &**s as &dyn crate::tick_generator::Scale),
                    AxisPosition::Left | AxisPosition::Right => y_tick_scale
                        .as_ref()
                        .map(|s| &**s as &dyn crate::tick_generator::Scale),
                };

                // Axis line vertices (LineList)
                let line_verts = renderer.generate_line_vertices(&ndc_bounds, config);
                all_line_vertices.extend(line_verts);

                // Tick instances (GPU instancing)
                let tick_insts = renderer.generate_tick_instances(
                    &ndc_bounds,
                    config,
                    *position,
                    tick_scale,
                    viewport_size,
                );
                all_tick_instances.extend(tick_insts);

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    tick_scale,
                    viewport_size,
                    self.config.label_formatter_for(*position),
                );

                // Build AxisInfo from pixel-space bounds for this axis
                let axis_info = AxisInfo::from_bounds(&pixel_bounds, *position);

                let layout = positioner.resolve_labels(&labels, &axis_info, constraints)?;

                // Remap hidden indices relative to the combined list
                for &idx in &layout.hidden_labels {
                    all_hidden.push(offset + idx);
                }
                any_rotated |= layout.rotated;
                offset += labels.len();
                all_positions.extend(layout.positions);
            }
        }

        let combined = LabelLayout {
            positions: all_positions,
            hidden_labels: all_hidden,
            margin_requirements: crate::label::Margins::default(),
            rotated: any_rotated,
        };

        let geom = AxisGeometry {
            line_vertices: all_line_vertices,
            tick_instances: all_tick_instances,
            labels: Vec::new(), // Labels are in the LabelLayout
        };

        Ok((geom, combined))
    }

    /// Queue axis-label and title text for multi-font rendering.
    ///
    /// This convenience method generates axis labels for each axis and
    /// queues them (and the optional chart title) through
    /// [`TextRenderer::queue_text_with_fonts`](crate::text::TextRenderer::queue_text_with_fonts),
    /// so that [`TextStyle::font_family`] is respected via the
    /// [`FontAtlasManager`](crate::text::FontAtlasManager).
    ///
    /// When an axis has a per-axis
    /// [`AxisConfiguration::label_style`](crate::axis::AxisConfiguration::label_style),
    /// that style is used instead of [`ChartConfig::label_style`].
    ///
    /// Call this **before** creating the render pass, then call
    /// [`TextRenderer::render_queued_text_multi`](crate::text::TextRenderer::render_queued_text_multi)
    /// inside the pass to draw the text.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::text::{
    ///     FontAtlasManager, FontDatabase, TextLayoutEngine, TextRenderer, TextStyle,
    /// };
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = std::sync::Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// let config = gup::chart_builder::ChartConfig::default()
    ///     .with_label_style(TextStyle::new(14.0).with_font_family("DejaVu Sans"))
    ///     .with_title("My Chart")
    ///     .with_title_style(TextStyle::new(18.0).bold().with_font_family("DejaVu Serif"));
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    ///
    /// // During frame rendering:
    /// // text_renderer.begin_frame();
    /// // chart.queue_chart_text(&frame, &mut text_renderer, &mut font_mgr, &mut layout)?;
    /// // let mut pass = frame.render_pass(None);
    /// // text_renderer.render_queued_text_multi(&mut pass, device, queue, &font_mgr, w, h)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn queue_chart_text(
        &mut self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
    ) -> GupResult<()> {
        let hover_reveal = self.config.hover_reveal;

        // Clear registry at start of each frame when hover reveal is active
        if hover_reveal {
            self.clipped_text_registry.clear();
        }

        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        let clipping_config = crate::text::ClippingStrategyConfig {
            enable_hover_reveal: true,
            ..crate::text::ClippingStrategyConfig::default()
        };

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    None,
                    viewport_size,
                    self.config.label_formatter_for(*position),
                );

                // Per-axis style overrides chart-level style
                let base_style = config
                    .label_style
                    .as_ref()
                    .unwrap_or(&self.config.label_style);

                for label in &labels {
                    let style = base_style.clone().with_anchor(label.anchor);

                    if hover_reveal {
                        let vp_bounds =
                            Self::label_viewport_bounds(*position, &chart_area, &self.config);
                        let layout_result = text_renderer.queue_text_with_fonts_layout(
                            frame,
                            &label.text,
                            label.screen_position,
                            &style,
                            font_manager,
                            layout_engine,
                            Some(&vp_bounds),
                            Some(&clipping_config),
                        )?;

                        // Register clipped text for hover reveal
                        if let Some(original) = &layout_result.original_text {
                            self.clipped_text_registry
                                .register(layout_result.bounds, original);
                        }
                    } else {
                        text_renderer.queue_text_with_fonts(
                            frame,
                            &label.text,
                            label.screen_position,
                            &style,
                            font_manager,
                            layout_engine,
                            None,
                            None,
                        )?;
                    }
                }
            }
        }

        // Queue chart title
        self.queue_title_text(frame, text_renderer, font_manager, layout_engine)?;

        // Queue colorbar labels (if the colorbar has been prepared)
        if let Some(ref cb_geom) = self.colorbar_geometry {
            for label in &cb_geom.labels {
                let style = self.config.label_style.clone().with_anchor(label.anchor);
                text_renderer.queue_text_with_fonts(
                    frame,
                    &label.text,
                    label.screen_position,
                    &style,
                    font_manager,
                    layout_engine,
                    None,
                    None,
                )?;
            }
        }

        Ok(())
    }

    /// Queue axis-label and title text with collision detection.
    ///
    /// Like [`queue_chart_text`](Self::queue_chart_text), but first runs
    /// labels through a [`LabelPositioner`] to resolve overlaps. Hidden
    /// labels are omitted from the text queue.
    ///
    /// When an axis has a per-axis
    /// [`AxisConfiguration::label_style`](crate::axis::AxisConfiguration::label_style),
    /// that style is used instead of [`ChartConfig::label_style`].
    pub fn queue_chart_text_resolved(
        &mut self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
        positioner: &mut LabelPositioner,
        constraints: &LabelConstraints,
    ) -> GupResult<()> {
        let hover_reveal = self.config.hover_reveal;

        // Clear registry at start of each frame when hover reveal is active
        if hover_reveal {
            self.clipped_text_registry.clear();
        }

        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        let clipping_config = crate::text::ClippingStrategyConfig {
            enable_hover_reveal: true,
            ..crate::text::ClippingStrategyConfig::default()
        };

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    None,
                    viewport_size,
                    self.config.label_formatter_for(*position),
                );

                let axis_info = AxisInfo::from_bounds(&pixel_bounds, *position);
                let layout = positioner.resolve_labels(&labels, &axis_info, constraints)?;

                // Per-axis style overrides chart-level style
                let base_style = config
                    .label_style
                    .as_ref()
                    .unwrap_or(&self.config.label_style);

                for lp in &layout.positions {
                    let style = base_style.clone().with_anchor(lp.anchor);

                    if hover_reveal {
                        let vp_bounds =
                            Self::label_viewport_bounds(*position, &chart_area, &self.config);
                        let layout_result = text_renderer.queue_text_with_fonts_layout(
                            frame,
                            &lp.text,
                            lp.position,
                            &style,
                            font_manager,
                            layout_engine,
                            Some(&vp_bounds),
                            Some(&clipping_config),
                        )?;

                        if let Some(original) = &layout_result.original_text {
                            self.clipped_text_registry
                                .register(layout_result.bounds, original);
                        }
                    } else {
                        text_renderer.queue_text_with_fonts(
                            frame,
                            &lp.text,
                            lp.position,
                            &style,
                            font_manager,
                            layout_engine,
                            None,
                            None,
                        )?;
                    }
                }
            }
        }

        // Queue chart title
        self.queue_title_text(frame, text_renderer, font_manager, layout_engine)?;

        // Queue colorbar labels (if the colorbar has been prepared)
        if let Some(ref cb_geom) = self.colorbar_geometry {
            for label in &cb_geom.labels {
                let style = self.config.label_style.clone().with_anchor(label.anchor);
                text_renderer.queue_text_with_fonts(
                    frame,
                    &label.text,
                    label.screen_position,
                    &style,
                    font_manager,
                    layout_engine,
                    None,
                    None,
                )?;
            }
        }

        Ok(())
    }

    /// Queue the chart title text (if configured).
    ///
    /// Renders the primary title and an optional subtitle, respecting the
    /// alignment, y-offset, and line spacing specified in [`TitleConfig`].
    fn queue_title_text(
        &self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
    ) -> GupResult<()> {
        let title_cfg = match &self.config.title_config {
            Some(cfg) => cfg,
            None => return Ok(()),
        };

        // Determine anchor and x position from alignment
        let (anchor, x) = match title_cfg.alignment {
            TitleAlignment::Left => (crate::text::TextAnchor::TopLeft, self.config.margins.left),
            TitleAlignment::Center => (crate::text::TextAnchor::TopCenter, self.config.width / 2.0),
            TitleAlignment::Right => (
                crate::text::TextAnchor::TopRight,
                self.config.width - self.config.margins.right,
            ),
        };

        let y = title_cfg.y_offset.unwrap_or(self.config.margins.top / 2.0);

        let title_style = self.config.title_style.clone().with_anchor(anchor);

        // Queue the main title
        let title_position = Vec2 { x, y };
        text_renderer.queue_text_with_fonts(
            frame,
            &title_cfg.text,
            title_position,
            &title_style,
            font_manager,
            layout_engine,
            None,
            None,
        )?;

        // Queue subtitle (if present), positioned below the main title
        if let Some(subtitle) = &title_cfg.subtitle {
            let subtitle_y = y + self.config.title_style.font_size * title_cfg.line_spacing;

            let subtitle_style = title_cfg.subtitle_style.clone().with_anchor(anchor);
            let subtitle_position = Vec2 { x, y: subtitle_y };

            text_renderer.queue_text_with_fonts(
                frame,
                subtitle,
                subtitle_position,
                &subtitle_style,
                font_manager,
                layout_engine,
                None,
                None,
            )?;
        }

        Ok(())
    }

    /// Compute viewport bounds for label clipping based on axis position.
    ///
    /// Returns a [`ViewportBounds`](crate::text::ViewportBounds) that
    /// constrains labels to their axis margin area, causing overlong
    /// labels to be clipped and eligible for hover reveal registration.
    fn label_viewport_bounds(
        position: AxisPosition,
        chart_area: &ChartArea,
        config: &ChartConfig,
    ) -> crate::text::ViewportBounds {
        use crate::text::{TextBounds, TextMargins, ViewportBounds};

        let viewport_rect = match position {
            AxisPosition::Bottom => TextBounds::new(
                chart_area.x,
                chart_area.y + chart_area.height,
                chart_area.x + chart_area.width,
                config.height,
            ),
            AxisPosition::Top => TextBounds::new(
                chart_area.x,
                0.0,
                chart_area.x + chart_area.width,
                chart_area.y,
            ),
            AxisPosition::Left => TextBounds::new(
                0.0,
                chart_area.y,
                chart_area.x,
                chart_area.y + chart_area.height,
            ),
            AxisPosition::Right => TextBounds::new(
                chart_area.x + chart_area.width,
                chart_area.y,
                config.width,
                chart_area.y + chart_area.height,
            ),
        };

        ViewportBounds {
            viewport_rect,
            container_bounds: None,
            text_margins: TextMargins::zero(),
        }
    }

    /// Queue tooltip text for the currently active hover reveal.
    ///
    /// If a tooltip is active (the user is hovering over truncated text),
    /// this queues the tooltip text for rendering. Call this after
    /// [`queue_chart_text`](Self::queue_chart_text) so that tooltip text
    /// renders on top of axis labels.
    ///
    /// Returns `true` if a tooltip was queued.
    pub fn queue_tooltip_text(
        &self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
    ) -> GupResult<bool> {
        if !self.config.hover_reveal {
            return Ok(false);
        }

        let tooltip = match self.hover_state.active_tooltip() {
            Some(t) => t,
            None => return Ok(false),
        };

        let tooltip_cfg = &self.config.tooltip_config;
        let style = crate::text::TextStyle::new(tooltip_cfg.font_size)
            .with_rgba(
                tooltip_cfg.text_color[0],
                tooltip_cfg.text_color[1],
                tooltip_cfg.text_color[2],
                tooltip_cfg.text_color[3] * tooltip.opacity,
            )
            .with_anchor(crate::text::TextAnchor::TopLeft);

        text_renderer.queue_text_with_fonts(
            frame,
            &tooltip.text,
            tooltip.position,
            &style,
            font_manager,
            layout_engine,
            None,
            None,
        )?;

        Ok(true)
    }

    // -----------------------------------------------------------------------
    // SVG Export
    // -----------------------------------------------------------------------

    /// Export this chart as an SVG document string.
    ///
    /// Generates axis geometry, grid lines, axis labels, and the chart
    /// title, then passes everything through [`SvgRenderer`](crate::export::svg::SvgRenderer) to produce
    /// a well-formed SVG string.
    ///
    /// Data marks are *not* automatically exported by this method because
    /// the generic `Selection<T, M>` stores mark attributes as closures
    /// that map `T → AttrValue` and cannot be evaluated without a GPU
    /// context.  To include data marks in the SVG, pass pre-built
    /// [`SvgElement`](crate::export::svg::SvgElement) instances via [`export_svg_with_marks`](Self::export_svg_with_marks).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::export::svg::SvgExportOptions;
    /// # use gup::prelude::*;
    /// # use gup::chart_builder::ComposedChart;
    /// # use std::sync::Arc;
    /// # async fn example() -> GupResult<()> {
    /// # let ctx = Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)] struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], ctx)?;
    /// # let config = gup::chart_builder::ChartConfig::default();
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    /// let options = SvgExportOptions::new(800, 600);
    /// let svg = chart.render_to_svg(&options)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn render_to_svg(
        &self,
        options: &crate::export::svg::SvgExportOptions,
    ) -> GupResult<String> {
        self.export_svg_with_marks(options, &[])
    }

    /// Export this chart as an SVG document string, including the given
    /// data-mark elements.
    ///
    /// This is the full-featured export entry point.  Caller-supplied
    /// [`SvgElement`](crate::export::svg::SvgElement) values (typically circles, rects, lines, etc.
    /// constructed from the chart's data) are included in the output
    /// under a `<g class="marks">` group.
    pub fn export_svg_with_marks(
        &self,
        options: &crate::export::svg::SvgExportOptions,
        data_elements: &[crate::export::svg::SvgElement],
    ) -> GupResult<String> {
        let renderer = crate::export::svg::SvgRenderer::new(options.clone());
        let geom = self.generate_axis_geometry_instanced();
        renderer.render(
            &self.config,
            &geom.line_vertices,
            &geom.tick_instances,
            &geom.labels,
            data_elements,
        )
    }

    /// Export this chart as an SVG file.
    ///
    /// Convenience wrapper around [`render_to_svg`](Self::render_to_svg)
    /// that writes the result to the given file path.
    pub fn export_svg(
        &self,
        path: impl AsRef<std::path::Path>,
        options: &crate::export::svg::SvgExportOptions,
    ) -> GupResult<()> {
        let svg = self.render_to_svg(options)?;
        crate::export::svg::write_svg_to_file(&svg, path)
    }

    // -----------------------------------------------------------------
    // PNG export
    // -----------------------------------------------------------------

    /// Render this chart to an off-screen GPU texture at the given pixel
    /// dimensions and return the result as PNG-encoded bytes.
    ///
    /// The method creates a temporary off-screen render target, prepares
    /// and issues all draw commands (axes, ticks, grid lines), reads the
    /// pixel data back through a staging buffer, and encodes it as a
    /// lossless PNG with RGBA colour.
    ///
    /// The chart's [`Selection`] must have been created with a
    /// [`RenderContext`] (i.e. `context()` returns `Some`).
    ///
    /// # Errors
    ///
    /// Returns an error if no GPU context is available, or if the GPU
    /// readback or PNG encoding fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let png_bytes = chart.render_to_png(800, 600)?;
    /// std::fs::write("chart.png", &png_bytes)?;
    /// ```
    pub fn render_to_png(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>> {
        let context =
            self.visualization
                .context()
                .cloned()
                .ok_or_else(|| GupError::RenderError {
                    message: "Cannot export PNG: chart has no GPU RenderContext".to_string(),
                })?;

        let device = context.device();
        let queue = context.queue();
        let surface_format = context.surface_format();

        // 1. Prepare GPU buffers for drawing.
        self.prepare_draw_commands(device, queue, surface_format);

        // 2. Create off-screen render target.
        let target = crate::export::png::OffscreenTarget::new(device, width, height);

        // 3. Create command encoder and render pass.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("png_export_render_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("png_export_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 4. Issue all draw commands.
            self.draw_grid_lines(&mut render_pass);
            self.draw_axis_lines(&mut render_pass);
            self.draw_ticks(&mut render_pass);
        }

        // 5. Submit rendering commands.
        queue.submit(std::iter::once(encoder.finish()));

        // 6. Read back the rendered pixels and encode as PNG.
        target.readback_as_png(device, queue)
    }

    /// Render the chart directly into the provided [`TextureView`].
    ///
    /// This is the zero-copy rendering path: all draw commands target the
    /// supplied view and are submitted to the GPU queue.  No readback or
    /// encoding takes place — the rendered pixels stay on the GPU.
    ///
    /// `surface_format` must match the format of the target texture so that
    /// render pipelines are created with a compatible colour target.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let view = my_texture.create_view(&Default::default());
    /// chart.render_to_texture_view(
    ///     &view,
    ///     wgpu::TextureFormat::Bgra8UnormSrgb,
    ///     800,
    ///     600,
    /// )?;
    /// ```
    pub fn render_to_texture_view(
        &mut self,
        view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        _width: u32,
        _height: u32,
    ) -> GupResult<()> {
        let context =
            self.visualization
                .context()
                .cloned()
                .ok_or_else(|| GupError::RenderError {
                    message: "Cannot render to texture: chart has no GPU RenderContext".to_string(),
                })?;

        let device = context.device();
        let queue = context.queue();

        // 1. Prepare GPU buffers for drawing.
        self.prepare_draw_commands(device, queue, surface_format);

        // 2. Create command encoder and render pass targeting the caller's view.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gup_texture_render_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gup_texture_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 3. Issue all draw commands.
            self.draw_grid_lines(&mut render_pass);
            self.draw_axis_lines(&mut render_pass);
            self.draw_ticks(&mut render_pass);
        }

        // 4. Submit rendering commands.
        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Render this chart to PNG at a logical size scaled by the given
    /// factor.
    ///
    /// The physical pixel dimensions of the output are
    /// `(logical_width × scale, logical_height × scale)`, producing a
    /// HiDPI/Retina-ready image when `scale > 1.0`.
    ///
    /// A scale of `1.0` is equivalent to
    /// [`render_to_png(logical_width, logical_height)`](Self::render_to_png).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // 2× Retina export at logical 400×300
    /// let png = chart.render_to_png_scaled(400, 300, 2.0)?;
    /// // Output image will be 800×600 pixels.
    /// ```
    pub fn render_to_png_scaled(
        &mut self,
        logical_width: u32,
        logical_height: u32,
        scale: f32,
    ) -> GupResult<Vec<u8>> {
        let phys_w = (logical_width as f32 * scale).round() as u32;
        let phys_h = (logical_height as f32 * scale).round() as u32;
        self.render_to_png(phys_w, phys_h)
    }

    /// Render this chart to PNG and write the result to a file.
    ///
    /// Convenience wrapper around [`render_to_png`](Self::render_to_png)
    /// that writes the PNG bytes directly to disk.
    ///
    /// # Errors
    ///
    /// Returns a [`GupError::FileError`] if the file cannot be written.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// chart.export_png("chart.png", 2400, 1600)?;
    /// ```
    pub fn export_png(
        &mut self,
        path: impl AsRef<std::path::Path>,
        width: u32,
        height: u32,
    ) -> GupResult<()> {
        let png_bytes = self.render_to_png(width, height)?;
        let path = path.as_ref();
        std::fs::write(path, &png_bytes).map_err(|e| GupError::FileError {
            path: path.display().to_string(),
            error: e.to_string(),
        })
    }

    // -----------------------------------------------------------------
    // HTML export
    // -----------------------------------------------------------------

    /// Export this chart as a self-contained HTML file.
    ///
    /// Uses the [`WasmStrategy::Url`](crate::export::WasmStrategy::Url) strategy with a default path of
    /// `"gup.wasm"`.  For full control over the WASM embedding strategy,
    /// page title, description, and author metadata, create an
    /// [`HtmlExporter`](crate::export::html::HtmlExporter) directly.
    ///
    /// The exported HTML contains:
    /// * An embedded SVG fallback for browsers without WebGPU.
    /// * A PNG thumbnail in Open Graph `<meta>` tags.
    /// * The chart definition as a JSON block.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// chart.export_html("chart.html")?;
    /// ```
    pub fn export_html(&mut self, path: impl AsRef<std::path::Path>) -> GupResult<()> {
        let exporter = crate::export::html::HtmlExporter::new(
            crate::export::html::WasmStrategy::Url("gup.wasm".to_string()),
        );
        exporter.export(self, path)
    }

    // -----------------------------------------------------------------
    // PDF export
    // -----------------------------------------------------------------

    /// Export this chart as a single-page PDF file.
    ///
    /// Convenience method for the common single-chart case.  Uses the
    /// SVG intermediate representation — no GPU commands are issued.
    ///
    /// Data marks must be provided separately as `SvgElement` values,
    /// exactly as with [`export_svg_with_marks`](Self::export_svg_with_marks).
    ///
    /// # Feature Gate
    ///
    /// Requires the **`pdf`** Cargo feature.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use gup::export::pdf::PdfOptions;
    ///
    /// chart.export_pdf("report.pdf", PdfOptions::a4())?;
    /// ```
    #[cfg(feature = "pdf")]
    pub fn export_pdf(
        &self,
        path: impl AsRef<std::path::Path>,
        options: crate::export::pdf::PdfOptions,
    ) -> GupResult<()> {
        self.export_pdf_with_marks(path, options, &[])
    }

    /// Export this chart as a single-page PDF file, including the given
    /// data-mark elements.
    ///
    /// # Feature Gate
    ///
    /// Requires the **`pdf`** Cargo feature.
    #[cfg(feature = "pdf")]
    pub fn export_pdf_with_marks(
        &self,
        path: impl AsRef<std::path::Path>,
        options: crate::export::pdf::PdfOptions,
        data_elements: &[crate::export::svg::SvgElement],
    ) -> GupResult<()> {
        let svg_opts = crate::export::svg::SvgExportOptions::new(800, 600);
        let geom = self.generate_axis_geometry_instanced();

        // Build SVG elements for chart structure.
        let svg_renderer = crate::export::svg::SvgRenderer::new(svg_opts.clone());
        let svg_string = svg_renderer.render(
            &self.config,
            &geom.line_vertices,
            &geom.tick_instances,
            &geom.labels,
            data_elements,
        )?;
        // We use the SVG approach to build elements, then convert to PDF.
        // Since we can't easily parse the SVG string back, we reconstruct
        // elements for the PDF renderer.
        let transform =
            crate::export::svg::ClipToSvg::new(svg_opts.width as f32, svg_opts.height as f32);

        let mut all_elements: Vec<crate::export::svg::SvgElement> = Vec::new();

        // Background
        let bg = self.config.background_color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
        all_elements.push(crate::export::svg::SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: svg_opts.width as f32,
            height: svg_opts.height as f32,
            fill: crate::export::svg::element::rgba_to_css(bg[0], bg[1], bg[2], bg[3]),
            stroke: None,
            stroke_width: None,
            rx: None,
        });

        // Axis lines
        for pair in geom.line_vertices.chunks_exact(2) {
            let (x1, y1) = transform.point(pair[0].position[0], pair[0].position[1]);
            let (x2, y2) = transform.point(pair[1].position[0], pair[1].position[1]);
            let color = crate::export::svg::element::rgba_to_css(
                pair[0].color[0],
                pair[0].color[1],
                pair[0].color[2],
                pair[0].color[3],
            );
            all_elements.push(crate::export::svg::SvgElement::Line {
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
        for inst in &geom.tick_instances {
            let (x1, y1) = transform.point(inst.position[0], inst.position[1]);
            let (x2, y2) = transform.point(
                inst.position[0] + inst.tick_vector[0],
                inst.position[1] + inst.tick_vector[1],
            );
            let color = crate::export::svg::element::rgba_to_css(
                inst.color[0],
                inst.color[1],
                inst.color[2],
                inst.color[3],
            );
            all_elements.push(crate::export::svg::SvgElement::Line {
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
        for label in &geom.labels {
            all_elements.push(crate::export::svg::SvgElement::Text {
                x: label.screen_position.x,
                y: label.screen_position.y,
                content: label.text.clone(),
                font_family: "sans-serif".to_string(),
                font_size: self.config.label_style.font_size,
                text_anchor: "start".to_string(),
                dominant_baseline: "central".to_string(),
                fill: crate::export::svg::element::rgba_to_css(
                    self.config.label_style.color.x,
                    self.config.label_style.color.y,
                    self.config.label_style.color.z,
                    self.config.label_style.color.w,
                ),
                font_weight: None,
            });
        }

        // Data marks
        all_elements.extend_from_slice(data_elements);

        // Ignore the SVG string — it was only generated to validate.
        let _ = svg_string;

        let mut doc = crate::export::pdf::PdfDocument::new(options);
        doc.add_page_from_elements(
            "Chart",
            &all_elements,
            svg_opts.width as f32,
            svg_opts.height as f32,
        )?;
        doc.write(path)
    }
}

#[derive(Debug, Clone)]
struct ChartArea {
    /// X position of chart area
    pub x: f32,
    /// Y position of chart area
    pub y: f32,
    /// Width of chart area
    pub width: f32,
    /// Height of chart area
    pub height: f32,
    /// Final margins used
    pub margins: Margins,
}

// Render layer manager for proper z-ordering of visual elements.
//
// This manager ensures that visual elements are rendered in the correct order:
// 1. Background layer (chart background)
// 2. Grid layer (grid lines behind data)
// 3. Data layer (main visualization data)
// 4. Axis layer (axes on top of data)
// 5. Annotation layer (labels, legends, etc.)
//
// TODO: This is currently a stub pending full Selection type implementation
/*
#[derive(Debug)]
pub struct RenderLayerManager {
    /// Background layer elements
    background_layer: Vec<Box<dyn RenderableElement>>,
    /// Grid layer elements (grid lines)
    grid_layer: Vec<Selection<LineAttributes, crate::selection::Line>>,
    /// Data layer elements (main visualization)
    data_layer: Vec<Box<dyn RenderableElement>>,
    /// Axis layer elements
    axis_layer: Vec<Box<dyn RenderableElement>>,
    /// Annotation layer elements
    annotation_layer: Vec<Box<dyn RenderableElement>>,
}

/// Trait for elements that can be rendered in layers.
pub trait RenderableElement: std::fmt::Debug {
    /// Render this element using the provided context.
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()>;
}

impl RenderLayerManager {
    /// Create a new empty render layer manager.
    pub fn new() -> Self {
        Self {
            background_layer: Vec::new(),
            grid_layer: Vec::new(),
            data_layer: Vec::new(),
            axis_layer: Vec::new(),
            annotation_layer: Vec::new(),
        }
    }

    /// Add grid selections to the grid layer.
    pub fn add_grid_selections(
        &mut self,
        selections: Vec<Selection<LineAttributes, crate::selection::Line>>,
    ) {
        self.grid_layer.extend(selections);
    }

    /// Add a data element to the data layer.
    pub fn add_data_element(&mut self, element: Box<dyn RenderableElement>) {
        self.data_layer.push(element);
    }

    /// Add an axis element to the axis layer.
    pub fn add_axis_element(&mut self, element: Box<dyn RenderableElement>) {
        self.axis_layer.push(element);
    }

    /// Render all layers in the correct z-order.
    pub fn render_all_layers(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Layer 1: Background
        for element in &mut self.background_layer {
            element.render(context)?;
        }

        // Layer 2: Grid lines (behind data)
        for selection in &mut self.grid_layer {
            selection.render()?;
        }

        // Layer 3: Data visualization (on top of grid)
        for element in &mut self.data_layer {
            element.render(context)?;
        }

        // Layer 4: Axes (on top of data)
        for element in &mut self.axis_layer {
            element.render(context)?;
        }

        // Layer 5: Annotations (on top of everything)
        for element in &mut self.annotation_layer {
            element.render(context)?;
        }

        Ok(())
    }

    /// Clear all layers.
    pub fn clear(&mut self) {
        self.background_layer.clear();
        self.grid_layer.clear();
        self.data_layer.clear();
        self.axis_layer.clear();
        self.annotation_layer.clear();
    }

    /// Get the total number of elements across all layers.
    pub fn total_element_count(&self) -> usize {
        self.background_layer.len()
            + self.grid_layer.len()
            + self.data_layer.len()
            + self.axis_layer.len()
            + self.annotation_layer.len()
    }
}

impl Default for RenderLayerManager {
    fn default() -> Self {
        Self::new()
    }
}
*/

/// Error types specific to chart building operations.
#[derive(Debug, Clone)]
pub enum ChartBuilderError {
    /// No data provided for chart creation
    EmptyData,

    /// Required accessor function not provided.
    MissingAccessor {
        /// Name of the missing accessor attribute.
        attribute: String,
    },

    /// Incompatible accessor function type.
    IncompatibleAccessor {
        /// Name of the accessor attribute.
        attribute: String,
        /// Expected accessor type.
        expected_type: String,
        /// Actual accessor type provided.
        actual_type: String,
    },

    /// Chart configuration error.
    ConfigurationError {
        /// Description of the configuration error.
        message: String,
    },

    /// Render context initialization failed.
    ContextError {
        /// Underlying rendering error.
        source: GupError,
    },
}

impl std::fmt::Display for ChartBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartBuilderError::EmptyData => {
                write!(f, "Cannot create chart: no data provided")
            }
            ChartBuilderError::MissingAccessor { attribute } => {
                write!(
                    f,
                    "Missing required accessor function for attribute: {attribute}"
                )
            }
            ChartBuilderError::IncompatibleAccessor {
                attribute,
                expected_type,
                actual_type,
            } => {
                write!(
                    f,
                    "Incompatible accessor for {attribute}: expected {expected_type}, got {actual_type}"
                )
            }
            ChartBuilderError::ConfigurationError { message } => {
                write!(f, "Chart configuration error: {message}")
            }
            ChartBuilderError::ContextError { source } => {
                write!(f, "Render context error: {source}")
            }
        }
    }
}

impl std::error::Error for ChartBuilderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChartBuilderError::ContextError { source } => Some(source),
            _ => None,
        }
    }
}

impl From<ChartBuilderError> for GupError {
    fn from(err: ChartBuilderError) -> Self {
        GupError::validation_error(format!("Chart builder error: {err}"))
    }
}

/// Helper trait to convert various accessor types to internal representations.
pub trait IntoAccessor<T> {
    /// The output type of the accessor function
    type Output;

    /// Convert to an internal accessor function representation
    #[allow(clippy::type_complexity)]
    #[cfg(not(target_arch = "wasm32"))]
    fn into_accessor_fn(self) -> Box<dyn Fn(&T) -> Self::Output + Send + Sync>;

    /// Convert to an internal accessor function representation
    #[allow(clippy::type_complexity)]
    #[cfg(target_arch = "wasm32")]
    fn into_accessor_fn(self) -> Box<dyn Fn(&T) -> Self::Output>;
}

// Implementation for closure-based accessors
#[cfg(not(target_arch = "wasm32"))]
impl<T, F, Output> IntoAccessor<T> for F
where
    F: Fn(&T) -> Output + MaybeSend + MaybeSync + 'static,
    Output: MaybeSend + MaybeSync + 'static,
{
    type Output = Output;

    fn into_accessor_fn(self) -> Box<dyn Fn(&T) -> Self::Output + Send + Sync> {
        Box::new(self)
    }
}

// Implementation for closure-based accessors
#[cfg(target_arch = "wasm32")]
impl<T, F, Output> IntoAccessor<T> for F
where
    F: Fn(&T) -> Output + 'static,
    Output: 'static,
{
    type Output = Output;

    fn into_accessor_fn(self) -> Box<dyn Fn(&T) -> Self::Output> {
        Box::new(self)
    }
}

// TODO: Re-enable tests when Selection type is implemented
/*
#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        x: f32,
        y: f32,
        value: f32,
        category: String,
    }

    #[test]
    fn test_chart_config_defaults() {
        let config = ChartConfig::default();
        assert_eq!(config.width, 800.0);
        assert_eq!(config.height, 600.0);
        assert!(config.show_axes);
        assert!(!config.show_grid);
        assert!(config.title_config.is_none());
        assert!(config.background_color.is_none());
    }

    #[test]
    fn test_margins() {
        let uniform_margins = Margins::uniform(20.0);
        assert_eq!(uniform_margins.top, 20.0);
        assert_eq!(uniform_margins.right, 20.0);
        assert_eq!(uniform_margins.bottom, 20.0);
        assert_eq!(uniform_margins.left, 20.0);

        let symmetric_margins = Margins::symmetric(30.0, 40.0);
        assert_eq!(symmetric_margins.top, 30.0);
        assert_eq!(symmetric_margins.right, 40.0);
        assert_eq!(symmetric_margins.bottom, 30.0);
        assert_eq!(symmetric_margins.left, 40.0);
    }

    #[test]
    fn test_axis_scale_scale_value_linear() {
        let scale = AxisScale::Linear(crate::shader_function::LinearScale::new(
            0.0, 100.0, -1.0, 1.0,
        ));
        assert!((scale.scale_value(0.0) - (-1.0)).abs() < 1e-6);
        assert!((scale.scale_value(50.0) - 0.0).abs() < 1e-6);
        assert!((scale.scale_value(100.0) - 1.0).abs() < 1e-6);
        // Extrapolation
        assert!((scale.scale_value(200.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_axis_scale_scale_value_degenerate() {
        // When domain_min == domain_max, return range_min.
        let scale = AxisScale::Linear(crate::shader_function::LinearScale::new(
            5.0, 5.0, -1.0, 1.0,
        ));
        assert!((scale.scale_value(5.0) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_chart_builder_error_display() {
        let error = ChartBuilderError::EmptyData;
        assert_eq!(format!("{error}"), "Cannot create chart: no data provided");

        let error = ChartBuilderError::MissingAccessor {
            attribute: "position".to_string(),
        };
        assert!(format!("{error}").contains("position"));

        let error = ChartBuilderError::IncompatibleAccessor {
            attribute: "color".to_string(),
            expected_type: "Color".to_string(),
            actual_type: "f32".to_string(),
        };
        let error_str = format!("{error}");
        assert!(error_str.contains("color"));
        assert!(error_str.contains("Color"));
        assert!(error_str.contains("f32"));
    }

    #[tokio::test]
    async fn test_bound_chart_builder_data_access() {
        let data = vec![
            TestData {
                x: 1.0,
                y: 2.0,
                value: 10.0,
                category: "A".to_string(),
            },
            TestData {
                x: 3.0,
                y: 4.0,
                value: 20.0,
                category: "B".to_string(),
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        // Create a mock builder for testing
        struct MockBuilder;
        impl ChartBuilder<TestData> for MockBuilder {
            type Output = ();

            fn build_with_data(
                self,
                _data: Vec<TestData>,
                _context: Arc<RenderContext>,
            ) -> GupResult<Self::Output> {
                Ok(())
            }
        }

        let bound_builder = BoundChartBuilder::new(MockBuilder, data.clone(), context);

        assert_eq!(bound_builder.len(), 2);
        assert!(!bound_builder.is_empty());
        assert_eq!(bound_builder.data().len(), 2);
        assert_eq!(bound_builder.data()[0].x, 1.0);
        assert_eq!(bound_builder.data()[1].category, "B");
    }

    #[test]
    fn test_into_accessor_trait() {
        let data = TestData {
            x: 5.0,
            y: 10.0,
            value: 15.0,
            category: "Test".to_string(),
        };

        // Test closure-based accessor
        let accessor = (|d: &TestData| d.x).into_accessor_fn();
        assert_eq!(accessor(&data), 5.0);

        let string_accessor = (|d: &TestData| d.category.clone()).into_accessor_fn();
        assert_eq!(string_accessor(&data), "Test");
    }

    // ---- Tests for pixel_bounds_to_ndc ----

    #[tokio::test]
    async fn test_pixel_bounds_to_ndc_center() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig {
            width: 800.0,
            height: 600.0,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        // Center of an 800x600 viewport is pixel (400, 300) -> NDC (0, 0)
        let pixel_bounds = AxisBounds::new(
            Vec2 { x: 400.0, y: 300.0 },
            Vec2 { x: 400.0, y: 300.0 },
            50.0,
        );
        let ndc = chart.pixel_bounds_to_ndc(&pixel_bounds);
        assert!((ndc.start.x - 0.0).abs() < 0.001);
        assert!((ndc.start.y - 0.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_pixel_bounds_to_ndc_corners() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig {
            width: 800.0,
            height: 600.0,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        // Top-left pixel (0,0) -> NDC (-1, 1)
        let bounds = AxisBounds::new(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 800.0, y: 600.0 }, 10.0);
        let ndc = chart.pixel_bounds_to_ndc(&bounds);
        assert!((ndc.start.x - (-1.0)).abs() < 0.001);
        assert!((ndc.start.y - 1.0).abs() < 0.001);
        // Bottom-right pixel (800,600) -> NDC (1, -1)
        assert!((ndc.end.x - 1.0).abs() < 0.001);
        assert!((ndc.end.y - (-1.0)).abs() < 0.001);
    }

    // ---- Tests for generate_axis_geometry ----

    #[tokio::test]
    async fn test_generate_axis_geometry_no_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig {
            show_axes: false,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        let (vertices, labels) = chart.generate_axis_geometry();
        assert!(vertices.is_empty(), "No axes configured means no vertices");
        assert!(labels.is_empty(), "No axes configured means no labels");
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_default_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let (vertices, labels) = chart.generate_axis_geometry();

        // Default axes: bottom + left, each with line (2 verts) + 6 major ticks (12 verts) = 14 each
        // Total = 28 vertices
        assert_eq!(vertices.len(), 28, "Bottom + left axes: 14 vertices each");

        // 6 labels per axis = 12 total
        assert_eq!(labels.len(), 12, "6 labels per axis x 2 axes");
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_vertices_in_ndc_range() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let (vertices, _) = chart.generate_axis_geometry();

        for v in &vertices {
            assert!(
                v.position[0] >= -1.1 && v.position[0] <= 1.1,
                "X vertex should be near NDC range: got {}",
                v.position[0],
            );
            assert!(
                v.position[1] >= -1.1 && v.position[1] <= 1.1,
                "Y vertex should be near NDC range: got {}",
                v.position[1],
            );
        }
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_labels_in_screen_range() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig::default(); // 800x600
        let chart = ComposedChart::new(sel, config).with_default_axes();

        let (_, labels) = chart.generate_axis_geometry();

        for label in &labels {
            assert!(
                label.screen_position.x >= -50.0 && label.screen_position.x <= 850.0,
                "Label X should be near viewport: got {}",
                label.screen_position.x,
            );
            assert!(
                label.screen_position.y >= -50.0 && label.screen_position.y <= 650.0,
                "Label Y should be near viewport: got {}",
                label.screen_position.y,
            );
        }
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_all_four_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig::default();
        let axis_config = AxisConfiguration::default();
        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )))
            .with_left_axis(Box::new(LinearAxis::new(
                AxisPosition::Left,
                axis_config.clone(),
            )))
            .with_top_axis(Box::new(LinearAxis::new(
                AxisPosition::Top,
                axis_config.clone(),
            )))
            .with_right_axis(Box::new(LinearAxis::new(AxisPosition::Right, axis_config)));

        let (vertices, labels) = chart.generate_axis_geometry();

        // 4 axes * 14 verts each = 56
        assert_eq!(vertices.len(), 56, "4 axes: 14 vertices each");
        // 4 axes * 6 labels each = 24
        assert_eq!(labels.len(), 24, "4 axes: 6 labels each");
    }
}
*/

#[cfg(test)]
mod tests_instanced_ticks {
    use super::*;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        x: f32,
        y: f32,
    }

    // ---- AxisGeometry unit tests (no GPU needed) ----

    #[test]
    fn test_axis_geometry_into_legacy_empty() {
        let geom = AxisGeometry {
            line_vertices: vec![],
            tick_instances: vec![],
            labels: vec![],
        };
        let (verts, labels) = geom.into_legacy();
        assert!(verts.is_empty());
        assert!(labels.is_empty());
    }

    #[test]
    fn test_axis_geometry_into_legacy_preserves_line_vertices() {
        let line_v = Vertex {
            position: [0.0, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let geom = AxisGeometry {
            line_vertices: vec![line_v, line_v],
            tick_instances: vec![],
            labels: vec![],
        };
        let (verts, _) = geom.into_legacy();
        assert_eq!(verts.len(), 2);
        assert_eq!(verts[0], line_v);
    }

    #[test]
    fn test_axis_geometry_into_legacy_expands_tick_instances() {
        use crate::axis::TickInstance;

        let inst = TickInstance {
            position: [0.5, -0.8],
            tick_vector: [0.0, -0.05],
            color: [1.0, 1.0, 1.0, 1.0],
        };

        let geom = AxisGeometry {
            line_vertices: vec![],
            tick_instances: vec![inst],
            labels: vec![],
        };
        let (verts, _) = geom.into_legacy();
        // One tick instance should produce two vertices (start + end)
        assert_eq!(verts.len(), 2);
        assert_eq!(verts[0].position, [0.5, -0.8], "start = instance position");
        assert_eq!(
            verts[1].position,
            [0.5, -0.85],
            "end = position + tick_vector"
        );
        assert_eq!(verts[0].color, inst.color);
        assert_eq!(verts[1].color, inst.color);
    }

    #[test]
    fn test_axis_geometry_into_legacy_combines_lines_and_ticks() {
        use crate::axis::TickInstance;

        let line_v = Vertex {
            position: [-1.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
        };
        let inst = TickInstance {
            position: [0.0, 0.0],
            tick_vector: [0.1, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        };

        let geom = AxisGeometry {
            line_vertices: vec![line_v, line_v],
            tick_instances: vec![inst, inst],
            labels: vec![],
        };
        let (verts, _) = geom.into_legacy();
        // 2 line verts + 2 ticks * 2 verts each = 6
        assert_eq!(verts.len(), 6);
    }

    #[test]
    fn test_axis_geometry_tick_count() {
        use crate::axis::TickInstance;

        let inst = TickInstance {
            position: [0.0, 0.0],
            tick_vector: [0.0, -0.05],
            color: [1.0; 4],
        };
        let geom = AxisGeometry {
            line_vertices: vec![],
            tick_instances: vec![inst; 10],
            labels: vec![],
        };
        assert_eq!(geom.tick_count(), 10);
    }

    // ---- generate_axis_geometry_instanced tests (GPU required) ----

    #[tokio::test]
    async fn test_instanced_no_axes_empty() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig {
            show_axes: false,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);
        let geom = chart.generate_axis_geometry_instanced();
        assert!(geom.line_vertices.is_empty());
        assert!(geom.tick_instances.is_empty());
        assert!(geom.labels.is_empty());
    }

    #[tokio::test]
    async fn test_instanced_default_axes_separates_lines_and_ticks() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();
        let geom = chart.generate_axis_geometry_instanced();

        // Default axes: bottom + left, each with show_line = true -> 2 verts each = 4 line verts
        assert_eq!(geom.line_vertices.len(), 4, "2 axes × 2 line vertices each");

        // Default AxisConfiguration has 6 major ticks per axis, show_minor_ticks = false
        // 2 axes × 6 ticks = 12 tick instances
        assert_eq!(
            geom.tick_instances.len(),
            12,
            "2 axes × 6 major tick instances"
        );

        // Labels
        assert_eq!(geom.labels.len(), 12, "6 labels per axis × 2 axes");
    }

    #[tokio::test]
    async fn test_instanced_matches_legacy_vertex_count() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        // Legacy path
        let (legacy_verts, legacy_labels) = chart.generate_axis_geometry();

        // Instanced path flattened to legacy
        let geom = chart.generate_axis_geometry_instanced();
        let (instanced_verts, instanced_labels) = geom.into_legacy();

        assert_eq!(
            legacy_verts.len(),
            instanced_verts.len(),
            "Legacy and instanced should produce same vertex count"
        );
        assert_eq!(
            legacy_labels.len(),
            instanced_labels.len(),
            "Legacy and instanced should produce same label count"
        );
    }

    #[tokio::test]
    async fn test_instanced_matches_legacy_vertex_positions() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let (legacy_verts, _) = chart.generate_axis_geometry();
        let geom = chart.generate_axis_geometry_instanced();
        let (instanced_verts, _) = geom.into_legacy();

        for (i, (lv, iv)) in legacy_verts.iter().zip(instanced_verts.iter()).enumerate() {
            assert!(
                (lv.position[0] - iv.position[0]).abs() < 1e-6,
                "Vertex {i} X mismatch: legacy={} instanced={}",
                lv.position[0],
                iv.position[0],
            );
            assert!(
                (lv.position[1] - iv.position[1]).abs() < 1e-6,
                "Vertex {i} Y mismatch: legacy={} instanced={}",
                lv.position[1],
                iv.position[1],
            );
        }
    }

    #[tokio::test]
    async fn test_instanced_four_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default();
        let axis_config = AxisConfiguration::default();
        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )))
            .with_left_axis(Box::new(LinearAxis::new(
                AxisPosition::Left,
                axis_config.clone(),
            )))
            .with_top_axis(Box::new(LinearAxis::new(
                AxisPosition::Top,
                axis_config.clone(),
            )))
            .with_right_axis(Box::new(LinearAxis::new(AxisPosition::Right, axis_config)));

        let geom = chart.generate_axis_geometry_instanced();

        // 4 axes × 2 line verts = 8
        assert_eq!(geom.line_vertices.len(), 8, "4 axes × 2 line verts each");
        // 4 axes × 6 tick instances = 24
        assert_eq!(
            geom.tick_instances.len(),
            24,
            "4 axes × 6 tick instances each"
        );
        // 4 axes × 6 labels = 24
        assert_eq!(geom.labels.len(), 24, "4 axes × 6 labels each");
    }

    #[tokio::test]
    async fn test_has_tick_data_initially_false() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();
        assert!(
            !chart.has_tick_data(),
            "Before render(), tick data should not be available"
        );
    }

    #[tokio::test]
    async fn test_instanced_data_reduction() {
        use crate::axis::TickInstance;
        use std::mem::size_of;

        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();
        let geom = chart.generate_axis_geometry_instanced();

        let instanced_bytes = geom.tick_instances.len() * size_of::<TickInstance>() + 8; // base geometry (2 × f32)
        let vertex_pair_bytes = geom.tick_instances.len() * 2 * size_of::<Vertex>();

        assert!(
            instanced_bytes < vertex_pair_bytes,
            "Instanced ({instanced_bytes} B) should be smaller than vertex pairs ({vertex_pair_bytes} B)"
        );
    }

    #[tokio::test]
    async fn test_has_axis_line_data_initially_false() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();
        assert!(
            !chart.has_axis_line_data(),
            "Before prepare_draw_commands(), axis-line data should not be available"
        );
    }

    #[tokio::test]
    async fn test_prepare_draw_commands_creates_axis_line_data() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context.clone())
                .unwrap();
        let mut chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        chart.prepare_draw_commands(
            context.device(),
            context.queue(),
            wgpu::TextureFormat::Bgra8Unorm,
        );

        assert!(
            chart.has_axis_line_data(),
            "After prepare_draw_commands(), axis-line data should be available"
        );
        assert!(
            chart.has_tick_data(),
            "After prepare_draw_commands(), tick data should also be available"
        );
    }

    #[tokio::test]
    async fn test_prepare_draw_commands_no_axes_no_data() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context.clone())
                .unwrap();
        let config = ChartConfig {
            show_axes: false,
            ..ChartConfig::default()
        };
        let mut chart = ComposedChart::new(sel, config);

        chart.prepare_draw_commands(
            context.device(),
            context.queue(),
            wgpu::TextureFormat::Bgra8Unorm,
        );

        assert!(
            !chart.has_axis_line_data(),
            "No axes means no axis-line data"
        );
        assert!(!chart.has_tick_data(), "No axes means no tick data");
    }

    #[tokio::test]
    async fn test_prepare_draw_commands_pipeline_reused_across_calls() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::Circle>::new(vec![], context.clone())
                .unwrap();
        let mut chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();
        let format = wgpu::TextureFormat::Bgra8Unorm;

        // First call creates the pipeline
        chart.prepare_draw_commands(context.device(), context.queue(), format);
        assert!(chart.has_axis_line_data());

        // Pipeline fields should remain Some after a second call
        chart.prepare_draw_commands(context.device(), context.queue(), format);
        assert!(chart.has_axis_line_data());
        assert!(chart.has_tick_data());
    }
}

#[cfg(test)]
mod tests_multi_font {
    use super::*;
    use crate::text::TextStyle;

    // --- ChartConfig text style tests (no GPU required) ---

    #[test]
    fn test_chart_config_default_label_style() {
        let config = ChartConfig::default();
        assert_eq!(config.label_style.font_size, 14.0);
        assert_eq!(config.label_style.font_family, None);
    }

    #[test]
    fn test_chart_config_default_title_style() {
        let config = ChartConfig::default();
        assert_eq!(config.title_style.font_size, 18.0);
        assert_eq!(config.title_style.weight, 1.0); // bold
        assert_eq!(config.title_style.font_family, None);
    }

    #[test]
    fn test_chart_config_with_label_style() {
        let config = ChartConfig::default()
            .with_label_style(TextStyle::new(16.0).with_font_family("DejaVu Sans"));
        assert_eq!(config.label_style.font_size, 16.0);
        assert_eq!(
            config.label_style.font_family,
            Some("DejaVu Sans".to_string())
        );
    }

    #[test]
    fn test_chart_config_with_title_style() {
        let config = ChartConfig::default()
            .with_title_style(TextStyle::new(24.0).with_font_family("DejaVu Serif"));
        assert_eq!(config.title_style.font_size, 24.0);
        assert_eq!(
            config.title_style.font_family,
            Some("DejaVu Serif".to_string())
        );
    }

    #[test]
    fn test_chart_config_with_title() {
        let config = ChartConfig::default().with_title("My Chart");
        assert_eq!(config.title(), Some("My Chart"));
    }

    #[test]
    fn test_chart_config_title_accepts_string() {
        let config = ChartConfig::default().with_title(String::from("Dynamic Title"));
        assert_eq!(config.title(), Some("Dynamic Title"));
    }

    // --- GPU-dependent tests (require single-threaded test runner) ---

    #[tokio::test]
    async fn test_queue_chart_text_no_axes() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig {
            show_axes: false,
            ..ChartConfig::default()
        };
        let mut chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::empty(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_default_axes() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let mut chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // The font manager should have created at least one atlas (the default)
        assert!(
            font_manager.atlas_count() > 0,
            "Expected at least one font atlas"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_font_family() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_label_style(TextStyle::new(14.0).with_font_family("DejaVu Sans"));
        let mut chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // DejaVu Sans atlas should have been created
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected a DejaVu Sans atlas to be created"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_title() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_title("Test Title")
            .with_title_style(TextStyle::new(20.0).with_font_family("DejaVu Serif"));
        let mut chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Both default atlas (for labels) and DejaVu Serif (for title) should exist
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected a DejaVu Serif atlas for the title"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_resolved_with_collision_detection() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let mut chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();
        let mut positioner = crate::label::LabelPositioner::new();
        let constraints = crate::label::LabelConstraints::axis_labels();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text_resolved(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
            &mut positioner,
            &constraints,
        );
        assert!(
            result.is_ok(),
            "queue_chart_text_resolved failed: {result:?}"
        );
    }

    // --- TitleConfig and TitleAlignment tests (no GPU required) ---

    #[test]
    fn test_title_config_new() {
        let tc = TitleConfig::new("Hello");
        assert_eq!(tc.text, "Hello");
        assert_eq!(tc.alignment, TitleAlignment::Center);
        assert!(tc.y_offset.is_none());
        assert!(tc.subtitle.is_none());
        assert_eq!(tc.line_spacing, 1.2);
    }

    #[test]
    fn test_title_config_builders() {
        let tc = TitleConfig::new("Main")
            .with_alignment(TitleAlignment::Left)
            .with_y_offset(10.0)
            .with_subtitle("Sub")
            .with_subtitle_style(TextStyle::new(12.0))
            .with_line_spacing(1.5);

        assert_eq!(tc.alignment, TitleAlignment::Left);
        assert_eq!(tc.y_offset, Some(10.0));
        assert_eq!(tc.subtitle, Some("Sub".to_string()));
        assert_eq!(tc.subtitle_style.font_size, 12.0);
        assert_eq!(tc.line_spacing, 1.5);
    }

    #[test]
    fn test_title_alignment_default_is_center() {
        assert_eq!(TitleAlignment::default(), TitleAlignment::Center);
    }

    #[test]
    fn test_title_config_line_spacing_min() {
        let tc = TitleConfig::new("X").with_line_spacing(-5.0);
        assert_eq!(tc.line_spacing, 0.1);
    }

    #[test]
    fn test_chart_config_with_title_creates_title_config() {
        let config = ChartConfig::default().with_title("My Title");
        let tc = config.title_config.as_ref().unwrap();
        assert_eq!(tc.text, "My Title");
        assert_eq!(tc.alignment, TitleAlignment::Center);
    }

    #[test]
    fn test_chart_config_with_title_config_full() {
        let config = ChartConfig::default().with_title_config(
            TitleConfig::new("Revenue")
                .with_alignment(TitleAlignment::Right)
                .with_subtitle("Q4 2024"),
        );
        let tc = config.title_config.as_ref().unwrap();
        assert_eq!(tc.text, "Revenue");
        assert_eq!(tc.alignment, TitleAlignment::Right);
        assert_eq!(tc.subtitle, Some("Q4 2024".to_string()));
    }

    #[test]
    fn test_chart_config_title_accessor() {
        let config = ChartConfig::default();
        assert!(config.title().is_none());

        let config = config.with_title("Test");
        assert_eq!(config.title(), Some("Test"));
    }

    #[test]
    fn test_chart_config_no_title_by_default() {
        let config = ChartConfig::default();
        assert!(config.title_config.is_none());
    }

    // --- GPU tests for title rendering with TitleConfig ---

    #[tokio::test]
    async fn test_queue_title_text_with_subtitle() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_title_config(
                TitleConfig::new("Main Title")
                    .with_subtitle("Subtitle Here")
                    .with_subtitle_style(
                        TextStyle::new(12.0)
                            .with_font_family("DejaVu Sans")
                            .with_rgba(0.5, 0.5, 0.5, 1.0),
                    ),
            )
            .with_title_style(TextStyle::new(20.0).bold().with_font_family("DejaVu Serif"));

        let mut chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Title font atlas should exist
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected DejaVu Serif atlas for the title"
        );
        // Subtitle font atlas should exist
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected DejaVu Sans atlas for the subtitle"
        );
    }

    #[tokio::test]
    async fn test_queue_title_text_left_aligned() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_title_config(TitleConfig::new("Left Title").with_alignment(TitleAlignment::Left));

        let mut chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");
    }

    #[tokio::test]
    async fn test_queue_title_text_no_title() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        // No title set — should render without error
        let config = ChartConfig::default();
        let mut chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");
    }

    // --- Per-axis label style override tests (GUP-217) ---

    #[test]
    fn test_axis_config_default_has_no_label_style() {
        let config = AxisConfiguration::default();
        assert!(
            config.label_style.is_none(),
            "Default AxisConfiguration should have no label_style override"
        );
    }

    #[test]
    fn test_axis_config_with_label_style() {
        let style = TextStyle::new(12.0).with_font_family("Monospace");
        let config = AxisConfiguration::default().with_label_style(style.clone());
        assert_eq!(config.label_style, Some(style));
    }

    #[test]
    fn test_axis_config_label_style_preserves_font_family() {
        let config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(10.0).with_font_family("DejaVu Serif"));
        let ls = config.label_style.unwrap();
        assert_eq!(ls.font_family, Some("DejaVu Serif".to_string()));
        assert_eq!(ls.font_size, 10.0);
    }

    #[tokio::test]
    async fn test_queue_chart_text_per_axis_style() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();

        // Chart-level label style uses default font
        let config = ChartConfig::default();

        // Create axes with per-axis label styles
        let bottom_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(12.0).with_font_family("DejaVu Sans"));
        let left_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(16.0).with_font_family("DejaVu Serif"));

        let mut chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                bottom_config,
            )))
            .with_left_axis(Box::new(LinearAxis::new(AxisPosition::Left, left_config)));

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Both per-axis fonts should have atlases created
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected a DejaVu Sans atlas for the bottom axis"
        );
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected a DejaVu Serif atlas for the left axis"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_mixed_chart_and_axis_styles() {
        // Chart-level style applies to axes without per-axis overrides
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();

        // Chart-level label style
        let config = ChartConfig::default()
            .with_label_style(TextStyle::new(14.0).with_font_family("DejaVu Sans"));

        // Only bottom axis gets a per-axis override; left axis uses chart-level
        let bottom_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(12.0).with_font_family("DejaVu Serif"));

        let mut chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                bottom_config,
            )))
            .with_left_axis(Box::new(LinearAxis::new(
                AxisPosition::Left,
                AxisConfiguration::default(),
            )));

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Bottom axis per-axis font
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected a DejaVu Serif atlas for the bottom axis override"
        );
        // Left axis falls back to chart-level font
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected a DejaVu Sans atlas for the left axis (chart-level fallback)"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_resolved_per_axis_style() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();

        let bottom_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(12.0).with_font_family("DejaVu Sans"));
        let left_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(16.0).with_font_family("DejaVu Serif"));

        let mut chart = ComposedChart::new(sel, ChartConfig::default())
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                bottom_config,
            )))
            .with_left_axis(Box::new(LinearAxis::new(AxisPosition::Left, left_config)));

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();
        let mut positioner = crate::label::LabelPositioner::new();
        let constraints = crate::label::LabelConstraints::axis_labels();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text_resolved(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
            &mut positioner,
            &constraints,
        );
        assert!(
            result.is_ok(),
            "queue_chart_text_resolved with per-axis styles failed: {result:?}"
        );

        // Both per-axis fonts should have atlases
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected DejaVu Sans atlas for bottom axis"
        );
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected DejaVu Serif atlas for left axis"
        );
    }
}

#[cfg(test)]
mod tests_hover_reveal {
    use super::*;
    use crate::text::hover_reveal::TooltipConfig;

    #[derive(Debug, Clone)]
    struct D {
        x: f32,
    }

    // --- Non-GPU tests (ChartConfig, ComposedChart state) ---

    #[test]
    fn test_chart_config_hover_reveal_default_off() {
        let config = ChartConfig::default();
        assert!(!config.hover_reveal, "hover reveal should default to off");
    }

    #[test]
    fn test_chart_config_with_hover_reveal() {
        let config = ChartConfig::default().with_hover_reveal(true);
        assert!(config.hover_reveal);
    }

    #[test]
    fn test_chart_config_with_tooltip_config_enables_hover_reveal() {
        let tooltip = TooltipConfig {
            show_delay: 0.5,
            ..TooltipConfig::default()
        };
        let config = ChartConfig::default().with_tooltip_config(tooltip);
        assert!(
            config.hover_reveal,
            "with_tooltip_config should enable hover_reveal"
        );
        assert!(
            (config.tooltip_config.show_delay - 0.5).abs() < f32::EPSILON,
            "custom show_delay should be preserved"
        );
    }

    #[test]
    fn test_scatter_builder_hover_reveal() {
        use crate::chart_builder::builders::{ConfigurableBuilder, scatter};
        let builder = scatter::<D>().hover_reveal(true);
        assert!(builder.config.hover_reveal);
    }

    #[test]
    fn test_scatter_builder_tooltip_config() {
        use crate::chart_builder::builders::{ConfigurableBuilder, scatter};
        let tooltip = TooltipConfig {
            font_size: 20.0,
            ..TooltipConfig::default()
        };
        let builder = scatter::<D>().tooltip_config(tooltip);
        assert!(builder.config.hover_reveal);
        assert!((builder.config.tooltip_config.font_size - 20.0).abs() < f32::EPSILON);
    }

    // --- GPU-dependent tests ---

    #[tokio::test]
    async fn test_composed_chart_owns_registry_and_state() {
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default().with_hover_reveal(true);
        let chart = ComposedChart::new(sel, config);

        assert!(chart.hover_reveal_enabled());
        assert!(chart.clipped_text_registry().is_empty());
        assert!(!chart.hover_state().is_active());
    }

    #[tokio::test]
    async fn test_update_hover_noop_when_disabled() {
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default(); // hover_reveal defaults to false
        let mut chart = ComposedChart::new(sel, config);

        // Should be a no-op
        chart.update_hover(100.0, 200.0, 0.016);
        assert!(!chart.hover_state().is_active());
    }

    #[tokio::test]
    async fn test_queue_chart_text_clears_registry() {
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default().with_hover_reveal(true);
        let mut chart = ComposedChart::new(sel, config);

        // Manually register an entry
        chart
            .clipped_text_registry_mut()
            .register(crate::text::TextBounds::new(0.0, 0.0, 10.0, 10.0), "test");
        assert_eq!(chart.clipped_text_registry().len(), 1);

        // queue_chart_text should clear it
        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");
    }

    #[tokio::test]
    async fn test_queue_tooltip_text_returns_false_when_disabled() {
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default(); // hover_reveal off
        let chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let queued = chart
            .queue_tooltip_text(
                &frame,
                &mut text_renderer,
                &mut font_manager,
                &mut layout_engine,
            )
            .unwrap();
        assert!(
            !queued,
            "Should not queue tooltip when hover reveal is disabled"
        );
    }

    #[tokio::test]
    async fn test_queue_tooltip_text_returns_false_when_no_hover() {
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default().with_hover_reveal(true);
        let chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let queued = chart
            .queue_tooltip_text(
                &frame,
                &mut text_renderer,
                &mut font_manager,
                &mut layout_engine,
            )
            .unwrap();
        assert!(!queued, "Should not queue tooltip when nothing is hovered");
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_hover_reveal_and_axes() {
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default().with_hover_reveal(true);
        let mut chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(
            result.is_ok(),
            "queue_chart_text with hover reveal failed: {result:?}"
        );
    }

    #[test]
    fn test_label_viewport_bounds_bottom_axis() {
        let config = ChartConfig::default();
        let chart_area = ChartArea {
            x: 60.0,
            y: 40.0,
            width: 700.0,
            height: 500.0,
            margins: Margins::default(),
        };
        let bounds = ComposedChart::<D, crate::Circle>::label_viewport_bounds(
            AxisPosition::Bottom,
            &chart_area,
            &config,
        );
        // Bottom axis viewport should be below the chart area
        assert!(bounds.viewport_rect.top >= chart_area.y + chart_area.height);
        assert!((bounds.viewport_rect.bottom - config.height).abs() < f32::EPSILON);
    }

    #[test]
    fn test_label_viewport_bounds_left_axis() {
        let config = ChartConfig::default();
        let chart_area = ChartArea {
            x: 60.0,
            y: 40.0,
            width: 700.0,
            height: 500.0,
            margins: Margins::default(),
        };
        let bounds = ComposedChart::<D, crate::Circle>::label_viewport_bounds(
            AxisPosition::Left,
            &chart_area,
            &config,
        );
        // Left axis viewport should be to the left of chart area
        assert!((bounds.viewport_rect.left - 0.0).abs() < f32::EPSILON);
        assert!((bounds.viewport_rect.right - chart_area.x).abs() < f32::EPSILON);
    }

    // --- Builder trait conformance tests ---

    #[test]
    fn test_line_builder_hover_reveal() {
        use crate::chart_builder::builders::{ConfigurableBuilder, line};
        let builder = line::<D>().hover_reveal(true);
        assert!(builder.config.hover_reveal);
    }

    #[test]
    fn test_bar_builder_hover_reveal() {
        use crate::chart_builder::builders::{ConfigurableBuilder, bar};
        let builder = bar::<D>().hover_reveal(true);
        assert!(builder.config.hover_reveal);
    }

    #[test]
    fn test_area_builder_hover_reveal() {
        use crate::chart_builder::builders::{ConfigurableBuilder, area};
        let builder = area::<D>().hover_reveal(true);
        assert!(builder.config.hover_reveal);
    }

    #[test]
    fn test_heatmap_builder_hover_reveal() {
        use crate::chart_builder::builders::{ConfigurableBuilder, heatmap};
        let builder = heatmap::<D>().hover_reveal(true);
        assert!(builder.config.hover_reveal);
    }

    #[test]
    fn test_boxplot_builder_hover_reveal() {
        use crate::chart_builder::builders::{BoxPlotBuilder, ConfigurableBuilder};
        let builder = BoxPlotBuilder::<D>::new().hover_reveal(true);
        assert!(builder.config.hover_reveal);
    }
}

#[cfg(test)]
mod tests_colorbar {
    use super::*;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct D {
        x: f32,
        y: f32,
    }

    #[test]
    fn test_chart_config_show_colorbar_default_is_false() {
        let config = ChartConfig::default();
        assert!(!config.show_colorbar);
    }

    #[test]
    fn test_chart_config_with_colorbar() {
        let config = ChartConfig::default().with_colorbar(true);
        assert!(config.show_colorbar);
    }

    #[tokio::test]
    async fn test_has_colorbar_requires_both_flag_and_scale() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        // No scale, no flag
        let config = ChartConfig::default();
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 0.0, y: 0.0 }], context.clone())
            .unwrap();
        let chart = ComposedChart::new(sel, config);
        assert!(!chart.has_colorbar());

        // Flag only, no scale
        let config = ChartConfig::default().with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 0.0, y: 0.0 }], context.clone())
            .unwrap();
        let chart = ComposedChart::new(sel, config);
        assert!(!chart.has_colorbar());

        // Scale only, no flag
        let config = ChartConfig::default().with_color_scale(ColorScale::viridis(0.0, 100.0));
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 0.0, y: 0.0 }], context.clone())
            .unwrap();
        let chart = ComposedChart::new(sel, config);
        assert!(!chart.has_colorbar());

        // Both set
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(0.0, 100.0))
            .with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 0.0, y: 0.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config);
        assert!(chart.has_colorbar());
    }

    #[tokio::test]
    async fn test_colorbar_geometry_generated_when_enabled() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(0.0, 100.0))
            .with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 1.0, y: 2.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config).with_default_axes();
        let geom = chart.generate_colorbar_geometry();
        assert!(geom.is_some());
        let geom = geom.unwrap();
        assert!(!geom.gradient_vertices.is_empty());
        assert!(!geom.tick_instances.is_empty());
        assert!(!geom.labels.is_empty());
        assert_eq!(geom.line_vertices.len(), 8); // outline
    }

    #[tokio::test]
    async fn test_colorbar_geometry_none_when_disabled() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(0.0, 100.0))
            .with_colorbar(false);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 1.0, y: 2.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config).with_default_axes();
        let geom = chart.generate_colorbar_geometry();
        assert!(geom.is_none());
    }

    #[tokio::test]
    async fn test_colorbar_geometry_gradient_vertex_count() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(0.0, 100.0))
            .with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 1.0, y: 2.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config).with_default_axes();
        let geom = chart.generate_colorbar_geometry().unwrap();
        // Default 64 segments × 6 verts = 384
        assert_eq!(geom.gradient_vertices.len(), 64 * 6);
    }

    #[tokio::test]
    async fn test_colorbar_labels_contain_domain_values() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(0.0, 100.0))
            .with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 1.0, y: 2.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config).with_default_axes();
        let geom = chart.generate_colorbar_geometry().unwrap();

        // Labels should contain values within [0, 100]
        for label in &geom.labels {
            assert!(
                label.value >= 0.0 && label.value <= 100.0,
                "Label value {} should be in domain [0, 100]",
                label.value
            );
        }
    }

    #[tokio::test]
    async fn test_colorbar_composable_with_scatter_chart() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::plasma(0.0, 50.0))
            .with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(
            vec![D { x: 1.0, y: 2.0 }, D { x: 3.0, y: 4.0 }],
            context,
        )
        .unwrap();
        let chart = ComposedChart::new(sel, config).with_default_axes();

        assert!(chart.has_colorbar());
        let geom = chart.generate_colorbar_geometry().unwrap();
        assert!(!geom.gradient_vertices.is_empty());
    }

    #[tokio::test]
    async fn test_colorbar_inherits_fill_domain() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(10.0, 200.0))
            .with_colorbar(true);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 1.0, y: 2.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config);

        let geom = chart.generate_colorbar_geometry().unwrap();

        // Labels should be in the [10, 200] domain
        let min_val = geom.labels.iter().map(|l| l.value).fold(f64::MAX, f64::min);
        let max_val = geom.labels.iter().map(|l| l.value).fold(f64::MIN, f64::max);

        assert!(
            min_val >= 10.0 - 1.0,
            "min label {min_val} should be near domain min 10"
        );
        assert!(
            max_val <= 200.0 + 1.0,
            "max label {max_val} should be near domain max 200"
        );
    }

    #[tokio::test]
    async fn test_colorbar_suppressed_by_colorbar_false() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        // ColorScale set but show_colorbar = false
        let config = ChartConfig::default()
            .with_color_scale(ColorScale::viridis(0.0, 100.0))
            .with_colorbar(false);
        let sel = Selection::<D, crate::Circle>::new(vec![D { x: 1.0, y: 2.0 }], context).unwrap();
        let chart = ComposedChart::new(sel, config).with_default_axes();

        assert!(!chart.has_colorbar());
        assert!(chart.generate_colorbar_geometry().is_none());
    }
}
