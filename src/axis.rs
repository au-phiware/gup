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

use crate::error::GupResult;
use crate::render::RenderContext;
use crate::shader_function::Vec2;
use crate::tick_generator::{LinearTickGenerator, Scale, TickGenerator};

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
}

/// Core trait for axis implementations.
///
/// The Axis trait defines the interface that all axis types must implement.
/// It provides methods for rendering, layout calculation, and tick positioning
/// that integrate with the GPU-accelerated rendering system.
pub trait Axis: Send + Sync + std::fmt::Debug + 'static {
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

/// GPU-accelerated axis renderer using the Line mark system.
///
/// AxisRenderer provides efficient rendering of axis components by
/// leveraging the existing Line mark implementation for GPU acceleration.
pub struct AxisRenderer {
    // Future: cache render pipelines and resources here
}

impl AxisRenderer {
    /// Create a new axis renderer.
    pub fn new() -> Self {
        Self {}
    }

    /// Render the main axis line using GPU-accelerated Line marks.
    pub fn render_axis_line(
        &self,
        _context: &mut RenderContext,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
    ) -> GupResult<()> {
        // Create line attributes for the axis line
        use crate::mark::{LineAttributes, LineStyle};
        use crate::shader_function::Vec4;

        let _line_attrs = LineAttributes {
            start: bounds.start,
            end: bounds.end,
            color: Vec4 {
                x: config.line_color[0],
                y: config.line_color[1],
                z: config.line_color[2],
                w: config.line_color[3],
            },
            width: config.line_width,
            style: LineStyle::Solid,
        };

        // For now, we'll prepare the line data structure
        // Future integration will use the Selection system to render
        // the line using the existing GPU pipeline

        // This is a placeholder - actual rendering will be integrated
        // when connecting to the chart builder system
        Ok(())
    }

    /// Render tick marks at specified positions.
    pub fn render_ticks(
        &self,
        _context: &mut RenderContext,
        bounds: &AxisBounds,
        tick_positions: &[Vec2],
        tick_length: f32,
        config: &AxisConfiguration,
    ) -> GupResult<()> {
        use crate::mark::{LineAttributes, LineStyle};
        use crate::shader_function::Vec4;

        let normal = bounds.normal();
        let tick_color = Vec4 {
            x: config.line_color[0],
            y: config.line_color[1],
            z: config.line_color[2],
            w: config.line_color[3],
        };

        // Generate tick line attributes
        let _tick_lines: Vec<LineAttributes> = tick_positions
            .iter()
            .map(|&pos| {
                let tick_start = Vec2 {
                    x: pos.x - normal.x * tick_length * 0.5,
                    y: pos.y - normal.y * tick_length * 0.5,
                };
                let tick_end = Vec2 {
                    x: pos.x + normal.x * tick_length * 0.5,
                    y: pos.y + normal.y * tick_length * 0.5,
                };

                LineAttributes {
                    start: tick_start,
                    end: tick_end,
                    color: tick_color,
                    width: config.line_width,
                    style: LineStyle::Solid,
                }
            })
            .collect();

        // For now, this is a placeholder
        // Future integration will batch render all tick lines efficiently
        Ok(())
    }
}

impl Default for AxisRenderer {
    fn default() -> Self {
        Self::new()
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
}
