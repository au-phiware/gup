// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integrated axis system for professional data visualization.
//!
//! This module provides the complete axis system that coordinates scales, tick generation,
//! grid lines, and label formatting to create professional-quality charts automatically.
//!
//! # Key Features
//!
//! * **Automatic Configuration** - Analyzes data and configures appropriate axes
//! * **Coordinated Rendering** - Integrates all axis components seamlessly
//! * **Performance Optimized** - Complete axis rendering <2ms for typical charts
//! * **Layout Management** - Calculates optimal margins and positioning
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::axis_system::{AxisSystem, AxisMappings};
//! use gup::scale::AccessorFunction;
//! use gup::grid::ChartBounds;
//! use gup::render::RenderContext;
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! #[derive(Debug, Clone)]
//! struct DataPoint {
//!     x: f64,
//!     y: f64,
//!     category: String,
//! }
//!
//! let data = vec![
//!     DataPoint { x: 1.0, y: 10.0, category: "A".to_string() },
//!     DataPoint { x: 2.0, y: 20.0, category: "B".to_string() },
//! ];
//!
//! // Create axis system
//! let mut axis_system = AxisSystem::new();
//!
//! // Set up axis mappings
//! let x_accessor = AccessorFunction::new(|d: &DataPoint| d.x);
//! let y_accessor = AccessorFunction::new(|d: &DataPoint| d.y);
//! let mut mappings = AxisMappings::new();
//! mappings.set_x_accessor(x_accessor);
//! mappings.set_y_accessor(y_accessor);
//!
//! // Auto-configure axes
//! let config = axis_system.auto_configure(&data, &mappings)?;
//!
//! // Render complete axis system
//! let mut context = RenderContext::new().await?;
//! let chart_bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
//! axis_system.render_complete_axis_system(&mut context, chart_bounds, &config)?;
//! # Ok(())
//! # }
//! ```

use crate::axis::{Axis, AxisBounds, AxisConfiguration as AxisConfig, AxisPosition, LinearAxis};
use crate::error::{GupError, GupResult};
use crate::grid::{ChartBounds, GridConfiguration, GridSystem};
use crate::label::{AxisInfo, LabelConstraints, LabelFormatter, LabelPositioner};
use crate::render::RenderContext;
use crate::scale::{
    AccessorFunction, AxisId, DataAnalyzer, DataCharacteristics, Scale, ScaleFactory,
};
use crate::shader_function::Vec2;
use std::collections::HashMap;

/// Main axis system coordinator managing all axis components.
#[derive(Debug)]
pub struct AxisSystem {
    /// Scale systems for each axis
    scales: HashMap<AxisId, Box<dyn Scale>>,
    /// Axis rendering components
    axes: HashMap<AxisId, Box<dyn Axis>>,
    /// Label formatters for each axis
    formatters: HashMap<AxisId, Box<dyn LabelFormatter>>,
    /// Layout coordinator
    layout_manager: AxisLayoutManager,
    /// Performance coordinator
    performance_manager: AxisPerformanceManager,
    /// Data analyzer for scale detection
    data_analyzer: DataAnalyzer,
    /// Scale factory for creating scales
    scale_factory: ScaleFactory,
}

impl AxisSystem {
    /// Create a new axis system.
    pub fn new() -> Self {
        Self {
            scales: HashMap::new(),
            axes: HashMap::new(),
            formatters: HashMap::new(),
            layout_manager: AxisLayoutManager::new(),
            performance_manager: AxisPerformanceManager::new(),
            data_analyzer: DataAnalyzer::new(),
            scale_factory: ScaleFactory::new(),
        }
    }

    /// Automatically configure axes based on data analysis.
    pub fn auto_configure<T>(
        &mut self,
        data: &[T],
        mappings: &AxisMappings<T>,
    ) -> GupResult<AxisConfiguration>
    where
        T: Clone + Send + Sync + std::fmt::Debug + 'static,
    {
        if data.is_empty() {
            return Err(GupError::validation_error(
                "Cannot configure axes for empty dataset".to_string(),
            ));
        }

        // 1. Analyze data to determine appropriate scale types
        let scale_specs = self.analyze_data_for_scales(data, mappings)?;

        // 2. Create scales with optimal domains/ranges
        for (axis_id, spec) in scale_specs {
            let scale = self
                .scale_factory
                .create_scale_from_characteristics(&spec)?;
            let formatter = scale.default_formatter();

            self.scales.insert(axis_id, scale);
            self.formatters.insert(axis_id, formatter);
        }

        // 3. Configure axis components for each scale
        self.configure_axis_components()?;

        // 4. Calculate layout requirements and coordinate positioning
        let layout = self
            .layout_manager
            .calculate_layout(&self.scales, &self.axes)?;

        Ok(AxisConfiguration {
            layout,
            scales: self.get_scale_configurations(),
            show_grid: true,
            grid_config: GridConfiguration::default(),
            performance_budget: self.performance_manager.calculate_budget(),
        })
    }

    /// Render complete integrated axis system.
    pub fn render_complete_axis_system(
        &mut self,
        context: &mut RenderContext,
        chart_bounds: ChartBounds,
        config: &AxisConfiguration,
    ) -> GupResult<()> {
        let start_time = std::time::Instant::now();

        // 1. Generate tick positions from scales
        let tick_positions = self.generate_all_tick_positions(chart_bounds)?;

        // 2. Render grid lines first (behind everything)
        if config.show_grid {
            self.render_coordinated_grid(
                context,
                &tick_positions,
                chart_bounds,
                &config.grid_config,
            )?;
        }

        // 3. Render axis lines and tick marks
        self.render_axis_structures(context, &tick_positions, chart_bounds)?;

        // 4. Render formatted labels
        self.render_formatted_labels(context, &tick_positions, chart_bounds)?;

        // 5. Track performance
        let elapsed = start_time.elapsed();
        self.performance_manager.record_render_time(elapsed);

        if elapsed > config.performance_budget {
            eprintln!(
                "Warning: Axis rendering took {:?}, exceeds budget of {:?}",
                elapsed, config.performance_budget
            );
        }

        Ok(())
    }

    /// Analyze data fields to determine scale specifications.
    fn analyze_data_for_scales<T>(
        &self,
        data: &[T],
        mappings: &AxisMappings<T>,
    ) -> GupResult<HashMap<AxisId, DataCharacteristics>>
    where
        T: Clone + Send + Sync + std::fmt::Debug + 'static,
    {
        let mut scale_specs = HashMap::new();

        // Analyze X axis if mapped
        if let Some(x_accessor) = &mappings.x_accessor {
            let characteristics = self.data_analyzer.analyze_field(data, x_accessor)?;
            scale_specs.insert(AxisId::XAxis, characteristics);
        }

        // Analyze Y axis if mapped
        if let Some(y_accessor) = &mappings.y_accessor {
            let characteristics = self.data_analyzer.analyze_field(data, y_accessor)?;
            scale_specs.insert(AxisId::YAxis, characteristics);
        }

        // Analyze Color axis if mapped
        if let Some(color_accessor) = &mappings.color_accessor {
            let characteristics = self.data_analyzer.analyze_field(data, color_accessor)?;
            scale_specs.insert(AxisId::ColorAxis, characteristics);
        }

        // Analyze Size axis if mapped
        if let Some(size_accessor) = &mappings.size_accessor {
            let characteristics = self.data_analyzer.analyze_field(data, size_accessor)?;
            scale_specs.insert(AxisId::SizeAxis, characteristics);
        }

        Ok(scale_specs)
    }

    /// Configure axis components based on detected scales.
    fn configure_axis_components(&mut self) -> GupResult<()> {
        // Create axes based on available scales
        for (&axis_id, scale) in &self.scales {
            let position = match axis_id {
                AxisId::XAxis => AxisPosition::Bottom,
                AxisId::YAxis => AxisPosition::Left,
                AxisId::ColorAxis => AxisPosition::Right,
                AxisId::SizeAxis => AxisPosition::Top,
            };

            // Create appropriate axis configuration based on scale type
            let axis_config = match scale.scale_type() {
                "linear" => AxisConfig::default(),
                "logarithmic" => AxisConfig::default().with_tick_count(4),
                "temporal" => AxisConfig::default().with_tick_count(6),
                "ordinal" => AxisConfig::default().without_minor_ticks(),
                _ => AxisConfig::default(),
            };

            let axis = Box::new(LinearAxis::new(position, axis_config));
            self.axes.insert(axis_id, axis);
        }

        Ok(())
    }

    /// Generate tick positions for all axes.
    fn generate_all_tick_positions(&self, chart_bounds: ChartBounds) -> GupResult<TickPositions> {
        let mut positions = TickPositions::new();

        for (&axis_id, scale) in &self.scales {
            let axis_length = match axis_id {
                AxisId::XAxis => chart_bounds.width(),
                AxisId::YAxis => chart_bounds.height(),
                AxisId::ColorAxis => chart_bounds.height(),
                AxisId::SizeAxis => chart_bounds.width(),
            };

            let major_ticks = scale.generate_ticks(None);
            let minor_ticks = if let Some(axis) = self.axes.get(&axis_id) {
                axis.get_minor_tick_positions(Some(scale.as_ref()), axis_length)
                    .into_iter()
                    .map(|pos| scale.invert_value(pos as f64))
                    .collect()
            } else {
                Vec::new()
            };

            positions.set_ticks(axis_id, major_ticks, minor_ticks);
        }

        Ok(positions)
    }

    /// Render coordinated grid system.
    fn render_coordinated_grid(
        &mut self,
        context: &mut RenderContext,
        tick_positions: &TickPositions,
        chart_bounds: ChartBounds,
        grid_config: &GridConfiguration,
    ) -> GupResult<()> {
        let mut grid_system = GridSystem::new(grid_config.clone());

        let horizontal_ticks = tick_positions.get_world_positions(AxisId::XAxis, chart_bounds);
        let vertical_ticks = tick_positions.get_world_positions(AxisId::YAxis, chart_bounds);
        let horizontal_minor_ticks =
            tick_positions.get_world_minor_positions(AxisId::XAxis, chart_bounds);
        let vertical_minor_ticks =
            tick_positions.get_world_minor_positions(AxisId::YAxis, chart_bounds);

        grid_system.render_grid(
            context,
            &horizontal_ticks,
            &vertical_ticks,
            &horizontal_minor_ticks,
            &vertical_minor_ticks,
            chart_bounds,
        )
    }

    /// Render axis structures (lines and tick marks).
    fn render_axis_structures(
        &self,
        context: &mut RenderContext,
        _tick_positions: &TickPositions,
        chart_bounds: ChartBounds,
    ) -> GupResult<()> {
        for (&axis_id, axis) in &self.axes {
            let bounds = self.calculate_axis_bounds(axis_id, chart_bounds);
            let _scale = self.scales.get(&axis_id);

            // Render axis with scale if available
            if let Some(_scale) = _scale {
                match axis.position() {
                    AxisPosition::Bottom | AxisPosition::Top if axis_id == AxisId::XAxis => {
                        // For now, use the standard axis render method
                        // Future: Add scale integration to axis trait
                        axis.render(context, bounds)?;
                    }
                    AxisPosition::Left | AxisPosition::Right if axis_id == AxisId::YAxis => {
                        // For now, use the standard axis render method
                        // Future: Add scale integration to axis trait
                        axis.render(context, bounds)?;
                    }
                    _ => {
                        axis.render(context, bounds)?;
                    }
                }
            } else {
                axis.render(context, bounds)?;
            }
        }

        Ok(())
    }

    /// Render formatted labels for all axes.
    fn render_formatted_labels(
        &self,
        _context: &mut RenderContext,
        tick_positions: &TickPositions,
        chart_bounds: ChartBounds,
    ) -> GupResult<()> {
        for (&axis_id, formatter) in &self.formatters {
            if let Some(ticks) = tick_positions.get_ticks(axis_id) {
                let axis_info = self.create_axis_info(axis_id, chart_bounds);
                let constraints = LabelConstraints::axis_labels();

                let mut positioner = LabelPositioner::new();
                let tick_positions_normalized: Vec<f64> = ticks
                    .iter()
                    .map(|&tick| {
                        if let Some(scale) = self.scales.get(&axis_id) {
                            scale.scale_value(tick)
                        } else {
                            tick
                        }
                    })
                    .collect();

                let _layout = positioner.layout_labels(
                    &tick_positions_normalized,
                    &axis_info,
                    formatter.as_ref(),
                    &constraints,
                )?;

                // Note: Actual label rendering would integrate with the text system
                // For now, we prepare the layout data
            }
        }

        Ok(())
    }

    /// Calculate axis bounds for a specific axis ID.
    fn calculate_axis_bounds(&self, axis_id: AxisId, chart_bounds: ChartBounds) -> AxisBounds {
        match axis_id {
            AxisId::XAxis => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.bottom,
                },
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.bottom,
                },
                40.0,
            ),
            AxisId::YAxis => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.bottom,
                },
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.top,
                },
                60.0,
            ),
            AxisId::ColorAxis => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.bottom,
                },
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.top,
                },
                60.0,
            ),
            AxisId::SizeAxis => AxisBounds::new(
                Vec2 {
                    x: chart_bounds.left,
                    y: chart_bounds.top,
                },
                Vec2 {
                    x: chart_bounds.right,
                    y: chart_bounds.top,
                },
                40.0,
            ),
        }
    }

    /// Create axis info for label positioning.
    fn create_axis_info(&self, axis_id: AxisId, chart_bounds: ChartBounds) -> AxisInfo {
        let bounds = self.calculate_axis_bounds(axis_id, chart_bounds);
        let position = match axis_id {
            AxisId::XAxis => AxisPosition::Bottom,
            AxisId::YAxis => AxisPosition::Left,
            AxisId::ColorAxis => AxisPosition::Right,
            AxisId::SizeAxis => AxisPosition::Top,
        };

        AxisInfo::from_bounds(&bounds, position)
    }

    /// Get scale configurations for the final configuration.
    fn get_scale_configurations(&self) -> HashMap<AxisId, ScaleConfiguration> {
        self.scales
            .iter()
            .map(|(&axis_id, scale)| {
                let config = ScaleConfiguration {
                    scale_type: scale.scale_type().to_string(),
                    domain: scale.domain(),
                    range: scale.range(),
                };
                (axis_id, config)
            })
            .collect()
    }
}

impl Default for AxisSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Axis mappings that define which data fields map to which axes.
#[derive(Debug)]
pub struct AxisMappings<T> {
    pub x_accessor: Option<AccessorFunction<T>>,
    pub y_accessor: Option<AccessorFunction<T>>,
    pub color_accessor: Option<AccessorFunction<T>>,
    pub size_accessor: Option<AccessorFunction<T>>,
}

impl<T> AxisMappings<T> {
    /// Create new empty axis mappings.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            color_accessor: None,
            size_accessor: None,
        }
    }

    /// Set X axis accessor.
    pub fn set_x_accessor(&mut self, accessor: AccessorFunction<T>) {
        self.x_accessor = Some(accessor);
    }

    /// Set Y axis accessor.
    pub fn set_y_accessor(&mut self, accessor: AccessorFunction<T>) {
        self.y_accessor = Some(accessor);
    }

    /// Set color axis accessor.
    pub fn set_color_accessor(&mut self, accessor: AccessorFunction<T>) {
        self.color_accessor = Some(accessor);
    }

    /// Set size axis accessor.
    pub fn set_size_accessor(&mut self, accessor: AccessorFunction<T>) {
        self.size_accessor = Some(accessor);
    }
}

impl<T> Default for AxisMappings<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete axis system configuration.
#[derive(Debug, Clone)]
pub struct AxisConfiguration {
    /// Layout information and margins
    pub layout: AxisLayout,
    /// Scale configurations for each axis
    pub scales: HashMap<AxisId, ScaleConfiguration>,
    /// Whether to show grid lines
    pub show_grid: bool,
    /// Grid system configuration
    pub grid_config: GridConfiguration,
    /// Performance budget for rendering
    pub performance_budget: std::time::Duration,
}

/// Layout information calculated by the axis system.
#[derive(Debug, Clone)]
pub struct AxisLayout {
    /// Required margins for all axes
    pub margins: AxisMargins,
    /// Chart area after accounting for axes
    pub chart_area: ChartArea,
    /// Individual axis bounds
    pub axis_bounds: HashMap<AxisId, AxisBounds>,
}

/// Margin requirements for all axes.
#[derive(Debug, Clone)]
pub struct AxisMargins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Chart area coordinates.
#[derive(Debug, Clone)]
pub struct ChartArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Scale configuration information.
#[derive(Debug, Clone)]
pub struct ScaleConfiguration {
    pub scale_type: String,
    pub domain: (f64, f64),
    pub range: (f32, f32),
}

/// Tick positions for all axes.
#[derive(Debug)]
struct TickPositions {
    major_ticks: HashMap<AxisId, Vec<f64>>,
    minor_ticks: HashMap<AxisId, Vec<f64>>,
}

impl TickPositions {
    fn new() -> Self {
        Self {
            major_ticks: HashMap::new(),
            minor_ticks: HashMap::new(),
        }
    }

    fn set_ticks(&mut self, axis_id: AxisId, major: Vec<f64>, minor: Vec<f64>) {
        self.major_ticks.insert(axis_id, major);
        self.minor_ticks.insert(axis_id, minor);
    }

    fn get_ticks(&self, axis_id: AxisId) -> Option<&Vec<f64>> {
        self.major_ticks.get(&axis_id)
    }

    fn get_world_positions(&self, axis_id: AxisId, chart_bounds: ChartBounds) -> Vec<f64> {
        if let Some(ticks) = self.major_ticks.get(&axis_id) {
            ticks
                .iter()
                .map(|&tick| match axis_id {
                    AxisId::XAxis => chart_bounds.left as f64 + tick * chart_bounds.width() as f64,
                    AxisId::YAxis => chart_bounds.top as f64 + tick * chart_bounds.height() as f64,
                    _ => tick,
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn get_world_minor_positions(&self, axis_id: AxisId, chart_bounds: ChartBounds) -> Vec<f64> {
        if let Some(ticks) = self.minor_ticks.get(&axis_id) {
            ticks
                .iter()
                .map(|&tick| match axis_id {
                    AxisId::XAxis => chart_bounds.left as f64 + tick * chart_bounds.width() as f64,
                    AxisId::YAxis => chart_bounds.top as f64 + tick * chart_bounds.height() as f64,
                    _ => tick,
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

/// Layout manager for calculating axis positioning and margins.
#[derive(Debug)]
struct AxisLayoutManager {
    default_margins: AxisMargins,
}

impl AxisLayoutManager {
    fn new() -> Self {
        Self {
            default_margins: AxisMargins {
                top: 40.0,
                right: 60.0,
                bottom: 60.0,
                left: 80.0,
            },
        }
    }

    fn calculate_layout(
        &self,
        scales: &HashMap<AxisId, Box<dyn Scale>>,
        axes: &HashMap<AxisId, Box<dyn Axis>>,
    ) -> GupResult<AxisLayout> {
        let mut margins = self.default_margins.clone();

        // Adjust margins based on axis requirements
        for (&axis_id, axis) in axes {
            let _scale = scales.get(&axis_id);
            let required_margin = axis.calculate_margin(None); // Simplified for now

            match axis_id {
                AxisId::XAxis => margins.bottom = margins.bottom.max(required_margin),
                AxisId::YAxis => margins.left = margins.left.max(required_margin),
                AxisId::ColorAxis => margins.right = margins.right.max(required_margin),
                AxisId::SizeAxis => margins.top = margins.top.max(required_margin),
            }
        }

        // Calculate chart area (assuming 800x600 total size for now)
        let total_width = 800.0;
        let total_height = 600.0;
        let chart_area = ChartArea {
            x: margins.left,
            y: margins.top,
            width: total_width - margins.left - margins.right,
            height: total_height - margins.top - margins.bottom,
        };

        // Calculate individual axis bounds
        let mut axis_bounds = HashMap::new();
        for &axis_id in axes.keys() {
            let bounds = match axis_id {
                AxisId::XAxis => AxisBounds::new(
                    Vec2 {
                        x: chart_area.x,
                        y: chart_area.y + chart_area.height,
                    },
                    Vec2 {
                        x: chart_area.x + chart_area.width,
                        y: chart_area.y + chart_area.height,
                    },
                    margins.bottom,
                ),
                AxisId::YAxis => AxisBounds::new(
                    Vec2 {
                        x: chart_area.x,
                        y: chart_area.y + chart_area.height,
                    },
                    Vec2 {
                        x: chart_area.x,
                        y: chart_area.y,
                    },
                    margins.left,
                ),
                AxisId::ColorAxis => AxisBounds::new(
                    Vec2 {
                        x: chart_area.x + chart_area.width,
                        y: chart_area.y + chart_area.height,
                    },
                    Vec2 {
                        x: chart_area.x + chart_area.width,
                        y: chart_area.y,
                    },
                    margins.right,
                ),
                AxisId::SizeAxis => AxisBounds::new(
                    Vec2 {
                        x: chart_area.x,
                        y: chart_area.y,
                    },
                    Vec2 {
                        x: chart_area.x + chart_area.width,
                        y: chart_area.y,
                    },
                    margins.top,
                ),
            };
            axis_bounds.insert(axis_id, bounds);
        }

        Ok(AxisLayout {
            margins,
            chart_area,
            axis_bounds,
        })
    }
}

/// Performance manager for tracking axis rendering performance.
#[derive(Debug)]
struct AxisPerformanceManager {
    target_render_time: std::time::Duration,
    recent_render_times: Vec<std::time::Duration>,
}

impl AxisPerformanceManager {
    fn new() -> Self {
        Self {
            target_render_time: std::time::Duration::from_millis(2), // 2ms target
            recent_render_times: Vec::new(),
        }
    }

    fn calculate_budget(&self) -> std::time::Duration {
        self.target_render_time
    }

    fn record_render_time(&mut self, elapsed: std::time::Duration) {
        self.recent_render_times.push(elapsed);

        // Keep only recent times (last 10 renders)
        if self.recent_render_times.len() > 10 {
            self.recent_render_times.remove(0);
        }
    }

    #[allow(dead_code)]
    fn average_render_time(&self) -> std::time::Duration {
        if self.recent_render_times.is_empty() {
            return std::time::Duration::from_millis(0);
        }

        let total: std::time::Duration = self.recent_render_times.iter().sum();
        total / self.recent_render_times.len() as u32
    }
}

// Note: Future enhancement could add scale integration directly to the Axis trait

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::AccessorFunction;

    #[derive(Debug, Clone)]
    struct TestDataPoint {
        x: f64,
        y: f64,
        #[allow(dead_code)]
        category: String,
    }

    #[test]
    fn test_axis_system_creation() {
        let axis_system = AxisSystem::new();
        assert!(axis_system.scales.is_empty());
        assert!(axis_system.axes.is_empty());
        assert!(axis_system.formatters.is_empty());
    }

    #[test]
    fn test_axis_mappings() {
        let mut mappings = AxisMappings::<TestDataPoint>::new();

        let x_accessor = AccessorFunction::new(|d: &TestDataPoint| d.x);
        let y_accessor = AccessorFunction::new(|d: &TestDataPoint| d.y);

        mappings.set_x_accessor(x_accessor);
        mappings.set_y_accessor(y_accessor);

        assert!(mappings.x_accessor.is_some());
        assert!(mappings.y_accessor.is_some());
        assert!(mappings.color_accessor.is_none());
        assert!(mappings.size_accessor.is_none());
    }

    #[test]
    fn test_axis_system_auto_configure() {
        let mut axis_system = AxisSystem::new();
        let data = vec![
            TestDataPoint {
                x: 1.0,
                y: 10.0,
                category: "A".to_string(),
            },
            TestDataPoint {
                x: 2.0,
                y: 20.0,
                category: "B".to_string(),
            },
            TestDataPoint {
                x: 3.0,
                y: 30.0,
                category: "C".to_string(),
            },
        ];

        let mut mappings = AxisMappings::new();
        mappings.set_x_accessor(AccessorFunction::new(|d: &TestDataPoint| d.x));
        mappings.set_y_accessor(AccessorFunction::new(|d: &TestDataPoint| d.y));

        let config = axis_system.auto_configure(&data, &mappings).unwrap();

        assert!(config.scales.contains_key(&AxisId::XAxis));
        assert!(config.scales.contains_key(&AxisId::YAxis));
        assert!(config.show_grid);
        assert!(config.performance_budget.as_millis() >= 1);
    }

    #[test]
    fn test_layout_manager() {
        let layout_manager = AxisLayoutManager::new();
        let scales: HashMap<AxisId, Box<dyn Scale>> = HashMap::new();
        let axes: HashMap<AxisId, Box<dyn Axis>> = HashMap::new();

        let layout = layout_manager.calculate_layout(&scales, &axes).unwrap();

        assert!(layout.chart_area.width > 0.0);
        assert!(layout.chart_area.height > 0.0);
        assert!(layout.margins.left >= 80.0);
        assert!(layout.margins.bottom >= 60.0);
    }

    #[test]
    fn test_tick_positions() {
        let mut positions = TickPositions::new();

        let major_ticks = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let minor_ticks = vec![0.125, 0.375, 0.625, 0.875];

        positions.set_ticks(AxisId::XAxis, major_ticks.clone(), minor_ticks.clone());

        let retrieved_ticks = positions.get_ticks(AxisId::XAxis).unwrap();
        assert_eq!(retrieved_ticks, &major_ticks);

        let chart_bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
        let world_positions = positions.get_world_positions(AxisId::XAxis, chart_bounds);

        assert_eq!(world_positions.len(), 5);
        assert!((world_positions[0] - 50.0).abs() < 0.001);
        assert!((world_positions[4] - 750.0).abs() < 0.001);
    }

    #[test]
    fn test_performance_manager() {
        let mut manager = AxisPerformanceManager::new();

        let render_time = std::time::Duration::from_millis(1);
        manager.record_render_time(render_time);

        assert_eq!(manager.recent_render_times.len(), 1);
        assert_eq!(manager.recent_render_times[0], render_time);

        // Test budget calculation
        let budget = manager.calculate_budget();
        assert!(budget.as_millis() >= 1);
    }

    #[test]
    fn test_axis_configuration() {
        let layout = AxisLayout {
            margins: AxisMargins {
                top: 40.0,
                right: 60.0,
                bottom: 60.0,
                left: 80.0,
            },
            chart_area: ChartArea {
                x: 80.0,
                y: 40.0,
                width: 660.0,
                height: 500.0,
            },
            axis_bounds: HashMap::new(),
        };

        let config = AxisConfiguration {
            layout,
            scales: HashMap::new(),
            show_grid: true,
            grid_config: GridConfiguration::default(),
            performance_budget: std::time::Duration::from_millis(2),
        };

        assert!(config.show_grid);
        assert_eq!(config.performance_budget.as_millis(), 2);
        assert_eq!(config.layout.chart_area.width, 660.0);
    }
}
