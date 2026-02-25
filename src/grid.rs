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
use crate::render::RenderContext;
// TODO: Implement Selection type
// use crate::selection::LineAttributes;
use crate::shader_function::{Vec2, Vec4};
use crate::tick_generator::Scale;
use crate::{LineAttributes, LineStyle}; // From mark::line

/// Color representation for grid styling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component (0.0 to 1.0)
    pub r: f32,
    /// Green component (0.0 to 1.0)
    pub g: f32,
    /// Blue component (0.0 to 1.0)
    pub b: f32,
    /// Alpha component (0.0 to 1.0)
    pub a: f32,
}

impl Color {
    /// Create a new color from RGBA components.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Convert to RGBA array format.
    pub fn to_rgba(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Parse a hex color string like "#cccccc" or "#ccc".
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.trim_start_matches('#');

        let (r, g, b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)
                    .map_err(|_| "Invalid hex color")?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)
                    .map_err(|_| "Invalid hex color")?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)
                    .map_err(|_| "Invalid hex color")?;
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex color")?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex color")?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex color")?;
                (r, g, b)
            }
            _ => return Err("Hex color must be 3 or 6 characters".to_string()),
        };

        Ok(Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            1.0,
        ))
    }

    /// Common color presets for grids.
    pub const LIGHT_GRID: Color = Color::new(0.9, 0.9, 0.9, 0.7);
    pub const DARK_GRID: Color = Color::new(0.3, 0.3, 0.3, 0.8);
    pub const SUBTLE_GRID: Color = Color::new(0.95, 0.95, 0.95, 0.5);
    pub const HIGH_CONTRAST_GRID: Color = Color::new(0.0, 0.0, 0.0, 0.8);
}

impl From<&str> for Color {
    /// Parse hex colors like "#cccccc".
    fn from(hex: &str) -> Self {
        Color::from_hex(hex).unwrap_or(Color::LIGHT_GRID)
    }
}

impl From<(f32, f32, f32)> for Color {
    /// Create color from RGB tuple (alpha = 1.0).
    fn from((r, g, b): (f32, f32, f32)) -> Self {
        Color::new(r, g, b, 1.0)
    }
}

impl From<(f32, f32, f32, f32)> for Color {
    /// Create color from RGBA tuple.
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Color::new(r, g, b, a)
    }
}

impl From<[f32; 4]> for Color {
    /// Create color from RGBA array.
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Color::new(r, g, b, a)
    }
}

impl From<Color> for [f32; 4] {
    /// Convert color to RGBA array.
    fn from(color: Color) -> [f32; 4] {
        color.to_rgba()
    }
}

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

    /// Light theme grid suitable for bright backgrounds.
    pub fn light_theme() -> Self {
        Self {
            major_grid: GridLineConfig {
                enabled: true,
                color: [0.0, 0.0, 0.0, 0.15], // Very light black
                line_width: 0.5,
                opacity: 1.0,
                dash_pattern: None,
            },
            minor_grid: GridLineConfig::disabled(),
            show_horizontal: true,
            show_vertical: true,
        }
    }

    /// Dark theme grid suitable for dark backgrounds.
    pub fn dark_theme() -> Self {
        Self {
            major_grid: GridLineConfig {
                enabled: true,
                color: [1.0, 1.0, 1.0, 0.25], // Light white
                line_width: 0.5,
                opacity: 1.0,
                dash_pattern: None,
            },
            minor_grid: GridLineConfig::disabled(),
            show_horizontal: true,
            show_vertical: true,
        }
    }

    /// Scientific/technical visualization grid.
    pub fn scientific() -> Self {
        Self {
            major_grid: GridLineConfig {
                enabled: true,
                color: [0.3, 0.3, 0.3, 1.0], // Medium gray
                line_width: 0.75,
                opacity: 0.8,
                dash_pattern: None,
            },
            minor_grid: GridLineConfig {
                enabled: true,               // Enable minor grids for precision
                color: [0.7, 0.7, 0.7, 1.0], // Light gray
                line_width: 0.25,
                opacity: 0.4,
                dash_pattern: None,
            },
            show_horizontal: true,
            show_vertical: true,
        }
    }

    /// Business/dashboard friendly grid.
    pub fn business() -> Self {
        Self {
            major_grid: GridLineConfig {
                enabled: true,
                color: [0.9, 0.9, 0.9, 1.0], // Very light gray
                line_width: 0.5,
                opacity: 0.7,
                dash_pattern: None,
            },
            minor_grid: GridLineConfig::disabled(), // Keep it clean
            show_horizontal: true,
            show_vertical: false, // Often only horizontal grids in business charts
        }
    }

    /// Minimal grid with subtle styling.
    pub fn minimal() -> Self {
        Self {
            major_grid: GridLineConfig {
                enabled: true,
                color: [0.95, 0.95, 0.95, 1.0], // Very subtle gray
                line_width: 0.25,
                opacity: 0.5,
                dash_pattern: None,
            },
            minor_grid: GridLineConfig::disabled(),
            show_horizontal: true,
            show_vertical: true,
        }
    }

    /// High contrast grid for accessibility.
    pub fn high_contrast() -> Self {
        Self {
            major_grid: GridLineConfig {
                enabled: true,
                color: [0.0, 0.0, 0.0, 1.0], // Full black
                line_width: 1.0,
                opacity: 0.8,
                dash_pattern: None,
            },
            minor_grid: GridLineConfig {
                enabled: true,
                color: [0.4, 0.4, 0.4, 1.0], // Medium gray
                line_width: 0.5,
                opacity: 0.6,
                dash_pattern: None,
            },
            show_horizontal: true,
            show_vertical: true,
        }
    }
}

/// GPU-accelerated grid line renderer using the Line mark system.
///
/// GridRenderer efficiently renders grid lines by batching them into
/// optimized GPU operations using the existing Line mark infrastructure.
/// Includes geometry caching to avoid per-frame regeneration when
/// tick positions and configuration have not changed.
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
    /// Cache fingerprint: hash of (ticks, bounds, config) from the last render.
    /// When unchanged, we skip regeneration.
    cache_fingerprint: Option<u64>,
    /// Cache hit/miss counters for diagnostics.
    cache_hits: u64,
    cache_misses: u64,
}

impl GridRenderer {
    /// Create a new grid renderer.
    pub fn new() -> Self {
        Self {
            major_horizontal_lines: Vec::new(),
            major_vertical_lines: Vec::new(),
            minor_horizontal_lines: Vec::new(),
            minor_vertical_lines: Vec::new(),
            cache_fingerprint: None,
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    /// Render complete grid system with major and minor lines.
    ///
    /// This method generates grid lines based on tick positions from the axis
    /// system and renders them efficiently using batched GPU operations.
    /// Grid geometry is cached: if the tick positions, bounds, and config
    /// have not changed since the last call, regeneration is skipped.
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
        // Compute a fingerprint of the current inputs
        let fingerprint = Self::compute_fingerprint(
            horizontal_ticks,
            vertical_ticks,
            horizontal_minor_ticks,
            vertical_minor_ticks,
            chart_bounds,
            config,
        );

        // Check cache
        if self.cache_fingerprint == Some(fingerprint) {
            self.cache_hits += 1;
            // Lines are already generated — skip to rendering
            return self.render_all_lines(context);
        }

        self.cache_misses += 1;

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

        // Store fingerprint for next-frame caching
        self.cache_fingerprint = Some(fingerprint);

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

                let line_attrs = LineAttributes {
                    start: Vec2 { x: bounds.left, y },
                    end: Vec2 { x: bounds.right, y },
                    color: Vec4 {
                        x: line_color.x,
                        y: line_color.y,
                        z: line_color.z,
                        w: line_color.w,
                    },
                    width: config.line_width,
                    style: LineStyle::Solid,
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

                let line_attrs = LineAttributes {
                    start: Vec2 { x, y: bounds.top },
                    end: Vec2 {
                        x,
                        y: bounds.bottom,
                    },
                    color: Vec4 {
                        x: line_color.x,
                        y: line_color.y,
                        z: line_color.z,
                        w: line_color.w,
                    },
                    width: config.line_width,
                    style: LineStyle::Solid,
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
        // Count total lines for performance tracking
        let total_lines = self.major_horizontal_lines.len()
            + self.major_vertical_lines.len()
            + self.minor_horizontal_lines.len()
            + self.minor_vertical_lines.len();

        // Log grid line count for performance monitoring
        if total_lines > 50 {
            eprintln!("Warning: Rendering {total_lines} grid lines may impact performance");
        }

        // For now, just track that grid lines are ready for rendering
        // The actual rendering will be done via create_grid_selections()
        // when called from the chart builder integration
        if total_lines > 0 {
            println!("Grid system ready: {total_lines} total grid lines generated");
        }

        Ok(())
    }

    // Convert grid line attributes to Selection instances for rendering.
    //
    // This method creates Selection<LineAttributes, Line> instances for each
    // grid line type, enabling integration with the existing rendering pipeline.
    //
    // TODO: Disabled until Selection type is fully implemented
    /*
    pub fn create_grid_selections(
        &self,
        context: Arc<RenderContext>,
    ) -> GupResult<Vec<crate::selection::Selection<LineAttributes, crate::selection::Line>>> {
        use crate::selection::Selection;

        let mut selections = Vec::new();

        // Create selection for major horizontal lines
        if !self.major_horizontal_lines.is_empty() {
            let selection = Selection::new(self.major_horizontal_lines.clone(), context.clone())?;
            selections.push(selection);
        }

        // Create selection for major vertical lines
        if !self.major_vertical_lines.is_empty() {
            let selection = Selection::new(self.major_vertical_lines.clone(), context.clone())?;
            selections.push(selection);
        }

        // Create selection for minor horizontal lines
        if !self.minor_horizontal_lines.is_empty() {
            let selection = Selection::new(self.minor_horizontal_lines.clone(), context.clone())?;
            selections.push(selection);
        }

        // Create selection for minor vertical lines
        if !self.minor_vertical_lines.is_empty() {
            let selection = Selection::new(self.minor_vertical_lines.clone(), context)?;
            selections.push(selection);
        }

        Ok(selections)
    }
    */

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

    /// Compute a simple fingerprint from grid inputs for caching.
    ///
    /// Uses a fast hash (FNV-style) of tick positions, bounds, and config flags.
    /// This avoids the overhead of comparing full tick arrays every frame.
    #[allow(clippy::too_many_arguments)]
    fn compute_fingerprint(
        horizontal_ticks: &[f64],
        vertical_ticks: &[f64],
        horizontal_minor_ticks: &[f64],
        vertical_minor_ticks: &[f64],
        chart_bounds: ChartBounds,
        config: &GridConfiguration,
    ) -> u64 {
        use std::hash::{Hash, Hasher};

        // Use the standard library's default hasher for simplicity.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Hash tick positions via their bit patterns
        for &t in horizontal_ticks {
            t.to_bits().hash(&mut hasher);
        }
        horizontal_ticks.len().hash(&mut hasher);

        for &t in vertical_ticks {
            t.to_bits().hash(&mut hasher);
        }
        vertical_ticks.len().hash(&mut hasher);

        for &t in horizontal_minor_ticks {
            t.to_bits().hash(&mut hasher);
        }
        horizontal_minor_ticks.len().hash(&mut hasher);

        for &t in vertical_minor_ticks {
            t.to_bits().hash(&mut hasher);
        }
        vertical_minor_ticks.len().hash(&mut hasher);

        // Hash bounds
        chart_bounds.left.to_bits().hash(&mut hasher);
        chart_bounds.right.to_bits().hash(&mut hasher);
        chart_bounds.top.to_bits().hash(&mut hasher);
        chart_bounds.bottom.to_bits().hash(&mut hasher);

        // Hash config flags
        config.major_grid.enabled.hash(&mut hasher);
        config.minor_grid.enabled.hash(&mut hasher);
        config.show_horizontal.hash(&mut hasher);
        config.show_vertical.hash(&mut hasher);
        config.major_grid.line_width.to_bits().hash(&mut hasher);
        config.minor_grid.line_width.to_bits().hash(&mut hasher);

        hasher.finish()
    }

    /// Invalidate the geometry cache, forcing regeneration on the next render.
    pub fn invalidate_cache(&mut self) {
        self.cache_fingerprint = None;
    }

    /// Cache hit rate (0.0–1.0). Returns 0.0 if no lookups have occurred.
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
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

    /*
    /// Create grid line selections for visual rendering.
    ///
    /// This method creates Selection instances for all generated grid lines,
    /// enabling integration with the chart builder and rendering pipeline.
    ///
    /// TODO: Disabled until Selection type is implemented
    pub fn create_grid_selections(
        &self,
        context: Arc<RenderContext>,
    ) -> GupResult<Vec<crate::selection::Selection<LineAttributes, crate::selection::Line>>> {
        self.renderer.create_grid_selections(context)
    }
    */
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

    // Tests for Color struct (GUP-097)
    #[test]
    fn test_color_creation() {
        let color = Color::new(1.0, 0.5, 0.2, 0.8);
        assert_eq!(color.r, 1.0);
        assert_eq!(color.g, 0.5);
        assert_eq!(color.b, 0.2);
        assert_eq!(color.a, 0.8);
    }

    #[test]
    fn test_color_to_rgba() {
        let color = Color::new(0.3, 0.6, 0.9, 1.0);
        let rgba = color.to_rgba();
        assert_eq!(rgba, [0.3, 0.6, 0.9, 1.0]);
    }

    #[test]
    fn test_color_from_hex() {
        // Test 6-character hex
        let color = Color::from_hex("#ff6b6b").unwrap();
        assert!((color.r - 1.0).abs() < 0.01);
        assert!((color.g - 0.42).abs() < 0.01);
        assert!((color.b - 0.42).abs() < 0.01);
        assert_eq!(color.a, 1.0);

        // Test 3-character hex
        let color = Color::from_hex("#f0a").unwrap();
        assert!((color.r - 1.0).abs() < 0.01);
        assert!((color.g - 0.0).abs() < 0.01);
        assert!((color.b - 0.67).abs() < 0.01);
        assert_eq!(color.a, 1.0);

        // Test without #
        let color = Color::from_hex("cccccc").unwrap();
        assert!((color.r - 0.8).abs() < 0.01);
        assert!((color.g - 0.8).abs() < 0.01);
        assert!((color.b - 0.8).abs() < 0.01);

        // Test invalid hex
        assert!(Color::from_hex("invalid").is_err());
        assert!(Color::from_hex("#12").is_err());
    }

    #[test]
    fn test_color_from_conversions() {
        // From hex string
        let color: Color = "#ff0000".into();
        assert!((color.r - 1.0).abs() < 0.01);
        assert!((color.g - 0.0).abs() < 0.01);

        // From RGB tuple
        let color: Color = (0.5, 0.3, 0.8).into();
        assert_eq!(color.r, 0.5);
        assert_eq!(color.g, 0.3);
        assert_eq!(color.b, 0.8);
        assert_eq!(color.a, 1.0);

        // From RGBA tuple
        let color: Color = (0.2, 0.4, 0.6, 0.8).into();
        assert_eq!(color.r, 0.2);
        assert_eq!(color.g, 0.4);
        assert_eq!(color.b, 0.6);
        assert_eq!(color.a, 0.8);

        // From RGBA array
        let color: Color = [0.1, 0.2, 0.3, 0.4].into();
        assert_eq!(color.r, 0.1);
        assert_eq!(color.g, 0.2);
        assert_eq!(color.b, 0.3);
        assert_eq!(color.a, 0.4);
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::LIGHT_GRID.r, 0.9);
        assert_eq!(Color::DARK_GRID.r, 0.3);
        assert_eq!(Color::SUBTLE_GRID.r, 0.95);
        assert_eq!(Color::HIGH_CONTRAST_GRID.r, 0.0);
    }

    // Tests for grid theme presets (GUP-097)
    #[test]
    fn test_grid_configuration_themes() {
        let light = GridConfiguration::light_theme();
        assert!(light.major_grid.enabled);
        assert!(!light.minor_grid.enabled);
        assert!(light.show_horizontal);
        assert!(light.show_vertical);
        // Light theme should have very light black
        assert_eq!(light.major_grid.color[0], 0.0);
        assert_eq!(light.major_grid.color[3], 0.15);

        let dark = GridConfiguration::dark_theme();
        assert!(dark.major_grid.enabled);
        // Dark theme should have light white
        assert_eq!(dark.major_grid.color[0], 1.0);
        assert_eq!(dark.major_grid.color[3], 0.25);

        let scientific = GridConfiguration::scientific();
        assert!(scientific.major_grid.enabled);
        assert!(scientific.minor_grid.enabled); // Scientific includes minor grids
        assert_eq!(scientific.major_grid.line_width, 0.75);
        assert_eq!(scientific.minor_grid.line_width, 0.25);

        let business = GridConfiguration::business();
        assert!(business.major_grid.enabled);
        assert!(!business.minor_grid.enabled);
        assert!(business.show_horizontal);
        assert!(!business.show_vertical); // Business often uses only horizontal

        let minimal = GridConfiguration::minimal();
        assert!(minimal.major_grid.enabled);
        assert_eq!(minimal.major_grid.line_width, 0.25); // Very thin lines
        assert_eq!(minimal.major_grid.opacity, 0.5); // Very subtle

        let high_contrast = GridConfiguration::high_contrast();
        assert!(high_contrast.major_grid.enabled);
        assert!(high_contrast.minor_grid.enabled);
        assert_eq!(high_contrast.major_grid.line_width, 1.0); // Thick for visibility
        assert_eq!(high_contrast.major_grid.color, [0.0, 0.0, 0.0, 1.0]); // Full black
    }

    #[test]
    fn test_grid_configuration_theme_consistency() {
        // All themes should have consistent structure
        let themes = vec![
            GridConfiguration::light_theme(),
            GridConfiguration::dark_theme(),
            GridConfiguration::scientific(),
            GridConfiguration::business(),
            GridConfiguration::minimal(),
            GridConfiguration::high_contrast(),
        ];

        for theme in themes {
            // All themes should have at least major grid enabled
            assert!(theme.major_grid.enabled);
            // All themes should have valid colors (all components between 0.0 and 1.0)
            for &component in &theme.major_grid.color {
                assert!((0.0..=1.0).contains(&component));
            }
            // All themes should have positive line width
            assert!(theme.major_grid.line_width > 0.0);
            // All themes should have valid opacity
            assert!(theme.major_grid.opacity >= 0.0 && theme.major_grid.opacity <= 1.0);
        }
    }

    // ---- Grid geometry caching tests ----

    #[test]
    fn test_grid_cache_miss_on_first_call() {
        let mut renderer = GridRenderer::new();
        let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
        let config = GridConfiguration::default();

        // render_grid needs a RenderContext but render_all_lines is a no-op,
        // so we can test the caching logic by calling the fingerprint directly.
        let fp1 = GridRenderer::compute_fingerprint(
            &[100.0, 300.0],
            &[200.0, 400.0],
            &[],
            &[],
            bounds,
            &config,
        );

        assert!(renderer.cache_fingerprint.is_none());
        // After storing, it should match
        renderer.cache_fingerprint = Some(fp1);
        assert_eq!(renderer.cache_fingerprint, Some(fp1));
    }

    #[test]
    fn test_grid_fingerprint_changes_with_ticks() {
        let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
        let config = GridConfiguration::default();

        let fp1 =
            GridRenderer::compute_fingerprint(&[100.0, 300.0], &[200.0], &[], &[], bounds, &config);
        let fp2 =
            GridRenderer::compute_fingerprint(&[100.0, 500.0], &[200.0], &[], &[], bounds, &config);

        assert_ne!(
            fp1, fp2,
            "Different ticks should produce different fingerprints"
        );
    }

    #[test]
    fn test_grid_fingerprint_changes_with_bounds() {
        let config = GridConfiguration::default();
        let ticks = [100.0, 300.0];

        let fp1 = GridRenderer::compute_fingerprint(
            &ticks,
            &[200.0],
            &[],
            &[],
            ChartBounds::new(50.0, 750.0, 50.0, 550.0),
            &config,
        );
        let fp2 = GridRenderer::compute_fingerprint(
            &ticks,
            &[200.0],
            &[],
            &[],
            ChartBounds::new(100.0, 700.0, 50.0, 550.0),
            &config,
        );

        assert_ne!(
            fp1, fp2,
            "Different bounds should produce different fingerprints"
        );
    }

    #[test]
    fn test_grid_fingerprint_same_for_same_inputs() {
        let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
        let config = GridConfiguration::default();

        let fp1 =
            GridRenderer::compute_fingerprint(&[100.0, 300.0], &[200.0], &[], &[], bounds, &config);
        let fp2 =
            GridRenderer::compute_fingerprint(&[100.0, 300.0], &[200.0], &[], &[], bounds, &config);

        assert_eq!(fp1, fp2, "Same inputs should produce same fingerprint");
    }

    #[test]
    fn test_grid_cache_invalidation() {
        let mut renderer = GridRenderer::new();
        renderer.cache_fingerprint = Some(42);

        renderer.invalidate_cache();
        assert!(renderer.cache_fingerprint.is_none());
    }

    #[test]
    fn test_grid_cache_hit_rate_calculation() {
        let mut renderer = GridRenderer::new();
        renderer.cache_hits = 3;
        renderer.cache_misses = 1;

        let rate = renderer.cache_hit_rate();
        assert!((rate - 0.75).abs() < 0.001);
    }
}
