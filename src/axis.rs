// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core axis system for professional data visualization.
//!
//! This module provides the foundation for axis rendering in Gup visualizations,
//! including automatic positioning, tick generation, and GPU-accelerated rendering
//! using the existing Mark system.
//!
//! # Core Components
//!
//! * **`Axis`** trait - Core interface for all axis types
//! * **`LinearAxis`** - Linear scale axis implementation
//! * **`AxisRenderer`** - GPU-based rendering using Line marks
//! * **`AxisConfiguration`** - Appearance and behavior settings
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::axis::{LinearAxis, AxisPosition, AxisConfiguration};
//! use gup::RenderContext;
//! use std::sync::Arc;
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! let context = Arc::new(RenderContext::new().await?);
//! let config = AxisConfiguration::default();
//! let axis = LinearAxis::new(AxisPosition::Bottom, config);
//!
//! // Axis will be rendered automatically by chart builders
//! # Ok(())
//! # }
//! ```

use crate::axis_performance::{AxisGeometryCache, AxisLODManager, LODConfiguration};
use crate::error::GupResult;
use crate::label::{LabelFormatter, NumericFormatter};
use crate::render::{RenderContext, Vertex};
use crate::shader_function::Vec2;
use crate::text::{TextAnchor, TextStyle};
use crate::tick_generator::{LinearTickGenerator, Scale, TickGenerator};
use crate::{MaybeSend, MaybeSync};

/// Position of an axis relative to the chart area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisPosition {
    /// Top of the chart (horizontal axis)
    Top,
    /// Bottom of the chart (horizontal axis)
    Bottom,
    /// Left side of the chart (vertical axis)
    Left,
    /// Right side of the chart (vertical axis)
    Right,
}

impl AxisPosition {
    /// Check if this is a horizontal axis (top or bottom).
    pub fn is_horizontal(&self) -> bool {
        matches!(self, AxisPosition::Top | AxisPosition::Bottom)
    }

    /// Check if this is a vertical axis (left or right).
    pub fn is_vertical(&self) -> bool {
        !self.is_horizontal()
    }
}

/// Bounds and coordinate information for axis rendering.
#[derive(Debug, Clone)]
pub struct AxisBounds {
    /// Start point of the axis line
    pub start: Vec2,
    /// End point of the axis line
    pub end: Vec2,
    /// Available margin space for labels and ticks
    pub available_margin: f32,
}

impl AxisBounds {
    /// Create new axis bounds.
    pub fn new(start: Vec2, end: Vec2, available_margin: f32) -> Self {
        Self {
            start,
            end,
            available_margin,
        }
    }

    /// Calculate the length of the axis.
    pub fn length(&self) -> f32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get the direction vector of the axis (normalized).
    pub fn direction(&self) -> Vec2 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let length = self.length();
        if length > 0.0 {
            Vec2 {
                x: dx / length,
                y: dy / length,
            }
        } else {
            Vec2 { x: 1.0, y: 0.0 }
        }
    }

    /// Get the normal vector perpendicular to the axis.
    pub fn normal(&self) -> Vec2 {
        let dir = self.direction();
        Vec2 {
            x: -dir.y,
            y: dir.x,
        }
    }
}

/// Configuration for axis appearance and behavior.
#[derive(Debug, Clone)]
pub struct AxisConfiguration {
    /// Whether to show the main axis line
    pub show_line: bool,
    /// Whether to show major tick marks
    pub show_major_ticks: bool,
    /// Whether to show minor tick marks
    pub show_minor_ticks: bool,
    /// Length of major ticks in pixels
    pub major_tick_length: f32,
    /// Length of minor ticks in pixels
    pub minor_tick_length: f32,
    /// Color of axis lines and ticks (RGBA)
    pub line_color: [f32; 4],
    /// Width of axis lines in pixels
    pub line_width: f32,
    /// Target number of major ticks (None for automatic)
    pub target_tick_count: Option<usize>,
    /// Number of minor tick subdivisions between major ticks
    pub minor_tick_subdivisions: usize,
    /// Optional per-axis text style for labels.
    ///
    /// When set, this style overrides the chart-level `ChartConfig::label_style`
    /// for labels on this axis. When `None`, the chart-level style is used.
    pub label_style: Option<TextStyle>,
}

impl Default for AxisConfiguration {
    fn default() -> Self {
        Self {
            show_line: true,
            show_major_ticks: true,
            show_minor_ticks: false,
            major_tick_length: 6.0,
            minor_tick_length: 3.0,
            line_color: [0.2, 0.2, 0.2, 1.0], // Dark gray
            line_width: 1.0,
            target_tick_count: None,    // Automatic tick count
            minor_tick_subdivisions: 5, // 5 subdivisions between major ticks
            label_style: None,          // Uses chart-level style
        }
    }
}

impl AxisConfiguration {
    /// Create a new axis configuration with custom colors.
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.line_color = color;
        self
    }

    /// Create a new axis configuration with custom line width.
    pub fn with_line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Create a new axis configuration with custom tick lengths.
    pub fn with_tick_lengths(mut self, major: f32, minor: f32) -> Self {
        self.major_tick_length = major;
        self.minor_tick_length = minor;
        self
    }

    /// Hide minor ticks.
    pub fn without_minor_ticks(mut self) -> Self {
        self.show_minor_ticks = false;
        self
    }

    /// Hide all ticks.
    pub fn without_ticks(mut self) -> Self {
        self.show_major_ticks = false;
        self.show_minor_ticks = false;
        self
    }

    /// Hide the axis line (keeping only ticks).
    pub fn without_line(mut self) -> Self {
        self.show_line = false;
        self
    }

    /// Set target number of major ticks.
    pub fn with_tick_count(mut self, count: usize) -> Self {
        self.target_tick_count = Some(count);
        self
    }

    /// Set number of minor tick subdivisions.
    pub fn with_minor_subdivisions(mut self, subdivisions: usize) -> Self {
        self.minor_tick_subdivisions = subdivisions;
        self
    }

    /// Set a per-axis label text style.
    ///
    /// When set, this style overrides the chart-level `ChartConfig::label_style`
    /// for labels rendered on this axis. This enables different fonts, sizes,
    /// or colours for each axis.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gup::axis::AxisConfiguration;
    /// use gup::text::TextStyle;
    ///
    /// let config = AxisConfiguration::default()
    ///     .with_label_style(TextStyle::new(12.0).with_font_family("Monospace"));
    /// assert!(config.label_style.is_some());
    /// ```
    pub fn with_label_style(mut self, style: TextStyle) -> Self {
        self.label_style = Some(style);
        self
    }
}

/// Core trait for axis implementations.
///
/// The Axis trait defines the interface that all axis types must implement.
/// It provides methods for rendering, layout calculation, and tick positioning
/// that integrate with the GPU-accelerated rendering system.
pub trait Axis: MaybeSend + MaybeSync + std::fmt::Debug + 'static {
    /// Get the position of this axis relative to the chart area.
    fn position(&self) -> AxisPosition;

    /// Render the axis (line, ticks, and basic structure) using GPU acceleration.
    ///
    /// This method uses the existing Line mark system to efficiently render
    /// axis components with batched GPU operations.
    fn render(&self, context: &mut RenderContext, bounds: AxisBounds) -> GupResult<()>;

    /// Calculate the margin space needed for this axis.
    ///
    /// This includes space for tick marks and labels. The scale parameter
    /// is used for automatic tick generation to determine space requirements.
    fn calculate_margin(&self, scale: Option<&dyn Scale>) -> f32 {
        let config = self.configuration();

        // Calculate required margin space based on configuration
        let tick_margin = if config.show_major_ticks {
            config.major_tick_length + 2.0 // Extra space for padding
        } else {
            0.0
        };

        let base_margin = match self.position() {
            AxisPosition::Left | AxisPosition::Right => 60.0, // Space for labels
            AxisPosition::Top | AxisPosition::Bottom => 40.0,
        };

        // Future: adjust margin based on scale range and label formatting
        let _ = scale; // Acknowledge parameter for future use

        base_margin + tick_margin
    }

    /// Get tick positions for integration with grid system.
    ///
    /// Returns normalized positions (0.0 to 1.0) along the axis where
    /// ticks should be placed. Uses automatic tick generation when scale is provided.
    fn get_tick_positions(&self, scale: Option<&dyn Scale>, pixel_range: f32) -> Vec<f32> {
        if let Some(scale) = scale {
            // Use automatic tick generation
            let generator = LinearTickGenerator::default();
            let config = self.configuration();

            let major_ticks =
                generator.generate_major_ticks(scale, pixel_range, config.target_tick_count);

            // Convert domain values to normalized positions
            major_ticks
                .iter()
                .map(|&tick| scale.normalize(tick) as f32)
                .collect()
        } else {
            // Fallback to basic tick positions
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        }
    }

    /// Get minor tick positions for integration with grid system.
    ///
    /// Returns normalized positions (0.0 to 1.0) along the axis where
    /// minor ticks should be placed.
    fn get_minor_tick_positions(&self, scale: Option<&dyn Scale>, pixel_range: f32) -> Vec<f32> {
        if let Some(scale) = scale {
            let generator = LinearTickGenerator::default();
            let config = self.configuration();

            let major_ticks =
                generator.generate_major_ticks(scale, pixel_range, config.target_tick_count);

            let minor_ticks =
                generator.generate_minor_ticks(scale, &major_ticks, config.minor_tick_subdivisions);

            // Convert domain values to normalized positions
            minor_ticks
                .iter()
                .map(|&tick| scale.normalize(tick) as f32)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the axis configuration for appearance settings.
    fn configuration(&self) -> &AxisConfiguration;

    /// Update the axis configuration.
    fn set_configuration(&mut self, config: AxisConfiguration);
}

/// Linear axis implementation for continuous numeric scales.
///
/// LinearAxis provides a basic implementation of the Axis trait for
/// linear numeric data. It renders axis lines and tick marks using
/// the GPU-accelerated Line mark system.
#[derive(Debug, Clone)]
pub struct LinearAxis {
    position: AxisPosition,
    config: AxisConfiguration,
}

impl LinearAxis {
    /// Create a new linear axis.
    pub fn new(position: AxisPosition, config: AxisConfiguration) -> Self {
        Self { position, config }
    }

    /// Create a new linear axis with default configuration.
    pub fn with_position(position: AxisPosition) -> Self {
        Self::new(position, AxisConfiguration::default())
    }

    /// Calculate tick positions based on axis bounds and optional scale.
    ///
    /// Uses automatic tick generation algorithms for professional-quality
    /// tick spacing when a scale is provided.
    fn calculate_tick_positions(
        &self,
        bounds: &AxisBounds,
        scale: Option<&dyn Scale>,
    ) -> Vec<Vec2> {
        let tick_positions = self.get_tick_positions(scale, bounds.length());
        let direction = bounds.direction();

        tick_positions
            .iter()
            .map(|&t| Vec2 {
                x: bounds.start.x + direction.x * bounds.length() * t,
                y: bounds.start.y + direction.y * bounds.length() * t,
            })
            .collect()
    }

    /// Calculate minor tick positions based on axis bounds and optional scale.
    fn calculate_minor_tick_positions(
        &self,
        bounds: &AxisBounds,
        scale: Option<&dyn Scale>,
    ) -> Vec<Vec2> {
        let tick_positions = self.get_minor_tick_positions(scale, bounds.length());
        let direction = bounds.direction();

        tick_positions
            .iter()
            .map(|&t| Vec2 {
                x: bounds.start.x + direction.x * bounds.length() * t,
                y: bounds.start.y + direction.y * bounds.length() * t,
            })
            .collect()
    }
}

impl Axis for LinearAxis {
    fn position(&self) -> AxisPosition {
        self.position
    }

    fn render(&self, context: &mut RenderContext, bounds: AxisBounds) -> GupResult<()> {
        self.render_with_scale(context, bounds, None)
    }

    fn calculate_margin(&self, scale: Option<&dyn Scale>) -> f32 {
        let config = self.configuration();

        // Calculate required margin space based on configuration
        let tick_margin = if config.show_major_ticks {
            config.major_tick_length + 2.0 // Extra space for padding
        } else {
            0.0
        };

        let base_margin = match self.position {
            AxisPosition::Left | AxisPosition::Right => 60.0, // Space for labels
            AxisPosition::Top | AxisPosition::Bottom => 40.0,
        };

        // Future: adjust margin based on scale range and label formatting
        let _ = scale; // Acknowledge parameter for future use

        base_margin + tick_margin
    }

    fn get_tick_positions(&self, scale: Option<&dyn Scale>, pixel_range: f32) -> Vec<f32> {
        if let Some(scale) = scale {
            // Use automatic tick generation
            let generator = LinearTickGenerator::default();
            let config = self.configuration();

            let major_ticks =
                generator.generate_major_ticks(scale, pixel_range, config.target_tick_count);

            // Convert domain values to normalized positions
            major_ticks
                .iter()
                .map(|&tick| scale.normalize(tick) as f32)
                .collect()
        } else {
            // Fallback to basic tick positions
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        }
    }

    fn get_minor_tick_positions(&self, scale: Option<&dyn Scale>, pixel_range: f32) -> Vec<f32> {
        if let Some(scale) = scale {
            let generator = LinearTickGenerator::default();
            let config = self.configuration();

            let major_ticks =
                generator.generate_major_ticks(scale, pixel_range, config.target_tick_count);

            let minor_ticks =
                generator.generate_minor_ticks(scale, &major_ticks, config.minor_tick_subdivisions);

            // Convert domain values to normalized positions
            minor_ticks
                .iter()
                .map(|&tick| scale.normalize(tick) as f32)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn configuration(&self) -> &AxisConfiguration {
        &self.config
    }

    fn set_configuration(&mut self, config: AxisConfiguration) {
        self.config = config;
    }
}

impl LinearAxis {
    /// Render the axis with an optional scale for automatic tick generation.
    pub fn render_with_scale(
        &self,
        context: &mut RenderContext,
        bounds: AxisBounds,
        scale: Option<&dyn Scale>,
    ) -> GupResult<()> {
        // Render main axis line if configured
        if self.config.show_line {
            // Use AxisRenderer to create Line marks for the axis line
            let _renderer = AxisRenderer::new();
            _renderer.render_axis_line(context, &bounds, &self.config)?;
        }

        // Render tick marks if configured
        if self.config.show_major_ticks {
            let _renderer = AxisRenderer::new();
            let tick_positions = self.calculate_tick_positions(&bounds, scale);

            _renderer.render_ticks(
                context,
                &bounds,
                &tick_positions,
                self.config.major_tick_length,
                &self.config,
            )?;
        }

        // Render minor ticks if configured
        if self.config.show_minor_ticks {
            let _renderer = AxisRenderer::new();
            let minor_tick_positions = self.calculate_minor_tick_positions(&bounds, scale);

            _renderer.render_ticks(
                context,
                &bounds,
                &minor_tick_positions,
                self.config.minor_tick_length,
                &self.config,
            )?;
        }

        Ok(())
    }
}

/// Data for a single axis tick label, ready for rendering with [`TextRenderer`](crate::text::TextRenderer).
///
/// Screen positions follow the convention expected by `TextRenderConfig`:
/// origin at top-left, X increases right, Y increases down.
#[derive(Debug, Clone)]
pub struct AxisLabel {
    /// Formatted label text (e.g. "0.00", "$1,234", "50%").
    pub text: String,
    /// Position in screen/pixel coordinates (for `TextRenderConfig::position`).
    ///
    /// Includes a small offset away from the axis line so labels don't
    /// overlap the tick marks.
    pub screen_position: Vec2,
    /// Position in clip space (NDC: -1.0 to 1.0) on the axis line,
    /// before the label offset is applied. Useful for alignment with
    /// other chart elements that work in NDC.
    pub ndc_position: Vec2,
    /// Recommended text anchor based on the axis position.
    ///
    /// - Bottom axis: [`TextAnchor::TopCenter`] (label hangs below tick)
    /// - Top axis: [`TextAnchor::BottomCenter`] (label sits above tick)
    /// - Left axis: [`TextAnchor::CenterRight`] (label to the left of tick)
    /// - Right axis: [`TextAnchor::CenterLeft`] (label to the right of tick)
    pub anchor: TextAnchor,
    /// The underlying data value that was formatted to produce [`text`](Self::text).
    pub value: f64,
}

/// Per-instance data for GPU-instanced tick mark rendering.
///
/// Instead of generating two vertices per tick (a `LineList` pair),
/// instanced rendering uses a single base line segment that is replicated
/// by the GPU for each tick. The `TickInstance` provides the per-tick
/// parameters: where to place the tick on the axis and how long/directed
/// it should be.
///
/// # Layout
///
/// The struct is `#[repr(C)]` with [`bytemuck::Pod`] so it can be uploaded
/// directly to a GPU instance buffer.
///
/// # Usage
///
/// ```rust
/// use gup::axis::{AxisRenderer, AxisBounds, AxisConfiguration, AxisPosition, TickInstance};
/// use gup::shader_function::Vec2;
///
/// let renderer = AxisRenderer::new();
/// let bounds = AxisBounds::new(
///     Vec2 { x: -0.8, y: -0.8 },
///     Vec2 { x: 0.8, y: -0.8 },
///     50.0,
/// );
/// let config = AxisConfiguration::default();
///
/// let instances = renderer.generate_tick_instances(
///     &bounds,
///     &config,
///     AxisPosition::Bottom,
///     None,
///     (800.0, 600.0),
/// );
///
/// // Each tick is one instance instead of two vertices
/// assert!(instances.len() <= 12); // ≤ 6 major + 6 minor (if enabled)
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TickInstance {
    /// NDC position on the axis line where the tick starts.
    pub position: [f32; 2],
    /// Direction and length of the tick in NDC units.
    ///
    /// The base geometry is a line from `position` to
    /// `position + tick_vector`.
    pub tick_vector: [f32; 2],
    /// RGBA colour of this tick mark.
    pub color: [f32; 4],
}

impl TickInstance {
    /// Byte size of a single instance (for GPU buffer stride).
    pub const SIZE: u64 = std::mem::size_of::<Self>() as u64;

    /// Create a new tick instance.
    pub fn new(position: [f32; 2], tick_vector: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position,
            tick_vector,
            color,
        }
    }

    /// `wgpu::VertexBufferLayout` describing the per-instance attributes.
    ///
    /// Use this alongside the base vertex buffer layout when creating an
    /// instanced render pipeline for tick marks.
    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: Self::SIZE,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // @location(1) position: vec2<f32>
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // @location(2) tick_vector: vec2<f32>
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // @location(3) color: vec4<f32>
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as u64,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Axis renderer that generates vertex data for GPU rendering.
///
/// `AxisRenderer` produces `Vec<Vertex>` data (using `LineList` topology)
/// for axis lines and tick marks. Callers are responsible for creating
/// their own render pipeline and drawing the vertices within their
/// render pass. This composable approach lets axes be rendered alongside
/// other chart elements in a single render pass.
///
/// # Coordinate System
///
/// All generated vertices are in clip space (NDC: -1.0 to 1.0).
/// Tick lengths in the [`AxisConfiguration`] are specified in pixels and
/// are converted to NDC using the provided `viewport_size` parameter.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::axis::{AxisRenderer, AxisBounds, AxisConfiguration, AxisPosition};
/// use gup::shader_function::Vec2;
///
/// let renderer = AxisRenderer::new();
/// let bounds = AxisBounds::new(
///     Vec2 { x: -0.8, y: -0.8 },
///     Vec2 { x: 0.8, y: -0.8 },
///     50.0,
/// );
/// let config = AxisConfiguration::default();
///
/// // Generate vertices for a bottom axis (viewport 800x600)
/// let vertices = renderer.generate_axis_vertices(
///     &bounds,
///     &config,
///     AxisPosition::Bottom,
///     None,
///     (800.0, 600.0),
/// );
///
/// // Draw `vertices` with a LineList pipeline in your render pass
/// ```
pub struct AxisRenderer {
    /// Geometry cache for avoiding per-frame vertex regeneration.
    geometry_cache: AxisGeometryCache,
    /// LOD manager for adaptive quality control.
    lod_manager: AxisLODManager,
}

impl AxisRenderer {
    /// Create a new axis renderer.
    pub fn new() -> Self {
        Self {
            geometry_cache: AxisGeometryCache::new(),
            lod_manager: AxisLODManager::default(),
        }
    }

    /// Create a new axis renderer with a specific LOD configuration.
    pub fn with_lod_config(lod_config: LODConfiguration) -> Self {
        Self {
            geometry_cache: AxisGeometryCache::new(),
            lod_manager: AxisLODManager::new(lod_config),
        }
    }

    /// Access the LOD manager for configuration or inspection.
    pub fn lod_manager(&self) -> &AxisLODManager {
        &self.lod_manager
    }

    /// Mutable access to the LOD manager.
    pub fn lod_manager_mut(&mut self) -> &mut AxisLODManager {
        &mut self.lod_manager
    }

    /// Access the geometry cache for diagnostics (hit rate, etc.).
    pub fn geometry_cache(&self) -> &AxisGeometryCache {
        &self.geometry_cache
    }

    /// Invalidate the geometry cache, forcing regeneration on the next call.
    pub fn invalidate_cache(&mut self) {
        self.geometry_cache.invalidate();
    }

    /// Generate axis vertices with automatic LOD selection and caching.
    ///
    /// This is the performance-optimized entry point that:
    /// 1. Selects an appropriate LOD based on axis pixel size and recent
    ///    render times.
    /// 2. Returns cached vertices when the axis configuration has not changed.
    /// 3. Falls back to full vertex generation only on cache miss.
    pub fn generate_axis_vertices_cached(
        &mut self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
        last_render_time: Option<std::time::Duration>,
    ) -> &[Vertex] {
        // 1. Calculate LOD
        let axis_pixel_length = Self::axis_pixel_length(bounds, viewport_size);
        let lod = self
            .lod_manager
            .calculate_lod(axis_pixel_length, last_render_time);

        // 2. Apply LOD to config
        let adjusted_config = lod.apply_to_config(config);

        // 3. Check cache
        if self
            .geometry_cache
            .get(bounds, &adjusted_config, position, viewport_size, lod)
            .is_some()
        {
            // Cache hit — return cached data
            return self
                .geometry_cache
                .get(bounds, &adjusted_config, position, viewport_size, lod)
                .unwrap();
        }

        // 4. Cache miss — generate and store
        let vertices =
            self.generate_axis_vertices(bounds, &adjusted_config, position, scale, viewport_size);
        self.geometry_cache.store(
            bounds,
            &adjusted_config,
            position,
            viewport_size,
            lod,
            vertices,
        );

        self.geometry_cache
            .get(bounds, &adjusted_config, position, viewport_size, lod)
            .unwrap()
    }

    /// Generate tick instances with automatic LOD selection and caching.
    ///
    /// This is the performance-optimized instanced counterpart to
    /// [`generate_axis_vertices_cached`](Self::generate_axis_vertices_cached).
    /// It follows the same LOD → cache → generate pipeline, but produces
    /// [`TickInstance`] data instead of `Vertex` pairs.
    pub fn generate_tick_instances_cached(
        &mut self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
        last_render_time: Option<std::time::Duration>,
    ) -> &[TickInstance] {
        // 1. Calculate LOD
        let axis_pixel_length = Self::axis_pixel_length(bounds, viewport_size);
        let lod = self
            .lod_manager
            .calculate_lod(axis_pixel_length, last_render_time);

        // 2. Apply LOD to config
        let adjusted_config = lod.apply_to_config(config);

        // 3. Check cache
        if self
            .geometry_cache
            .get_instances(bounds, &adjusted_config, position, viewport_size, lod)
            .is_some()
        {
            return self
                .geometry_cache
                .get_instances(bounds, &adjusted_config, position, viewport_size, lod)
                .unwrap();
        }

        // 4. Cache miss — generate and store
        let instances =
            self.generate_tick_instances(bounds, &adjusted_config, position, scale, viewport_size);
        self.geometry_cache.store_instances(
            bounds,
            &adjusted_config,
            position,
            viewport_size,
            lod,
            instances,
        );

        self.geometry_cache
            .get_instances(bounds, &adjusted_config, position, viewport_size, lod)
            .unwrap()
    }

    /// Compute the approximate pixel length of an axis from NDC bounds and viewport.
    fn axis_pixel_length(bounds: &AxisBounds, viewport_size: (f32, f32)) -> f32 {
        let ndc_len = bounds.length();
        // NDC range is 2.0 across each dimension.
        // For a horizontal axis, pixel length ≈ ndc_len / 2.0 * viewport_width.
        // Use the max component as a conservative estimate.
        let (vw, vh) = viewport_size;
        let dx = (bounds.end.x - bounds.start.x).abs() / 2.0 * vw;
        let dy = (bounds.end.y - bounds.start.y).abs() / 2.0 * vh;
        if ndc_len > 0.0 {
            (dx * dx + dy * dy).sqrt()
        } else {
            0.0
        }
    }

    /// Generate all vertices for an axis (line + major ticks + minor ticks).
    ///
    /// Returns a `Vec<Vertex>` suitable for drawing with `LineList` primitive
    /// topology. Each pair of consecutive vertices forms one line segment.
    ///
    /// # Parameters
    ///
    /// * `bounds` - The start and end points of the axis line in clip space.
    /// * `config` - Appearance settings (colors, tick lengths, visibility flags).
    /// * `position` - Where the axis sits relative to the chart area. This
    ///   determines the direction ticks extend (e.g. bottom-axis ticks point
    ///   downward, left-axis ticks point leftward).
    /// * `scale` - Optional scale for automatic tick generation. When `None`,
    ///   fallback tick positions at 0%, 20%, 40%, 60%, 80%, 100% are used.
    /// * `viewport_size` - `(width, height)` in pixels, used to convert
    ///   pixel-based tick lengths to NDC units.
    pub fn generate_axis_vertices(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();

        // Axis line
        if config.show_line {
            self.append_line_vertices(&mut vertices, bounds, config);
        }

        // Major ticks
        if config.show_major_ticks {
            let tick_positions =
                Self::compute_tick_positions(bounds, scale, config.target_tick_count, false, 0);

            self.append_tick_vertices(
                &mut vertices,
                bounds,
                &tick_positions,
                config.major_tick_length,
                config,
                position,
                viewport_size,
            );
        }

        // Minor ticks
        if config.show_minor_ticks {
            let tick_positions = Self::compute_tick_positions(
                bounds,
                scale,
                config.target_tick_count,
                true,
                config.minor_tick_subdivisions,
            );

            self.append_tick_vertices(
                &mut vertices,
                bounds,
                &tick_positions,
                config.minor_tick_length,
                config,
                position,
                viewport_size,
            );
        }

        vertices
    }

    /// Generate only the axis line vertices (no ticks).
    pub fn generate_line_vertices(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        if config.show_line {
            self.append_line_vertices(&mut vertices, bounds, config);
        }
        vertices
    }

    /// Generate only tick vertices (major and/or minor, no axis line).
    pub fn generate_tick_vertices(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();

        if config.show_major_ticks {
            let tick_positions =
                Self::compute_tick_positions(bounds, scale, config.target_tick_count, false, 0);

            self.append_tick_vertices(
                &mut vertices,
                bounds,
                &tick_positions,
                config.major_tick_length,
                config,
                position,
                viewport_size,
            );
        }

        if config.show_minor_ticks {
            let tick_positions = Self::compute_tick_positions(
                bounds,
                scale,
                config.target_tick_count,
                true,
                config.minor_tick_subdivisions,
            );

            self.append_tick_vertices(
                &mut vertices,
                bounds,
                &tick_positions,
                config.minor_tick_length,
                config,
                position,
                viewport_size,
            );
        }

        vertices
    }

    /// Generate per-instance data for instanced tick rendering.
    ///
    /// This is the instanced counterpart to
    /// [`generate_tick_vertices`](Self::generate_tick_vertices). Instead of
    /// producing two `Vertex` entries per tick, it produces one
    /// [`TickInstance`] per tick. A single base line segment (two vertices at
    /// `t = 0.0` and `t = 1.0`) is drawn once by the GPU and instanced
    /// across all tick positions using this per-instance data.
    ///
    /// # Vertex count comparison
    ///
    /// | Approach | Data per tick | Draw calls |
    /// |----------|--------------|------------|
    /// | Vertex pairs (`generate_tick_vertices`) | 2 × `Vertex` (48 B) | 1 |
    /// | Instanced (`generate_tick_instances`) | 1 × `TickInstance` (32 B) | 1 per tick type |
    ///
    /// # Parameters
    ///
    /// Same as [`generate_axis_vertices`](Self::generate_axis_vertices).
    ///
    /// # Returns
    ///
    /// A `Vec<TickInstance>` containing one entry per visible tick. Major
    /// and minor ticks are interleaved in the output (major first, then
    /// minor). If separate draw calls per tick type are desired, use
    /// [`generate_major_tick_instances`](Self::generate_major_tick_instances)
    /// and [`generate_minor_tick_instances`](Self::generate_minor_tick_instances).
    pub fn generate_tick_instances(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
    ) -> Vec<TickInstance> {
        let mut instances = Vec::new();

        if config.show_major_ticks {
            let tick_positions =
                Self::compute_tick_positions(bounds, scale, config.target_tick_count, false, 0);
            self.append_tick_instances(
                &mut instances,
                bounds,
                &tick_positions,
                config.major_tick_length,
                config,
                position,
                viewport_size,
            );
        }

        if config.show_minor_ticks {
            let tick_positions = Self::compute_tick_positions(
                bounds,
                scale,
                config.target_tick_count,
                true,
                config.minor_tick_subdivisions,
            );
            self.append_tick_instances(
                &mut instances,
                bounds,
                &tick_positions,
                config.minor_tick_length,
                config,
                position,
                viewport_size,
            );
        }

        instances
    }

    /// Generate instance data for major ticks only.
    ///
    /// Useful when major and minor ticks are rendered with separate draw
    /// calls (e.g. different pipeline states or line widths).
    pub fn generate_major_tick_instances(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
    ) -> Vec<TickInstance> {
        let mut instances = Vec::new();
        if config.show_major_ticks {
            let tick_positions =
                Self::compute_tick_positions(bounds, scale, config.target_tick_count, false, 0);
            self.append_tick_instances(
                &mut instances,
                bounds,
                &tick_positions,
                config.major_tick_length,
                config,
                position,
                viewport_size,
            );
        }
        instances
    }

    /// Generate instance data for minor ticks only.
    ///
    /// See [`generate_major_tick_instances`](Self::generate_major_tick_instances).
    pub fn generate_minor_tick_instances(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
    ) -> Vec<TickInstance> {
        let mut instances = Vec::new();
        if config.show_minor_ticks {
            let tick_positions = Self::compute_tick_positions(
                bounds,
                scale,
                config.target_tick_count,
                true,
                config.minor_tick_subdivisions,
            );
            self.append_tick_instances(
                &mut instances,
                bounds,
                &tick_positions,
                config.minor_tick_length,
                config,
                position,
                viewport_size,
            );
        }
        instances
    }
    ///
    /// Returns an [`AxisLabel`] for each major tick position, containing the
    /// formatted text, screen-space position, NDC position, and recommended
    /// text anchor. The caller feeds these into
    /// [`TextRenderer::render_text()`](crate::text::TextRenderer::render_text)
    /// or [`TextRenderer::queue_text()`](crate::text::TextRenderer::queue_text).
    ///
    /// # Parameters
    ///
    /// * `bounds` - The start and end points of the axis line in clip space.
    /// * `config` - Appearance settings (used for tick length to calculate
    ///   label offset).
    /// * `position` - Where the axis sits relative to the chart area.
    /// * `scale` - Optional scale for automatic tick generation and value
    ///   mapping. When `None`, fallback positions at 0%, 20%, …, 100% are
    ///   used with values equal to the normalized positions.
    /// * `viewport_size` - `(width, height)` in pixels.
    /// * `formatter` - Optional label formatter. When `None`, a default
    ///   [`NumericFormatter`] is used.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::axis::{AxisRenderer, AxisBounds, AxisConfiguration, AxisPosition};
    /// use gup::shader_function::Vec2;
    ///
    /// let renderer = AxisRenderer::new();
    /// let bounds = AxisBounds::new(
    ///     Vec2 { x: -0.8, y: -0.8 },
    ///     Vec2 { x: 0.8, y: -0.8 },
    ///     50.0,
    /// );
    /// let config = AxisConfiguration::default();
    ///
    /// let labels = renderer.generate_label_data(
    ///     &bounds,
    ///     &config,
    ///     AxisPosition::Bottom,
    ///     None,
    ///     (800.0, 600.0),
    ///     None,
    /// );
    ///
    /// // Each label has text, screen position, and anchor ready for TextRenderer
    /// for label in &labels {
    ///     println!("{} at ({}, {})", label.text, label.screen_position.x, label.screen_position.y);
    /// }
    /// ```
    pub fn generate_label_data(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
        formatter: Option<&dyn LabelFormatter>,
    ) -> Vec<AxisLabel> {
        let default_formatter = NumericFormatter::default();
        let formatter: &dyn LabelFormatter = formatter.unwrap_or(&default_formatter);

        // Get normalized tick positions (0.0..1.0)
        let normalized_positions =
            Self::compute_tick_positions(bounds, scale, config.target_tick_count, false, 0);

        // Compute the label offset in screen pixels.
        // Labels are placed beyond the tick end, with a small gap.
        let label_gap_px = 4.0;
        let tick_px = config.major_tick_length;
        let offset_px = tick_px + label_gap_px;

        let direction = bounds.direction();
        let axis_length = bounds.length();

        // Determine text anchor based on axis position
        let anchor = match position {
            AxisPosition::Bottom => TextAnchor::TopCenter,
            AxisPosition::Top => TextAnchor::BottomCenter,
            AxisPosition::Left => TextAnchor::CenterRight,
            AxisPosition::Right => TextAnchor::CenterLeft,
        };

        let (vw, vh) = viewport_size;

        normalized_positions
            .iter()
            .filter(|&&t| (0.0..=1.0).contains(&t))
            .map(|&t| {
                // NDC position on the axis line
                let ndc = Vec2 {
                    x: bounds.start.x + direction.x * axis_length * t,
                    y: bounds.start.y + direction.y * axis_length * t,
                };

                // Convert NDC to screen coordinates (origin top-left, Y down)
                let screen_on_axis = Vec2 {
                    x: (ndc.x + 1.0) * 0.5 * vw,
                    y: (1.0 - ndc.y) * 0.5 * vh,
                };

                // Apply label offset in screen space (away from chart area)
                let screen_position = match position {
                    AxisPosition::Bottom => Vec2 {
                        x: screen_on_axis.x,
                        y: screen_on_axis.y + offset_px, // below axis
                    },
                    AxisPosition::Top => Vec2 {
                        x: screen_on_axis.x,
                        y: screen_on_axis.y - offset_px, // above axis
                    },
                    AxisPosition::Left => Vec2 {
                        x: screen_on_axis.x - offset_px, // left of axis
                        y: screen_on_axis.y,
                    },
                    AxisPosition::Right => Vec2 {
                        x: screen_on_axis.x + offset_px, // right of axis
                        y: screen_on_axis.y,
                    },
                };

                // Determine the data value for formatting.
                // If a scale is provided, map the normalized position back to
                // a domain value. Otherwise, use the normalized position itself.
                let value = if let Some(scale) = scale {
                    scale.denormalize(t as f64)
                } else {
                    t as f64
                };

                let text = formatter.format_value(value);

                AxisLabel {
                    text,
                    screen_position,
                    ndc_position: ndc,
                    anchor,
                    value,
                }
            })
            .collect()
    }

    /// Generate label data with viewport culling and LOD-based limiting.
    ///
    /// This is the performance-optimized label generation path that:
    /// 1. Generates candidate labels the same way as [`generate_label_data`].
    /// 2. Culls labels whose screen position falls outside the viewport.
    /// 3. Caps the label count based on the current LOD level.
    ///
    /// Returns `(visible_labels, culled_count)`.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_labels_culled(
        &self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        scale: Option<&dyn Scale>,
        viewport_size: (f32, f32),
        formatter: Option<&dyn LabelFormatter>,
        viewport_bounds: &crate::axis_performance::ViewportBounds,
        lod: crate::axis_performance::LODLevel,
    ) -> (Vec<AxisLabel>, usize) {
        // Generate all candidate labels
        let all_labels =
            self.generate_label_data(bounds, config, position, scale, viewport_size, formatter);

        // Cull labels outside viewport
        let margin = 20.0; // Allow labels slightly outside viewport
        let screen_positions: Vec<[f32; 2]> = all_labels
            .iter()
            .map(|l| [l.screen_position.x, l.screen_position.y])
            .collect();

        let visible_indices =
            crate::axis_performance::cull_label_indices(&screen_positions, viewport_bounds, margin);

        let mut visible: Vec<AxisLabel> = visible_indices
            .iter()
            .map(|&i| all_labels[i].clone())
            .collect();

        let culled = all_labels.len() - visible.len();

        // Apply LOD label cap
        if let Some(max) = lod.max_labels() {
            if visible.len() > max && max > 0 {
                // Keep evenly spaced subset
                let step = visible.len() as f32 / max as f32;
                let mut kept = Vec::with_capacity(max);
                for i in 0..max {
                    let idx = (i as f32 * step) as usize;
                    if idx < visible.len() {
                        kept.push(visible[idx].clone());
                    }
                }
                visible = kept;
            } else if max == 0 {
                visible.clear();
            }
        }

        (visible, culled)
    }

    // ---- internal helpers ----

    /// Append the axis line as two vertices.
    fn append_line_vertices(
        &self,
        vertices: &mut Vec<Vertex>,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
    ) {
        vertices.push(Vertex {
            position: [bounds.start.x, bounds.start.y],
            color: config.line_color,
        });
        vertices.push(Vertex {
            position: [bounds.end.x, bounds.end.y],
            color: config.line_color,
        });
    }

    /// Append tick mark vertices. Each tick is a pair of vertices (LineList).
    ///
    /// Ticks extend outward from the chart area:
    /// - Bottom axis: ticks go downward (-Y)
    /// - Top axis: ticks go upward (+Y)
    /// - Left axis: ticks go leftward (-X)
    /// - Right axis: ticks go rightward (+X)
    #[allow(clippy::too_many_arguments)]
    fn append_tick_vertices(
        &self,
        vertices: &mut Vec<Vertex>,
        bounds: &AxisBounds,
        normalized_positions: &[f32],
        tick_length_px: f32,
        config: &AxisConfiguration,
        position: AxisPosition,
        viewport_size: (f32, f32),
    ) {
        // Convert pixel tick length to NDC.
        // NDC range is 2.0 across each axis dimension.
        let tick_length_ndc = match position {
            AxisPosition::Top | AxisPosition::Bottom => tick_length_px * 2.0 / viewport_size.1,
            AxisPosition::Left | AxisPosition::Right => tick_length_px * 2.0 / viewport_size.0,
        };

        let direction = bounds.direction();
        let axis_length = bounds.length();

        for &t in normalized_positions {
            if !(0.0..=1.0).contains(&t) {
                continue;
            }

            // Position along the axis line
            let on_axis = Vec2 {
                x: bounds.start.x + direction.x * axis_length * t,
                y: bounds.start.y + direction.y * axis_length * t,
            };

            // Tick extends outward from the chart area
            let tick_end = match position {
                AxisPosition::Bottom => Vec2 {
                    x: on_axis.x,
                    y: on_axis.y - tick_length_ndc,
                },
                AxisPosition::Top => Vec2 {
                    x: on_axis.x,
                    y: on_axis.y + tick_length_ndc,
                },
                AxisPosition::Left => Vec2 {
                    x: on_axis.x - tick_length_ndc,
                    y: on_axis.y,
                },
                AxisPosition::Right => Vec2 {
                    x: on_axis.x + tick_length_ndc,
                    y: on_axis.y,
                },
            };

            vertices.push(Vertex {
                position: [on_axis.x, on_axis.y],
                color: config.line_color,
            });
            vertices.push(Vertex {
                position: [tick_end.x, tick_end.y],
                color: config.line_color,
            });
        }
    }

    /// Append instanced tick data. Each tick becomes a single [`TickInstance`].
    #[allow(clippy::too_many_arguments)]
    fn append_tick_instances(
        &self,
        instances: &mut Vec<TickInstance>,
        bounds: &AxisBounds,
        normalized_positions: &[f32],
        tick_length_px: f32,
        config: &AxisConfiguration,
        position: AxisPosition,
        viewport_size: (f32, f32),
    ) {
        let tick_vector = Self::compute_tick_vector(tick_length_px, position, viewport_size);
        let direction = bounds.direction();
        let axis_length = bounds.length();

        for &t in normalized_positions {
            if !(0.0..=1.0).contains(&t) {
                continue;
            }

            let on_axis = [
                bounds.start.x + direction.x * axis_length * t,
                bounds.start.y + direction.y * axis_length * t,
            ];

            instances.push(TickInstance {
                position: on_axis,
                tick_vector,
                color: config.line_color,
            });
        }
    }

    /// Convert a pixel-space tick length and axis position into an NDC tick
    /// direction vector.
    fn compute_tick_vector(
        tick_length_px: f32,
        position: AxisPosition,
        viewport_size: (f32, f32),
    ) -> [f32; 2] {
        let tick_length_ndc = match position {
            AxisPosition::Top | AxisPosition::Bottom => tick_length_px * 2.0 / viewport_size.1,
            AxisPosition::Left | AxisPosition::Right => tick_length_px * 2.0 / viewport_size.0,
        };

        match position {
            AxisPosition::Bottom => [0.0, -tick_length_ndc],
            AxisPosition::Top => [0.0, tick_length_ndc],
            AxisPosition::Left => [-tick_length_ndc, 0.0],
            AxisPosition::Right => [tick_length_ndc, 0.0],
        }
    }

    /// Compute normalized tick positions (0.0..1.0) along an axis.
    ///
    /// When `minor` is true, returns minor tick positions (between major ticks).
    /// When `minor` is false, returns major tick positions.
    fn compute_tick_positions(
        bounds: &AxisBounds,
        scale: Option<&dyn Scale>,
        target_tick_count: Option<usize>,
        minor: bool,
        minor_subdivisions: usize,
    ) -> Vec<f32> {
        let pixel_range = bounds.length();

        if let Some(scale) = scale {
            let generator = LinearTickGenerator::default();
            let major_ticks = generator.generate_major_ticks(scale, pixel_range, target_tick_count);

            if minor {
                let minor_ticks =
                    generator.generate_minor_ticks(scale, &major_ticks, minor_subdivisions);
                minor_ticks
                    .iter()
                    .map(|&v| scale.normalize(v) as f32)
                    .collect()
            } else {
                major_ticks
                    .iter()
                    .map(|&v| scale.normalize(v) as f32)
                    .collect()
            }
        } else if minor {
            Vec::new()
        } else {
            // Fallback: evenly spaced ticks
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        }
    }

    // ---- Legacy placeholder methods (kept for backward compatibility) ----

    /// Render the main axis line using GPU-accelerated Line marks.
    ///
    /// **Note:** This is a legacy placeholder that does not actually render
    /// anything. Use [`generate_axis_vertices`](Self::generate_axis_vertices)
    /// instead to get vertex data you can draw in your own render pass.
    pub fn render_axis_line(
        &self,
        _context: &mut RenderContext,
        _bounds: &AxisBounds,
        _config: &AxisConfiguration,
    ) -> GupResult<()> {
        // Legacy placeholder - use generate_axis_vertices() instead
        Ok(())
    }

    /// Render tick marks at specified positions.
    ///
    /// **Note:** This is a legacy placeholder that does not actually render
    /// anything. Use [`generate_axis_vertices`](Self::generate_axis_vertices)
    /// instead to get vertex data you can draw in your own render pass.
    pub fn render_ticks(
        &self,
        _context: &mut RenderContext,
        _bounds: &AxisBounds,
        _tick_positions: &[Vec2],
        _tick_length: f32,
        _config: &AxisConfiguration,
    ) -> GupResult<()> {
        // Legacy placeholder - use generate_axis_vertices() instead
        Ok(())
    }
}

impl Default for AxisRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU render pipeline for instanced tick marks.
///
/// `TickPipeline` owns a wgpu [`RenderPipeline`](wgpu::RenderPipeline)
/// configured for instanced `LineList` rendering of tick marks. A
/// two-vertex base geometry (a unit parameter from 0.0 to 1.0) is
/// instanced across all tick positions via a [`TickInstance`] buffer.
///
/// # Usage
///
/// ```rust,no_run
/// use gup::axis::{TickPipeline, TickInstance, AxisRenderer, AxisBounds, AxisConfiguration, AxisPosition};
/// use gup::shader_function::Vec2;
/// use gup::RenderContext;
/// use std::sync::Arc;
///
/// # async fn example() -> gup::error::GupResult<()> {
/// let context = Arc::new(RenderContext::new().await?);
/// let tick_pipeline = TickPipeline::new(context.device(), wgpu::TextureFormat::Bgra8Unorm);
///
/// // Generate instances
/// let renderer = AxisRenderer::new();
/// let bounds = AxisBounds::new(
///     Vec2 { x: -0.8, y: -0.8 },
///     Vec2 { x: 0.8, y: -0.8 },
///     50.0,
/// );
/// let config = AxisConfiguration::default();
/// let instances = renderer.generate_tick_instances(
///     &bounds, &config, AxisPosition::Bottom, None, (800.0, 600.0),
/// );
///
/// // Upload and draw
/// let (base_buf, inst_buf) = tick_pipeline.upload(context.device(), context.queue(), &instances);
/// // ... in a render pass:
/// // tick_pipeline.draw(&mut render_pass, &base_buf, &inst_buf, instances.len() as u32);
/// # Ok(())
/// # }
/// ```
pub struct TickPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl std::fmt::Debug for TickPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TickPipeline")
            .field("pipeline", &"<wgpu::RenderPipeline>")
            .finish()
    }
}

impl TickPipeline {
    /// Create a new instanced tick pipeline for the given surface format.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tick_instanced_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tick_instanced.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tick_instanced_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        // Base vertex buffer: a single f32 per vertex (the parameter t)
        let base_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<f32>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32,
            }],
        };

        let instance_layout = TickInstance::instance_buffer_layout();

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tick_instanced_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[base_vertex_layout, instance_layout],
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
                topology: wgpu::PrimitiveTopology::LineList,
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

    /// Upload the base geometry and instance data to GPU buffers.
    ///
    /// Returns `(base_vertex_buffer, instance_buffer)`.
    pub fn upload(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        instances: &[TickInstance],
    ) -> (wgpu::Buffer, wgpu::Buffer) {
        use wgpu::util::{BufferInitDescriptor, DeviceExt};

        // Base geometry: two floats [0.0, 1.0] forming one line segment
        let base_data: [f32; 2] = [0.0, 1.0];
        let base_buf = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("tick_base_vertices"),
            contents: bytemuck::cast_slice(&base_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let inst_buf = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("tick_instance_buffer"),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        (base_buf, inst_buf)
    }

    /// Record instanced draw commands into an active render pass.
    ///
    /// `base_buf` contains the two-vertex base geometry, `inst_buf`
    /// contains the per-tick instance data, and `instance_count` is the
    /// number of ticks to draw.
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        base_buf: &'a wgpu::Buffer,
        inst_buf: &'a wgpu::Buffer,
        instance_count: u32,
    ) {
        if instance_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, base_buf.slice(..));
        render_pass.set_vertex_buffer(1, inst_buf.slice(..));
        render_pass.draw(0..2, 0..instance_count);
    }

    /// Access the underlying render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

/// Cached render pipeline for axis-line drawing (`LineList` topology).
///
/// Axis lines (the spine of each axis) use a simple position+color vertex
/// layout identical to [`crate::render::Vertex`]. This pipeline is created
/// once and reused across frames, eliminating the overhead of rebuilding a
/// `wgpu::RenderPipeline` every frame.
///
/// # Usage
///
/// ```rust,no_run
/// use gup::axis::AxisLinePipeline;
/// use gup::render::Vertex;
/// use gup::RenderContext;
/// use std::sync::Arc;
///
/// # async fn example() -> gup::error::GupResult<()> {
/// let context = Arc::new(RenderContext::new().await?);
/// let pipeline = AxisLinePipeline::new(context.device(), wgpu::TextureFormat::Bgra8Unorm);
///
/// // Upload axis line vertices
/// let vertices: Vec<Vertex> = vec![]; // generated from AxisGeometry::line_vertices
/// let buf = pipeline.upload(context.device(), &vertices);
///
/// // ... in a render pass:
/// // pipeline.draw(&mut render_pass, &buf, vertices.len() as u32);
/// # Ok(())
/// # }
/// ```
pub struct AxisLinePipeline {
    pipeline: wgpu::RenderPipeline,
}

impl std::fmt::Debug for AxisLinePipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AxisLinePipeline")
            .field("pipeline", &"<wgpu::RenderPipeline>")
            .finish()
    }
}

impl AxisLinePipeline {
    /// Create a new axis-line pipeline for the given surface format.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("axis_line_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/basic.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("axis_line_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::render::Vertex>() as u64,
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
            label: Some("axis_line_pipeline"),
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
                topology: wgpu::PrimitiveTopology::LineList,
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

    /// Upload axis-line vertices to a GPU buffer.
    pub fn upload(
        &self,
        device: &wgpu::Device,
        vertices: &[crate::render::Vertex],
    ) -> wgpu::Buffer {
        use wgpu::util::{BufferInitDescriptor, DeviceExt};

        device.create_buffer_init(&BufferInitDescriptor {
            label: Some("axis_line_vertex_buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    /// Record axis-line draw commands into an active render pass.
    pub fn draw<'a>(
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

    /// Access the underlying render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::Vec2;

    #[test]
    fn test_axis_position() {
        assert!(AxisPosition::Top.is_horizontal());
        assert!(AxisPosition::Bottom.is_horizontal());
        assert!(AxisPosition::Left.is_vertical());
        assert!(AxisPosition::Right.is_vertical());
    }

    #[test]
    fn test_axis_bounds() {
        let start = Vec2 { x: 0.0, y: 0.0 };
        let end = Vec2 { x: 100.0, y: 0.0 };
        let bounds = AxisBounds::new(start, end, 50.0);

        assert_eq!(bounds.length(), 100.0);
        assert_eq!(bounds.available_margin, 50.0);

        let direction = bounds.direction();
        assert!((direction.x - 1.0).abs() < 0.001);
        assert!((direction.y - 0.0).abs() < 0.001);

        let normal = bounds.normal();
        assert!((normal.x - 0.0).abs() < 0.001);
        assert!((normal.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_axis_configuration_default() {
        let config = AxisConfiguration::default();
        assert!(config.show_line);
        assert!(config.show_major_ticks);
        assert!(!config.show_minor_ticks);
        assert_eq!(config.major_tick_length, 6.0);
        assert_eq!(config.minor_tick_length, 3.0);
        assert_eq!(config.line_color, [0.2, 0.2, 0.2, 1.0]);
        assert_eq!(config.line_width, 1.0);
        assert_eq!(config.target_tick_count, None);
        assert_eq!(config.minor_tick_subdivisions, 5);
    }

    #[test]
    fn test_axis_configuration_builder() {
        let config = AxisConfiguration::default()
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_line_width(2.0)
            .with_tick_lengths(8.0, 4.0)
            .without_minor_ticks();

        assert_eq!(config.line_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(config.line_width, 2.0);
        assert_eq!(config.major_tick_length, 8.0);
        assert_eq!(config.minor_tick_length, 4.0);
        assert!(!config.show_minor_ticks);
    }

    #[test]
    fn test_linear_axis_creation() {
        let config = AxisConfiguration::default();
        let axis = LinearAxis::new(AxisPosition::Bottom, config.clone());

        assert_eq!(axis.position(), AxisPosition::Bottom);
        assert_eq!(axis.configuration().line_width, config.line_width);
    }

    #[test]
    fn test_linear_axis_with_position() {
        let axis = LinearAxis::with_position(AxisPosition::Left);
        assert_eq!(axis.position(), AxisPosition::Left);
        assert!(axis.configuration().show_line);
    }

    #[test]
    fn test_linear_axis_tick_positions() {
        let axis = LinearAxis::with_position(AxisPosition::Bottom);
        let positions = axis.get_tick_positions(None, 800.0);

        // Should have basic tick positions
        assert_eq!(positions.len(), 6);
        assert_eq!(positions[0], 0.0);
        assert_eq!(positions[5], 1.0);
    }

    #[test]
    fn test_linear_axis_margin_calculation() {
        let axis = LinearAxis::with_position(AxisPosition::Bottom);
        let margin = axis.calculate_margin(None);

        // Should include base margin plus tick length
        assert!(margin > 40.0); // Base margin for horizontal axis
    }

    #[test]
    fn test_linear_axis_calculate_tick_positions() {
        let axis = LinearAxis::with_position(AxisPosition::Bottom);
        let start = Vec2 { x: 0.0, y: 100.0 };
        let end = Vec2 { x: 200.0, y: 100.0 };
        let bounds = AxisBounds::new(start, end, 50.0);

        let tick_positions = axis.calculate_tick_positions(&bounds, None);

        // Should have 6 tick positions along the axis
        assert_eq!(tick_positions.len(), 6);

        // First tick should be at start
        assert!((tick_positions[0].x - 0.0).abs() < 0.001);
        assert!((tick_positions[0].y - 100.0).abs() < 0.001);

        // Last tick should be at end
        assert!((tick_positions[5].x - 200.0).abs() < 0.001);
        assert!((tick_positions[5].y - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_axis_renderer_creation() {
        let _renderer = AxisRenderer::new();
        // Should create without error
    }

    #[test]
    fn test_axis_bounds_zero_length() {
        let start = Vec2 { x: 50.0, y: 50.0 };
        let end = Vec2 { x: 50.0, y: 50.0 };
        let bounds = AxisBounds::new(start, end, 20.0);

        assert_eq!(bounds.length(), 0.0);

        // Should provide default direction for zero-length axis
        let direction = bounds.direction();
        assert_eq!(direction.x, 1.0);
        assert_eq!(direction.y, 0.0);
    }

    #[test]
    fn test_axis_configuration_without_methods() {
        let config = AxisConfiguration::default().without_ticks().without_line();

        assert!(!config.show_line);
        assert!(!config.show_major_ticks);
        assert!(!config.show_minor_ticks);
    }

    #[test]
    fn test_linear_axis_configuration_update() {
        let mut axis = LinearAxis::with_position(AxisPosition::Top);
        let new_config = AxisConfiguration::default().with_color([0.0, 1.0, 0.0, 1.0]);

        axis.set_configuration(new_config);
        assert_eq!(axis.configuration().line_color, [0.0, 1.0, 0.0, 1.0]);
    }

    // ---- Tests for new vertex generation methods ----

    #[test]
    fn test_generate_axis_vertices_line_only() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default().without_ticks();

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // Line only: 2 vertices (one line segment)
        assert_eq!(vertices.len(), 2);
        assert_eq!(vertices[0].position, [-0.8, -0.8]);
        assert_eq!(vertices[1].position, [0.8, -0.8]);
        assert_eq!(vertices[0].color, config.line_color);
    }

    #[test]
    fn test_generate_axis_vertices_with_ticks() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default(); // show_line + show_major_ticks

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // 2 (line) + 6 * 2 (6 default major ticks, each 2 vertices) = 14
        assert_eq!(vertices.len(), 14);
    }

    #[test]
    fn test_bottom_axis_ticks_extend_downward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.5 }, Vec2 { x: 0.8, y: -0.5 }, 50.0);
        let config = AxisConfiguration::default()
            .without_line()
            .with_tick_lengths(10.0, 5.0);

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // 6 ticks * 2 verts = 12
        assert_eq!(vertices.len(), 12);

        // Each tick: start on axis, end below (smaller Y)
        for pair in vertices.chunks(2) {
            let on_axis = pair[0];
            let tick_end = pair[1];
            assert!(
                (on_axis.position[1] - (-0.5)).abs() < 0.001,
                "Tick start should be on axis line"
            );
            assert!(
                tick_end.position[1] < on_axis.position[1],
                "Bottom axis tick should extend downward (Y decreases)"
            );
        }
    }

    #[test]
    fn test_top_axis_ticks_extend_upward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: 0.5 }, Vec2 { x: 0.8, y: 0.5 }, 50.0);
        let config = AxisConfiguration::default().without_line();

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Top,
            None,
            (800.0, 600.0),
        );

        for pair in vertices.chunks(2) {
            let on_axis = pair[0];
            let tick_end = pair[1];
            assert!(
                (on_axis.position[1] - 0.5).abs() < 0.001,
                "Tick start should be on axis line"
            );
            assert!(
                tick_end.position[1] > on_axis.position[1],
                "Top axis tick should extend upward (Y increases)"
            );
        }
    }

    #[test]
    fn test_left_axis_ticks_extend_leftward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.5, y: -0.8 }, Vec2 { x: -0.5, y: 0.8 }, 50.0);
        let config = AxisConfiguration::default().without_line();

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Left,
            None,
            (800.0, 600.0),
        );

        for pair in vertices.chunks(2) {
            let on_axis = pair[0];
            let tick_end = pair[1];
            assert!(
                (on_axis.position[0] - (-0.5)).abs() < 0.001,
                "Tick start should be on axis line"
            );
            assert!(
                tick_end.position[0] < on_axis.position[0],
                "Left axis tick should extend leftward (X decreases)"
            );
        }
    }

    #[test]
    fn test_right_axis_ticks_extend_rightward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: 0.5, y: -0.8 }, Vec2 { x: 0.5, y: 0.8 }, 50.0);
        let config = AxisConfiguration::default().without_line();

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Right,
            None,
            (800.0, 600.0),
        );

        for pair in vertices.chunks(2) {
            let on_axis = pair[0];
            let tick_end = pair[1];
            assert!(
                (on_axis.position[0] - 0.5).abs() < 0.001,
                "Tick start should be on axis line"
            );
            assert!(
                tick_end.position[0] > on_axis.position[0],
                "Right axis tick should extend rightward (X increases)"
            );
        }
    }

    #[test]
    fn test_generate_no_vertices_when_all_hidden() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default().without_line().without_ticks();

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        assert!(vertices.is_empty());
    }

    #[test]
    fn test_generate_line_vertices_only() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.5, y: 0.0 }, Vec2 { x: 0.5, y: 0.0 }, 50.0);
        let config = AxisConfiguration::default().with_color([1.0, 0.0, 0.0, 1.0]);

        let vertices = renderer.generate_line_vertices(&bounds, &config);

        assert_eq!(vertices.len(), 2);
        assert_eq!(vertices[0].position, [-0.5, 0.0]);
        assert_eq!(vertices[1].position, [0.5, 0.0]);
        assert_eq!(vertices[0].color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_generate_tick_vertices_only() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default(); // major ticks on, minor off

        let vertices = renderer.generate_tick_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // 6 default ticks * 2 verts = 12
        assert_eq!(vertices.len(), 12);
    }

    #[test]
    fn test_tick_length_scales_with_viewport() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: 0.0 }, Vec2 { x: 0.8, y: 0.0 }, 50.0);
        let config = AxisConfiguration::default()
            .without_line()
            .with_tick_lengths(10.0, 5.0);

        // Larger viewport = smaller NDC tick length for the same pixel size
        let verts_small = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (400.0, 300.0),
        );
        let verts_large = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // Both should have same number of vertices
        assert_eq!(verts_small.len(), verts_large.len());

        // Tick length in NDC should be larger for smaller viewport
        let tick_ndc_small = (verts_small[0].position[1] - verts_small[1].position[1]).abs();
        let tick_ndc_large = (verts_large[0].position[1] - verts_large[1].position[1]).abs();
        assert!(
            tick_ndc_small > tick_ndc_large,
            "Smaller viewport should produce larger NDC tick length"
        );
    }

    #[test]
    fn test_vertex_colors_match_config() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let color = [0.3, 0.6, 0.9, 1.0];
        let config = AxisConfiguration::default().with_color(color);

        let vertices = renderer.generate_axis_vertices(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        for v in &vertices {
            assert_eq!(
                v.color, color,
                "All vertices should use the configured color"
            );
        }
    }

    // ---- Tests for label generation ----

    #[test]
    fn test_generate_label_data_default_formatter() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // 6 default tick positions (0.0, 0.2, 0.4, 0.6, 0.8, 1.0)
        assert_eq!(labels.len(), 6);

        // Each label should have non-empty text
        for label in &labels {
            assert!(!label.text.is_empty(), "Label text should not be empty");
        }

        // Values should match the normalized positions
        assert!((labels[0].value - 0.0).abs() < 0.001);
        assert!((labels[5].value - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_label_anchors_per_position() {
        let renderer = AxisRenderer::new();
        let config = AxisConfiguration::default();
        let vp = (800.0, 600.0);

        let h_bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let v_bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: -0.8, y: 0.8 }, 50.0);

        let bottom =
            renderer.generate_label_data(&h_bounds, &config, AxisPosition::Bottom, None, vp, None);
        let top =
            renderer.generate_label_data(&h_bounds, &config, AxisPosition::Top, None, vp, None);
        let left =
            renderer.generate_label_data(&v_bounds, &config, AxisPosition::Left, None, vp, None);
        let right =
            renderer.generate_label_data(&v_bounds, &config, AxisPosition::Right, None, vp, None);

        assert_eq!(bottom[0].anchor, TextAnchor::TopCenter);
        assert_eq!(top[0].anchor, TextAnchor::BottomCenter);
        assert_eq!(left[0].anchor, TextAnchor::CenterRight);
        assert_eq!(right[0].anchor, TextAnchor::CenterLeft);
    }

    #[test]
    fn test_label_ndc_positions_on_axis() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.5 }, Vec2 { x: 0.8, y: -0.5 }, 50.0);
        let config = AxisConfiguration::default();

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // First label NDC should be at axis start
        assert!((labels[0].ndc_position.x - (-0.8)).abs() < 0.001);
        assert!((labels[0].ndc_position.y - (-0.5)).abs() < 0.001);

        // Last label NDC should be at axis end
        assert!((labels[5].ndc_position.x - 0.8).abs() < 0.001);
        assert!((labels[5].ndc_position.y - (-0.5)).abs() < 0.001);
    }

    #[test]
    fn test_label_screen_position_ndc_conversion() {
        let renderer = AxisRenderer::new();
        // Place axis at NDC origin for easy math
        let bounds = AxisBounds::new(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 0.0, y: 0.0 }, 50.0);
        let config = AxisConfiguration::default().with_tick_lengths(0.0, 0.0);

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // NDC (0,0) maps to screen center: (400, 300).
        // With tick_length=0 and gap=4px, bottom offset = 4px.
        for label in &labels {
            assert!(
                (label.screen_position.x - 400.0).abs() < 0.01,
                "screen_x should be viewport center"
            );
            assert!(
                (label.screen_position.y - 304.0).abs() < 0.01,
                "screen_y should be center + 4px gap: got {}",
                label.screen_position.y,
            );
        }
    }

    #[test]
    fn test_label_bottom_offset_below_axis() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.5 }, Vec2 { x: 0.8, y: -0.5 }, 50.0);
        let config = AxisConfiguration::default().with_tick_lengths(6.0, 3.0);

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // Screen Y for NDC y=-0.5: (1.0 - (-0.5)) * 0.5 * 600 = 450
        let axis_screen_y = (1.0 - (-0.5)) * 0.5 * 600.0;
        for label in &labels {
            assert!(
                label.screen_position.y > axis_screen_y,
                "Bottom axis labels should be below the axis line"
            );
        }
    }

    #[test]
    fn test_label_top_offset_above_axis() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: 0.5 }, Vec2 { x: 0.8, y: 0.5 }, 50.0);
        let config = AxisConfiguration::default();

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Top,
            None,
            (800.0, 600.0),
            None,
        );

        let axis_screen_y = (1.0 - 0.5) * 0.5 * 600.0; // 150
        for label in &labels {
            assert!(
                label.screen_position.y < axis_screen_y,
                "Top axis labels should be above the axis line"
            );
        }
    }

    #[test]
    fn test_label_left_offset_left_of_axis() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.5, y: -0.8 }, Vec2 { x: -0.5, y: 0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Left,
            None,
            (800.0, 600.0),
            None,
        );

        let axis_screen_x = ((-0.5) + 1.0) * 0.5 * 800.0; // 200
        for label in &labels {
            assert!(
                label.screen_position.x < axis_screen_x,
                "Left axis labels should be to the left of the axis line"
            );
        }
    }

    #[test]
    fn test_label_right_offset_right_of_axis() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: 0.5, y: -0.8 }, Vec2 { x: 0.5, y: 0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Right,
            None,
            (800.0, 600.0),
            None,
        );

        let axis_screen_x = (0.5 + 1.0) * 0.5 * 800.0; // 600
        for label in &labels {
            assert!(
                label.screen_position.x > axis_screen_x,
                "Right axis labels should be to the right of the axis line"
            );
        }
    }

    #[test]
    fn test_label_custom_formatter() {
        use crate::label::NumericFormatter;

        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();
        let pct_formatter = NumericFormatter::percentage(1, true);

        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            Some(&pct_formatter),
        );

        assert_eq!(labels.len(), 6);
        // First tick value is 0.0 -> 0% (multiplied by 100, 1 decimal)
        assert!(
            labels[0].text.contains('0'),
            "0.0 formatted as percentage should contain '0': got '{}'",
            labels[0].text,
        );
        // Last tick value is 1.0 -> 100%
        assert!(
            labels[5].text.contains("100"),
            "1.0 formatted as percentage should contain '100': got '{}'",
            labels[5].text,
        );
    }

    #[test]
    fn test_label_count_without_scale() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default().with_tick_count(3);

        // Without a scale, tick count config is ignored (falls back to 6)
        let labels = renderer.generate_label_data(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );
        assert_eq!(labels.len(), 6);
    }

    // ---- Tests for cached generation and LOD integration ----

    #[test]
    fn test_cached_generation_returns_same_data() {
        let mut renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let first = renderer
            .generate_axis_vertices_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                (800.0, 600.0),
                None,
            )
            .to_vec();

        let second = renderer
            .generate_axis_vertices_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                (800.0, 600.0),
                None,
            )
            .to_vec();

        assert_eq!(first.len(), second.len());
        assert_eq!(first, second);
        // Second call should be a cache hit
        assert!(renderer.geometry_cache().hit_rate() > 0.0);
    }

    #[test]
    fn test_cached_generation_with_small_axis_uses_lower_lod() {
        let mut renderer = AxisRenderer::new();
        // Very small axis in NDC → should get Minimal LOD
        let bounds = AxisBounds::new(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 0.01, y: 0.0 }, 50.0);
        let config = AxisConfiguration::default();

        let verts = renderer.generate_axis_vertices_cached(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // At Minimal LOD, only the axis line is shown (2 vertices)
        assert_eq!(
            verts.len(),
            2,
            "Minimal LOD should only show axis line (2 vertices)"
        );
    }

    #[test]
    fn test_cached_generation_with_large_axis_uses_high_lod() {
        let mut renderer = AxisRenderer::new();
        // Large axis in NDC → should get High LOD
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let verts = renderer.generate_axis_vertices_cached(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // At High LOD, should have line + major ticks: 2 + 6*2 = 14
        assert_eq!(
            verts.len(),
            14,
            "High LOD should show axis line and all major ticks"
        );
    }

    #[test]
    fn test_axis_pixel_length_calculation() {
        // Horizontal axis from -0.8 to 0.8 in NDC on 800x600 viewport
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: 0.0 }, Vec2 { x: 0.8, y: 0.0 }, 50.0);
        let pixel_len = AxisRenderer::axis_pixel_length(&bounds, (800.0, 600.0));
        // dx = 1.6, so pixel = 1.6/2 * 800 = 640
        assert!((pixel_len - 640.0).abs() < 1.0);
    }

    #[test]
    fn test_invalidate_cache_forces_regeneration() {
        let mut renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();

        // Fill cache
        let _ = renderer.generate_axis_vertices_cached(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        renderer.invalidate_cache();

        // Next call should be a cache miss
        let _ = renderer.generate_axis_vertices_cached(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
        );

        // 2 misses out of 3 lookups (miss, hit-on-generate, miss, hit-on-generate)
        // Actually: first call = 1 miss, generates & stores.
        // Second call after invalidate = 1 miss, generates & stores.
        // Lookups: miss(1) + hit(store-get), miss(2) + hit(store-get) = 4 total, 2 hits, 2 misses = 50%
        let lookups = renderer.geometry_cache().total_lookups();
        assert!(
            lookups >= 2,
            "Should have at least 2 lookups after invalidation"
        );
    }

    // ---- Tests for label culling ----

    #[test]
    fn test_generate_labels_culled_all_visible() {
        use crate::axis_performance::{LODLevel, ViewportBounds};

        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();
        let viewport = ViewportBounds::from_size(800.0, 600.0);

        let (labels, culled) = renderer.generate_labels_culled(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
            &viewport,
            LODLevel::High,
        );

        assert_eq!(labels.len(), 6, "All 6 labels should be visible");
        assert_eq!(culled, 0, "No labels should be culled");
    }

    #[test]
    fn test_generate_labels_culled_lod_limits() {
        use crate::axis_performance::{LODLevel, ViewportBounds};

        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();
        let viewport = ViewportBounds::from_size(800.0, 600.0);

        // Low LOD caps at 5 labels
        let (labels_low, _) = renderer.generate_labels_culled(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
            &viewport,
            LODLevel::Low,
        );
        assert!(
            labels_low.len() <= 5,
            "Low LOD should cap labels at 5, got {}",
            labels_low.len()
        );

        // Minimal LOD shows no labels
        let (labels_minimal, _) = renderer.generate_labels_culled(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
            &viewport,
            LODLevel::Minimal,
        );
        assert!(
            labels_minimal.is_empty(),
            "Minimal LOD should show no labels"
        );
    }

    #[test]
    fn test_generate_labels_culled_tiny_viewport() {
        use crate::axis_performance::{LODLevel, ViewportBounds};

        let renderer = AxisRenderer::new();
        // Axis spans full NDC range
        let bounds = AxisBounds::new(Vec2 { x: -1.0, y: -1.0 }, Vec2 { x: 1.0, y: -1.0 }, 50.0);
        let config = AxisConfiguration::default();
        // Tiny viewport that only covers a portion
        let viewport = ViewportBounds::new(350.0, 250.0, 450.0, 350.0);

        let (labels, culled) = renderer.generate_labels_culled(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
            None,
            &viewport,
            LODLevel::High,
        );

        assert!(
            culled > 0,
            "Some labels should be culled when viewport is tiny"
        );
        assert!(
            labels.len() < 6,
            "Fewer than 6 labels should be visible in tiny viewport"
        );
    }

    // ---- Tests for GPU-instanced tick rendering ----

    #[test]
    fn test_tick_instance_struct_size() {
        // position (2×f32) + tick_vector (2×f32) + color (4×f32) = 32 bytes
        assert_eq!(std::mem::size_of::<TickInstance>(), 32);
        assert_eq!(TickInstance::SIZE, 32);
    }

    #[test]
    fn test_tick_instance_new() {
        let ti = TickInstance::new([0.5, -0.5], [0.0, -0.02], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(ti.position, [0.5, -0.5]);
        assert_eq!(ti.tick_vector, [0.0, -0.02]);
        assert_eq!(ti.color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_tick_instance_bytemuck_round_trip() {
        let ti = TickInstance::new([0.1, 0.2], [0.3, 0.4], [0.5, 0.6, 0.7, 0.8]);
        let bytes: &[u8] = bytemuck::bytes_of(&ti);
        let recovered: &TickInstance = bytemuck::from_bytes(bytes);
        assert_eq!(*recovered, ti);
    }

    #[test]
    fn test_generate_tick_instances_count() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default(); // major ticks only

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // Default: 6 major ticks, no minor
        assert_eq!(instances.len(), 6);
    }

    #[test]
    fn test_tick_instances_match_vertex_pairs() {
        // The instanced data should represent the same line segments as vertex pairs.
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.5 }, Vec2 { x: 0.8, y: -0.5 }, 50.0);
        let config = AxisConfiguration::default().without_line();

        let viewport = (800.0, 600.0);
        let vertices =
            renderer.generate_tick_vertices(&bounds, &config, AxisPosition::Bottom, None, viewport);
        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            viewport,
        );

        // Each vertex pair maps to one instance
        assert_eq!(vertices.len(), instances.len() * 2);

        for (i, inst) in instances.iter().enumerate() {
            let v_start = &vertices[i * 2];
            let v_end = &vertices[i * 2 + 1];

            // Instance position matches the on-axis vertex
            assert!(
                (inst.position[0] - v_start.position[0]).abs() < 1e-6,
                "tick {i}: position x"
            );
            assert!(
                (inst.position[1] - v_start.position[1]).abs() < 1e-6,
                "tick {i}: position y"
            );

            // position + tick_vector should equal the tick-end vertex
            let end_x = inst.position[0] + inst.tick_vector[0];
            let end_y = inst.position[1] + inst.tick_vector[1];
            assert!((end_x - v_end.position[0]).abs() < 1e-6, "tick {i}: end x");
            assert!((end_y - v_end.position[1]).abs() < 1e-6, "tick {i}: end y");

            // Color matches
            assert_eq!(inst.color, v_start.color, "tick {i}: color");
        }
    }

    #[test]
    fn test_bottom_tick_instances_extend_downward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.5 }, Vec2 { x: 0.8, y: -0.5 }, 50.0);
        let config = AxisConfiguration::default();

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        for inst in &instances {
            assert_eq!(
                inst.tick_vector[0], 0.0,
                "Bottom ticks should not move in X"
            );
            assert!(
                inst.tick_vector[1] < 0.0,
                "Bottom ticks should extend downward (negative Y)"
            );
        }
    }

    #[test]
    fn test_top_tick_instances_extend_upward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: 0.5 }, Vec2 { x: 0.8, y: 0.5 }, 50.0);
        let config = AxisConfiguration::default();

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Top,
            None,
            (800.0, 600.0),
        );

        for inst in &instances {
            assert_eq!(inst.tick_vector[0], 0.0, "Top ticks should not move in X");
            assert!(
                inst.tick_vector[1] > 0.0,
                "Top ticks should extend upward (positive Y)"
            );
        }
    }

    #[test]
    fn test_left_tick_instances_extend_leftward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.5, y: -0.8 }, Vec2 { x: -0.5, y: 0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Left,
            None,
            (800.0, 600.0),
        );

        for inst in &instances {
            assert!(
                inst.tick_vector[0] < 0.0,
                "Left ticks should extend leftward (negative X)"
            );
            assert_eq!(inst.tick_vector[1], 0.0, "Left ticks should not move in Y");
        }
    }

    #[test]
    fn test_right_tick_instances_extend_rightward() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: 0.5, y: -0.8 }, Vec2 { x: 0.5, y: 0.8 }, 50.0);
        let config = AxisConfiguration::default();

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Right,
            None,
            (800.0, 600.0),
        );

        for inst in &instances {
            assert!(
                inst.tick_vector[0] > 0.0,
                "Right ticks should extend rightward (positive X)"
            );
            assert_eq!(inst.tick_vector[1], 0.0, "Right ticks should not move in Y");
        }
    }

    #[test]
    fn test_tick_instances_with_minor_ticks() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let mut config = AxisConfiguration::default();
        config.show_minor_ticks = true;

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        // Should have more instances than major-only
        assert!(instances.len() >= 6, "Should have at least 6 major ticks");
    }

    #[test]
    fn test_major_minor_separate_generation() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let mut config = AxisConfiguration::default();
        config.show_minor_ticks = true;

        let viewport = (800.0, 600.0);
        let major = renderer.generate_major_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            viewport,
        );
        let minor = renderer.generate_minor_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            viewport,
        );
        let combined = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            viewport,
        );

        assert_eq!(major.len() + minor.len(), combined.len());
    }

    #[test]
    fn test_tick_instances_empty_when_hidden() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default().without_ticks();

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        assert!(instances.is_empty());
    }

    #[test]
    fn test_tick_instance_color_matches_config() {
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let color = [1.0, 0.0, 0.0, 1.0];
        let config = AxisConfiguration::default().with_color(color);

        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            (800.0, 600.0),
        );

        for inst in &instances {
            assert_eq!(inst.color, color);
        }
    }

    #[test]
    fn test_tick_instance_vertex_count_reduction() {
        // This is the key performance metric: instances use less data than vertex pairs.
        let renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();
        let viewport = (800.0, 600.0);

        let vertices =
            renderer.generate_tick_vertices(&bounds, &config, AxisPosition::Bottom, None, viewport);
        let instances = renderer.generate_tick_instances(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            viewport,
        );

        let vertex_bytes = vertices.len() * std::mem::size_of::<Vertex>();
        let instance_bytes =
            instances.len() * std::mem::size_of::<TickInstance>() + 2 * std::mem::size_of::<f32>();

        assert!(
            instance_bytes < vertex_bytes,
            "Instance data ({instance_bytes} B) should be smaller than vertex data ({vertex_bytes} B)"
        );
    }

    #[test]
    fn test_tick_instance_buffer_layout() {
        let layout = TickInstance::instance_buffer_layout();
        assert_eq!(layout.array_stride, 32);
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Instance);
        assert_eq!(layout.attributes.len(), 3);
    }

    #[test]
    fn test_cached_tick_instances_returns_same_data() {
        let mut renderer = AxisRenderer::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();
        let viewport = (800.0, 600.0);

        // First call — cache miss
        let first = renderer
            .generate_tick_instances_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                viewport,
                None,
            )
            .to_vec();

        // Second call — cache hit (same inputs)
        let second = renderer
            .generate_tick_instances_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                viewport,
                None,
            )
            .to_vec();

        assert_eq!(first, second, "Cached data should match");
        assert!(
            renderer.geometry_cache().hit_rate() > 0.0,
            "Should have at least one cache hit"
        );
    }

    // ---- AxisLinePipeline tests (GPU required) ----

    #[tokio::test]
    async fn test_axis_line_pipeline_creation() {
        let context = crate::RenderContext::new().await.unwrap();
        let pipeline = AxisLinePipeline::new(context.device(), wgpu::TextureFormat::Bgra8Unorm);
        // Pipeline should be valid — just verify we can access it.
        let _ = pipeline.pipeline();
    }

    #[tokio::test]
    async fn test_axis_line_pipeline_upload_and_draw_zero() {
        let context = crate::RenderContext::new().await.unwrap();
        let pipeline = AxisLinePipeline::new(context.device(), wgpu::TextureFormat::Bgra8Unorm);
        let vertices: Vec<crate::render::Vertex> = vec![];
        let buf = pipeline.upload(context.device(), &vertices);
        // Zero-vertex draw should be a no-op (no panic).
        assert_eq!(buf.size(), 0);
    }
}
