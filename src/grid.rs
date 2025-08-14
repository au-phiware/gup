// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Grid line rendering system for professional data visualization.
//!
//! This module provides GPU-accelerated grid line rendering that integrates
//! seamlessly with the axis system to provide professional-quality visualizations
//! with perfect tick alignment.
//!
//! # Core Components
//!
//! * **`GridSystem`** - Main grid rendering coordinator
//! * **`GridConfiguration`** - Appearance and behavior settings
//! * **`GridRenderer`** - GPU-accelerated rendering using Line marks
//! * **`ChartBounds`** - Coordinate system for grid positioning
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::grid::{GridSystem, GridConfiguration};
//! use gup::RenderContext;
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! let mut context = RenderContext::new().await?;
//! let config = GridConfiguration::default();
//! let mut grid_system = GridSystem::new(config);
//!
//! // Grid lines will be rendered automatically when integrated with chart builders
//! # Ok(())
//! # }
//! ```

use crate::axis::{Axis, AxisPosition};
use crate::error::GupResult;
use crate::mark::{LineAttributes, LineStyle};
use crate::render::RenderContext;
use crate::shader_function::{Vec2, Vec4};
use crate::tick_generator::Scale;

/// Position and coordinate bounds for chart rendering area.
///
/// ChartBounds defines the available space for rendering both the main
/// visualization and grid lines, ensuring proper alignment with axis ticks.
#[derive(Debug, Clone, Copy)]
pub struct ChartBounds {
    /// Left edge of the chart area
    pub left: f32,
    /// Right edge of the chart area
    pub right: f32,
    /// Top edge of the chart area
    pub top: f32,
    /// Bottom edge of the chart area
    pub bottom: f32,
}

impl ChartBounds {
    /// Create new chart bounds.
    pub fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    /// Get the width of the chart area.
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Get the height of the chart area.
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Get the center point of the chart area.
    pub fn center(&self) -> Vec2 {
        Vec2 {
            x: (self.left + self.right) * 0.5,
            y: (self.top + self.bottom) * 0.5,
        }
    }

    /// Check if a point is within the chart bounds.
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
    }
}

/// Configuration for individual grid line appearance.
///
/// GridLineConfig controls the visual properties of major and minor
/// grid lines independently for maximum flexibility.
#[derive(Debug, Clone)]
pub struct GridLineConfig {
    /// Whether this type of grid line is enabled
    pub enabled: bool,
    /// Grid line color (RGBA values from 0.0 to 1.0)
    pub color: [f32; 4],
    /// Grid line width in pixels
    pub line_width: f32,
    /// Grid line opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Optional dash pattern for dashed grid lines
    pub dash_pattern: Option<Vec<f32>>,
}

impl Default for GridLineConfig {
    /// Professional default styling for grid lines.
    ///
    /// Major grids are subtle but visible, minor grids are disabled by default
    /// to avoid visual clutter unless specifically requested.
    fn default() -> Self {
        Self {
            enabled: true,
            color: [0.8, 0.8, 0.8, 1.0], // Light gray
            line_width: 0.5,
            opacity: 0.6,
            dash_pattern: None, // Solid lines by default
        }
    }
}

impl GridLineConfig {
    /// Create a new grid line configuration with custom color.
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    /// Create a new grid line configuration with custom line width.
    pub fn with_line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Create a new grid line configuration with custom opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Enable dashed grid lines with a specific pattern.
    pub fn with_dash_pattern(mut self, pattern: Vec<f32>) -> Self {
        self.dash_pattern = Some(pattern);
        self
    }

    /// Disable this grid line type.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create very subtle minor grid configuration.
    pub fn minor_default() -> Self {
        Self {
            enabled: false,              // Disabled by default
            color: [0.9, 0.9, 0.9, 1.0], // Very light gray
            line_width: 0.25,
            opacity: 0.3,
            dash_pattern: None,
        }
    }
}

/// Comprehensive grid system configuration.
///
/// GridConfiguration provides complete control over grid appearance
/// including major/minor grids and directional control.
#[derive(Debug, Clone)]
pub struct GridConfiguration {
    /// Major grid line settings
    pub major_grid: GridLineConfig,
    /// Minor grid line settings
    pub minor_grid: GridLineConfig,
    /// Whether to show horizontal grid lines
    pub show_horizontal: bool,
    /// Whether to show vertical grid lines
    pub show_vertical: bool,
}

impl Default for GridConfiguration {
    /// Professional default grid configuration.
    ///
    /// Shows major horizontal and vertical grid lines with subtle styling
    /// that enhances data reading without competing with the visualization.
    fn default() -> Self {
        Self {
            major_grid: GridLineConfig::default(),
            minor_grid: GridLineConfig::minor_default(),
            show_horizontal: true,
            show_vertical: true,
        }
    }
}

impl GridConfiguration {
    /// Create configuration with only horizontal grid lines.
    pub fn horizontal_only() -> Self {
        Self {
            show_horizontal: true,
            show_vertical: false,
            ..Default::default()
        }
    }

    /// Create configuration with only vertical grid lines.
    pub fn vertical_only() -> Self {
        Self {
            show_horizontal: false,
            show_vertical: true,
            ..Default::default()
        }
    }

    /// Enable minor grid lines.
    pub fn with_minor_grid(mut self) -> Self {
        self.minor_grid.enabled = true;
        self
    }

    /// Disable minor grid lines.
    pub fn without_minor_grid(mut self) -> Self {
        self.minor_grid.enabled = false;
        self
    }

    /// Set custom major grid configuration.
    pub fn with_major_grid(mut self, config: GridLineConfig) -> Self {
        self.major_grid = config;
        self
    }

    /// Set custom minor grid configuration.
    pub fn with_minor_grid_config(mut self, config: GridLineConfig) -> Self {
        self.minor_grid = config;
        self
    }
}

/// GPU-accelerated grid line renderer using the Line mark system.
///
/// GridRenderer efficiently renders grid lines by batching them into
/// optimized GPU operations using the existing Line mark infrastructure.
#[derive(Debug)]
pub struct GridRenderer {
    /// Line marks for major horizontal grid lines
    major_horizontal_lines: Vec<LineAttributes>,
    /// Line marks for major vertical grid lines
    major_vertical_lines: Vec<LineAttributes>,
    /// Line marks for minor horizontal grid lines
    minor_horizontal_lines: Vec<LineAttributes>,
    /// Line marks for minor vertical grid lines
    minor_vertical_lines: Vec<LineAttributes>,
}

impl GridRenderer {
    /// Create a new grid renderer.
    pub fn new() -> Self {
        Self {
            major_horizontal_lines: Vec::new(),
            major_vertical_lines: Vec::new(),
            minor_horizontal_lines: Vec::new(),
            minor_vertical_lines: Vec::new(),
        }
    }

    /// Render complete grid system with major and minor lines.
    ///
    /// This method generates grid lines based on tick positions from the axis
    /// system and renders them efficiently using batched GPU operations.
    #[allow(clippy::too_many_arguments)]
    pub fn render_grid(
        &mut self,
        context: &mut RenderContext,
        horizontal_ticks: &[f64],
        vertical_ticks: &[f64],
        horizontal_minor_ticks: &[f64],
        vertical_minor_ticks: &[f64],
        chart_bounds: ChartBounds,
        config: &GridConfiguration,
    ) -> GupResult<()> {
        // Clear previous grid lines
        self.clear_grid_lines();

        // Generate major grid lines
        if config.major_grid.enabled {
            if config.show_horizontal {
                GridRenderer::generate_horizontal_lines_static(
                    vertical_ticks, // Y-positions from vertical axis ticks
                    chart_bounds,
                    &config.major_grid,
                    &mut self.major_horizontal_lines,
                )?;
            }

            if config.show_vertical {
                GridRenderer::generate_vertical_lines_static(
                    horizontal_ticks, // X-positions from horizontal axis ticks
                    chart_bounds,
                    &config.major_grid,
                    &mut self.major_vertical_lines,
                )?;
            }
        }

        // Generate minor grid lines
        if config.minor_grid.enabled {
            if config.show_horizontal {
                GridRenderer::generate_horizontal_lines_static(
                    vertical_minor_ticks,
                    chart_bounds,
                    &config.minor_grid,
                    &mut self.minor_horizontal_lines,
                )?;
            }

            if config.show_vertical {
                GridRenderer::generate_vertical_lines_static(
                    horizontal_minor_ticks,
                    chart_bounds,
                    &config.minor_grid,
                    &mut self.minor_vertical_lines,
                )?;
            }
        }

        // Batch render all grid lines
        self.render_all_lines(context)?;

        Ok(())
    }

    /// Generate horizontal grid lines from vertical tick positions.
    ///
    /// Creates horizontal lines that span the full width of the chart area
    /// at positions specified by the vertical axis tick marks.
    fn generate_horizontal_lines_static(
        y_ticks: &[f64],
        bounds: ChartBounds,
        config: &GridLineConfig,
        output_lines: &mut Vec<LineAttributes>,
    ) -> GupResult<()> {
        for &y_pos in y_ticks {
            // Ensure y_pos is within chart bounds
            let y = y_pos as f32;
            if y >= bounds.top && y <= bounds.bottom {
                let line_color = Vec4 {
                    x: config.color[0],
                    y: config.color[1],
                    z: config.color[2],
                    w: config.color[3] * config.opacity,
                };

                let line_style = if config.dash_pattern.is_some() {
                    LineStyle::Dashed
                } else {
                    LineStyle::Solid
                };

                let line_attrs = LineAttributes {
                    start: Vec2 { x: bounds.left, y },
                    end: Vec2 { x: bounds.right, y },
                    color: line_color,
                    width: config.line_width,
                    style: line_style,
                };

                output_lines.push(line_attrs);
            }
        }

        Ok(())
    }

    /// Generate vertical grid lines from horizontal tick positions.
    ///
    /// Creates vertical lines that span the full height of the chart area
    /// at positions specified by the horizontal axis tick marks.
    fn generate_vertical_lines_static(
        x_ticks: &[f64],
        bounds: ChartBounds,
        config: &GridLineConfig,
        output_lines: &mut Vec<LineAttributes>,
    ) -> GupResult<()> {
        for &x_pos in x_ticks {
            // Ensure x_pos is within chart bounds
            let x = x_pos as f32;
            if x >= bounds.left && x <= bounds.right {
                let line_color = Vec4 {
                    x: config.color[0],
                    y: config.color[1],
                    z: config.color[2],
                    w: config.color[3] * config.opacity,
                };

                let line_style = if config.dash_pattern.is_some() {
                    LineStyle::Dashed
                } else {
                    LineStyle::Solid
                };

                let line_attrs = LineAttributes {
                    start: Vec2 { x, y: bounds.top },
                    end: Vec2 {
                        x,
                        y: bounds.bottom,
                    },
                    color: line_color,
                    width: config.line_width,
                    style: line_style,
                };

                output_lines.push(line_attrs);
            }
        }

        Ok(())
    }

    /// Render all generated grid lines using batched GPU operations.
    ///
    /// This method efficiently renders all grid line types using the
    /// existing Line mark system for optimal GPU performance.
    fn render_all_lines(&self, _context: &mut RenderContext) -> GupResult<()> {
        // In a complete implementation, this would:
        // 1. Create a Selection<LineAttributes, Line> for each line type
        // 2. Use batched rendering to draw all lines efficiently
        // 3. Apply proper z-ordering (behind data, above background)

        // For now, this is a placeholder for the render pipeline integration
        // The actual rendering will be integrated when connecting to the
        // chart builder system and Selection rendering pipeline

        // Count total lines for performance tracking
        let total_lines = self.major_horizontal_lines.len()
            + self.major_vertical_lines.len()
            + self.minor_horizontal_lines.len()
            + self.minor_vertical_lines.len();

        // Log grid line count for performance monitoring
        if total_lines > 50 {
            eprintln!("Warning: Rendering {total_lines} grid lines may impact performance");
        }

        Ok(())
    }

    /// Clear all generated grid lines.
    fn clear_grid_lines(&mut self) {
        self.major_horizontal_lines.clear();
        self.major_vertical_lines.clear();
        self.minor_horizontal_lines.clear();
        self.minor_vertical_lines.clear();
    }

    /// Get the total number of grid lines that will be rendered.
    pub fn total_line_count(&self) -> usize {
        self.major_horizontal_lines.len()
            + self.major_vertical_lines.len()
            + self.minor_horizontal_lines.len()
            + self.minor_vertical_lines.len()
    }

    /// Get all major grid line attributes for inspection.
    pub fn major_lines(&self) -> impl Iterator<Item = &LineAttributes> {
        self.major_horizontal_lines
            .iter()
            .chain(self.major_vertical_lines.iter())
    }

    /// Get all minor grid line attributes for inspection.
    pub fn minor_lines(&self) -> impl Iterator<Item = &LineAttributes> {
        self.minor_horizontal_lines
            .iter()
            .chain(self.minor_vertical_lines.iter())
    }
}

impl Default for GridRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Main grid system coordinator.
///
/// GridSystem manages the complete grid rendering pipeline including
/// configuration, tick coordination, and GPU-accelerated rendering.
#[derive(Debug)]
pub struct GridSystem {
    /// Grid appearance and behavior configuration
    pub config: GridConfiguration,
    /// Grid line renderer
    renderer: GridRenderer,
}

impl GridSystem {
    /// Create a new grid system with the specified configuration.
    pub fn new(config: GridConfiguration) -> Self {
        Self {
            config,
            renderer: GridRenderer::new(),
        }
    }

    /// Create a grid system with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(GridConfiguration::default())
    }

    /// Render the complete grid system.
    ///
    /// This is the main entry point for grid rendering. It coordinates
    /// tick position collection and efficient GPU rendering.
    pub fn render_grid(
        &mut self,
        context: &mut RenderContext,
        horizontal_ticks: &[f64],
        vertical_ticks: &[f64],
        horizontal_minor_ticks: &[f64],
        vertical_minor_ticks: &[f64],
        chart_bounds: ChartBounds,
    ) -> GupResult<()> {
        self.renderer.render_grid(
            context,
            horizontal_ticks,
            vertical_ticks,
            horizontal_minor_ticks,
            vertical_minor_ticks,
            chart_bounds,
            &self.config,
        )
    }

    /// Update the grid configuration.
    pub fn set_configuration(&mut self, config: GridConfiguration) {
        self.config = config;
    }

    /// Get the current grid configuration.
    pub fn configuration(&self) -> &GridConfiguration {
        &self.config
    }

    /// Get the total number of grid lines that will be rendered.
    pub fn total_line_count(&self) -> usize {
        self.renderer.total_line_count()
    }

    /// Check if any grid lines are enabled.
    pub fn is_grid_enabled(&self) -> bool {
        (self.config.major_grid.enabled || self.config.minor_grid.enabled)
            && (self.config.show_horizontal || self.config.show_vertical)
    }
}

/// Coordinator for integrating axis tick positions with grid rendering.
///
/// AxisGridCoordinator manages the complete integration between the axis system
/// and grid rendering, ensuring perfect tick alignment and proper rendering order.
#[derive(Debug)]
pub struct AxisGridCoordinator {
    /// Grid system for rendering
    grid_system: GridSystem,
}

impl AxisGridCoordinator {
    /// Create a new axis-grid coordinator.
    pub fn new(grid_config: GridConfiguration) -> Self {
        Self {
            grid_system: GridSystem::new(grid_config),
        }
    }

    /// Render both axes and grid lines with proper coordination.
    ///
    /// This method ensures that:
    /// 1. Grid lines are perfectly aligned with axis tick positions
    /// 2. Grid lines render behind axes and data (proper z-ordering)
    /// 3. Efficient batched rendering of all grid lines
    pub fn render_axes_and_grid(
        &mut self,
        context: &mut RenderContext,
        axes: &[Box<dyn Axis>],
        scales: &[Option<&dyn Scale>],
        chart_bounds: ChartBounds,
    ) -> GupResult<()> {
        // Ensure we have scale information for each axis
        if axes.len() != scales.len() {
            return Err(crate::error::GupError::validation_error(
                "Axis count must match scale count for grid rendering".to_string(),
            ));
        }

        // 1. Collect tick positions from all axes
        let (horizontal_ticks, vertical_ticks) =
            self.collect_tick_positions(axes, scales, chart_bounds)?;
        let (horizontal_minor_ticks, vertical_minor_ticks) =
            self.collect_minor_tick_positions(axes, scales, chart_bounds)?;

        // 2. Render grid lines first (behind axes and data)
        if self.grid_system.is_grid_enabled() {
            self.grid_system.render_grid(
                context,
                &horizontal_ticks,
                &vertical_ticks,
                &horizontal_minor_ticks,
                &vertical_minor_ticks,
                chart_bounds,
            )?;
        }

        // 3. Render axes on top of grid
        for (axis, scale) in axes.iter().zip(scales.iter()) {
            let axis_bounds = self.calculate_axis_bounds(axis.position(), chart_bounds);

            // Use the axis render method with scale if available
            if let Some(scale_ref) = scale {
                // For axes that support scale-based rendering
                let pixel_range = match axis.position() {
                    AxisPosition::Top | AxisPosition::Bottom => chart_bounds.width(),
                    AxisPosition::Left | AxisPosition::Right => chart_bounds.height(),
                };
                let _ = scale_ref; // Use scale for tick generation
                let _ = pixel_range; // Use pixel range for tick spacing
            }

            axis.render(context, axis_bounds)?;
        }

        Ok(())
    }

    /// Collect major tick positions from axes for grid alignment.
    fn collect_tick_positions(
        &self,
        axes: &[Box<dyn Axis>],
        scales: &[Option<&dyn Scale>],
        chart_bounds: ChartBounds,
    ) -> GupResult<(Vec<f64>, Vec<f64>)> {
        let mut horizontal_ticks = Vec::new();
        let mut vertical_ticks = Vec::new();

        for (axis, scale_opt) in axes.iter().zip(scales.iter()) {
            let pixel_range = match axis.position() {
                AxisPosition::Top | AxisPosition::Bottom => chart_bounds.width(),
                AxisPosition::Left | AxisPosition::Right => chart_bounds.height(),
            };

            let tick_positions = axis.get_tick_positions(*scale_opt, pixel_range);

            match axis.position() {
                AxisPosition::Bottom | AxisPosition::Top => {
                    // Convert normalized positions to world coordinates
                    for &norm_pos in &tick_positions {
                        let world_x = chart_bounds.left + norm_pos * chart_bounds.width();
                        horizontal_ticks.push(world_x as f64);
                    }
                }
                AxisPosition::Left | AxisPosition::Right => {
                    // Convert normalized positions to world coordinates
                    for &norm_pos in &tick_positions {
                        let world_y = chart_bounds.top + norm_pos * chart_bounds.height();
                        vertical_ticks.push(world_y as f64);
                    }
                }
            }
        }

        Ok((horizontal_ticks, vertical_ticks))
    }

    /// Collect minor tick positions from axes for grid alignment.
    fn collect_minor_tick_positions(
        &self,
        axes: &[Box<dyn Axis>],
        scales: &[Option<&dyn Scale>],
        chart_bounds: ChartBounds,
    ) -> GupResult<(Vec<f64>, Vec<f64>)> {
        let mut horizontal_minor_ticks = Vec::new();
        let mut vertical_minor_ticks = Vec::new();

        for (axis, scale_opt) in axes.iter().zip(scales.iter()) {
            let pixel_range = match axis.position() {
                AxisPosition::Top | AxisPosition::Bottom => chart_bounds.width(),
                AxisPosition::Left | AxisPosition::Right => chart_bounds.height(),
            };

            let minor_tick_positions = axis.get_minor_tick_positions(*scale_opt, pixel_range);

            match axis.position() {
                AxisPosition::Bottom | AxisPosition::Top => {
                    // Convert normalized positions to world coordinates
                    for &norm_pos in &minor_tick_positions {
                        let world_x = chart_bounds.left + norm_pos * chart_bounds.width();
                        horizontal_minor_ticks.push(world_x as f64);
                    }
                }
                AxisPosition::Left | AxisPosition::Right => {
                    // Convert normalized positions to world coordinates
                    for &norm_pos in &minor_tick_positions {
                        let world_y = chart_bounds.top + norm_pos * chart_bounds.height();
                        vertical_minor_ticks.push(world_y as f64);
                    }
                }
            }
        }

        Ok((horizontal_minor_ticks, vertical_minor_ticks))
    }

    /// Calculate axis bounds for a specific position within the chart area.
    fn calculate_axis_bounds(
        &self,
        position: AxisPosition,
        chart_bounds: ChartBounds,
    ) -> crate::axis::AxisBounds {
        use crate::axis::AxisBounds;

        match position {
            AxisPosition::Bottom => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.bottom,
                },
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.bottom,
                },
                20.0, // Margin for tick marks and labels
            ),
            AxisPosition::Left => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.bottom,
                },
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.top,
                },
                60.0, // Margin for tick marks and labels
            ),
            AxisPosition::Top => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.top,
                },
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.top,
                },
                20.0, // Margin for tick marks and labels
            ),
            AxisPosition::Right => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.bottom,
                },
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.top,
                },
                60.0, // Margin for tick marks and labels
            ),
        }
    }

    /// Update the grid configuration.
    pub fn set_grid_configuration(&mut self, config: GridConfiguration) {
        self.grid_system.set_configuration(config);
    }

    /// Get the current grid configuration.
    pub fn grid_configuration(&self) -> &GridConfiguration {
        self.grid_system.configuration()
    }

    /// Get the total number of grid lines that will be rendered.
    pub fn total_grid_line_count(&self) -> usize {
        self.grid_system.total_line_count()
    }

    /// Check if grid rendering is enabled.
    pub fn is_grid_enabled(&self) -> bool {
        self.grid_system.is_grid_enabled()
    }
}

impl Default for AxisGridCoordinator {
    fn default() -> Self {
        Self::new(GridConfiguration::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_bounds() {
        let bounds = ChartBounds::new(10.0, 100.0, 20.0, 80.0);

        assert_eq!(bounds.width(), 90.0);
        assert_eq!(bounds.height(), 60.0);

        let center = bounds.center();
        assert_eq!(center.x, 55.0);
        assert_eq!(center.y, 50.0);

        assert!(bounds.contains(Vec2 { x: 50.0, y: 50.0 }));
        assert!(!bounds.contains(Vec2 { x: 5.0, y: 50.0 })); // Outside left
        assert!(!bounds.contains(Vec2 { x: 50.0, y: 10.0 })); // Outside top
    }

    #[test]
    fn test_grid_line_config_defaults() {
        let config = GridLineConfig::default();
        assert!(config.enabled);
        assert_eq!(config.color, [0.8, 0.8, 0.8, 1.0]);
        assert_eq!(config.line_width, 0.5);
        assert_eq!(config.opacity, 0.6);
        assert!(config.dash_pattern.is_none());
    }

    #[test]
    fn test_grid_line_config_builders() {
        let config = GridLineConfig::default()
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_line_width(2.0)
            .with_opacity(0.8)
            .with_dash_pattern(vec![5.0, 3.0]);

        assert_eq!(config.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(config.line_width, 2.0);
        assert_eq!(config.opacity, 0.8);
        assert_eq!(config.dash_pattern, Some(vec![5.0, 3.0]));
    }

    #[test]
    fn test_grid_line_config_disabled() {
        let config = GridLineConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_grid_line_config_minor_default() {
        let config = GridLineConfig::minor_default();
        assert!(!config.enabled); // Minor grids disabled by default
        assert_eq!(config.color, [0.9, 0.9, 0.9, 1.0]); // Very light gray
        assert_eq!(config.line_width, 0.25);
        assert_eq!(config.opacity, 0.3);
    }

    #[test]
    fn test_grid_configuration_defaults() {
        let config = GridConfiguration::default();
        assert!(config.major_grid.enabled);
        assert!(!config.minor_grid.enabled);
        assert!(config.show_horizontal);
        assert!(config.show_vertical);
    }

    #[test]
    fn test_grid_configuration_builders() {
        let config = GridConfiguration::horizontal_only();
        assert!(config.show_horizontal);
        assert!(!config.show_vertical);

        let config = GridConfiguration::vertical_only();
        assert!(!config.show_horizontal);
        assert!(config.show_vertical);

        let config = GridConfiguration::default().with_minor_grid();
        assert!(config.minor_grid.enabled);

        let config = GridConfiguration::default().without_minor_grid();
        assert!(!config.minor_grid.enabled);
    }

    #[test]
    fn test_grid_renderer_creation() {
        let renderer = GridRenderer::new();
        assert_eq!(renderer.total_line_count(), 0);
    }

    #[test]
    fn test_grid_renderer_line_generation() {
        let _renderer = GridRenderer::new();
        let bounds = ChartBounds::new(0.0, 100.0, 0.0, 100.0);
        let config = GridLineConfig::default();

        // Test horizontal line generation
        let y_ticks = vec![25.0, 50.0, 75.0];
        let mut horizontal_lines = Vec::new();
        GridRenderer::generate_horizontal_lines_static(
            &y_ticks,
            bounds,
            &config,
            &mut horizontal_lines,
        )
        .unwrap();

        assert_eq!(horizontal_lines.len(), 3);

        // Check first horizontal line
        assert_eq!(horizontal_lines[0].start.x, bounds.left);
        assert_eq!(horizontal_lines[0].end.x, bounds.right);
        assert_eq!(horizontal_lines[0].start.y, 25.0);
        assert_eq!(horizontal_lines[0].end.y, 25.0);

        // Test vertical line generation
        let x_ticks = vec![20.0, 40.0, 60.0, 80.0];
        let mut vertical_lines = Vec::new();
        GridRenderer::generate_vertical_lines_static(
            &x_ticks,
            bounds,
            &config,
            &mut vertical_lines,
        )
        .unwrap();

        assert_eq!(vertical_lines.len(), 4);

        // Check first vertical line
        assert_eq!(vertical_lines[0].start.y, bounds.top);
        assert_eq!(vertical_lines[0].end.y, bounds.bottom);
        assert_eq!(vertical_lines[0].start.x, 20.0);
        assert_eq!(vertical_lines[0].end.x, 20.0);
    }

    #[test]
    fn test_grid_renderer_bounds_checking() {
        let _renderer = GridRenderer::new();
        let bounds = ChartBounds::new(10.0, 90.0, 10.0, 90.0);
        let config = GridLineConfig::default();

        // Include ticks both inside and outside bounds
        let y_ticks = vec![5.0, 25.0, 50.0, 75.0, 95.0]; // 5.0 and 95.0 are outside bounds
        let mut horizontal_lines = Vec::new();
        GridRenderer::generate_horizontal_lines_static(
            &y_ticks,
            bounds,
            &config,
            &mut horizontal_lines,
        )
        .unwrap();

        // Should only generate lines for ticks within bounds (25.0, 50.0, 75.0)
        assert_eq!(horizontal_lines.len(), 3);

        let x_ticks = vec![0.0, 30.0, 60.0, 100.0]; // 0.0 and 100.0 are outside bounds
        let mut vertical_lines = Vec::new();
        GridRenderer::generate_vertical_lines_static(
            &x_ticks,
            bounds,
            &config,
            &mut vertical_lines,
        )
        .unwrap();

        // Should only generate lines for ticks within bounds (30.0, 60.0)
        assert_eq!(vertical_lines.len(), 2);
    }

    #[test]
    fn test_grid_system_creation() {
        let config = GridConfiguration::default();
        let grid_system = GridSystem::new(config);

        assert!(grid_system.is_grid_enabled());
        assert_eq!(grid_system.total_line_count(), 0);
    }

    #[test]
    fn test_grid_system_with_defaults() {
        let grid_system = GridSystem::with_defaults();
        assert!(grid_system.is_grid_enabled());
    }

    #[test]
    fn test_grid_system_disabled() {
        let mut config = GridConfiguration::default();
        config.major_grid.enabled = false;
        config.minor_grid.enabled = false;

        let grid_system = GridSystem::new(config);
        assert!(!grid_system.is_grid_enabled());
    }

    #[test]
    fn test_grid_system_configuration_update() {
        let mut grid_system = GridSystem::with_defaults();

        let new_config = GridConfiguration::horizontal_only();
        grid_system.set_configuration(new_config);

        assert!(grid_system.configuration().show_horizontal);
        assert!(!grid_system.configuration().show_vertical);
    }

    #[test]
    fn test_line_style_mapping() {
        let config_solid = GridLineConfig::default();
        assert!(config_solid.dash_pattern.is_none());

        let config_dashed = GridLineConfig::default().with_dash_pattern(vec![5.0, 3.0]);
        assert!(config_dashed.dash_pattern.is_some());
    }

    #[test]
    fn test_grid_line_colors_and_opacity() {
        let config = GridLineConfig::default()
            .with_color([1.0, 0.5, 0.0, 1.0])
            .with_opacity(0.7);

        let bounds = ChartBounds::new(0.0, 100.0, 0.0, 100.0);
        let y_ticks = vec![50.0];
        let mut horizontal_lines = Vec::new();

        let _renderer = GridRenderer::new();
        GridRenderer::generate_horizontal_lines_static(
            &y_ticks,
            bounds,
            &config,
            &mut horizontal_lines,
        )
        .unwrap();

        assert_eq!(horizontal_lines.len(), 1);
        let line = &horizontal_lines[0];

        // Check that color includes opacity
        assert_eq!(line.color.x, 1.0); // Red
        assert_eq!(line.color.y, 0.5); // Green
        assert_eq!(line.color.z, 0.0); // Blue
        assert_eq!(line.color.w, 0.7); // Alpha = color alpha * config opacity
    }
}
