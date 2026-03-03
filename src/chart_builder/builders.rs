// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Individual chart type builders with grid system integration.
//!
//! This module implements specific chart builders for common visualization types:
//! scatter plots, line charts, bar charts, area charts, and heatmaps.
//!
//! # Grid System Integration
//!
//! All chart builders implement the [`GridCapableBuilder`] trait, which provides:
//!
//! - **One-call enabling**: `.grid()` adds professional grid lines instantly
//! - **Theme presets**: `.light_grid()`, `.scientific_grid()`, etc.
//! - **Quick styling**: `.grid_color()`, `.grid_opacity()`, `.grid_width()`
//! - **Directional control**: `.horizontal_grid()`, `.vertical_grid()`
//! - **Full configuration**: `.grid_configuration(GridConfiguration)`
//!
//! See [`crate::grid`] module and `docs/GRID_SYSTEM.md` for comprehensive
//! documentation.

pub mod area;
pub mod bar;
pub mod boxplot;
pub mod choropleth;
pub mod heatmap;
pub mod line;
pub mod scatter;

pub use area::*;
pub use bar::*;
pub use boxplot::*;
pub use choropleth::*;
pub use heatmap::*;
pub use line::*;
pub use scatter::*;

use super::ChartBuilderError;
use super::accessor::{AccessorValue, FieldAccessor};
use crate::error::GupResult;
use crate::grid::{Color, GridConfiguration, GridLineConfig};
use crate::selection::Selection;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;

/// Base trait for all chart builders providing common configuration methods.
pub trait ConfigurableBuilder: Sized {
    /// Set the chart title.
    fn title(self, title: impl Into<String>) -> Self;

    /// Set the chart width in pixels.
    fn width(self, width: f32) -> Self;

    /// Set the chart height in pixels.
    fn height(self, height: f32) -> Self;

    /// Set the chart background color.
    fn background(self, color: [f32; 4]) -> Self;

    /// Enable or disable axes display.
    fn show_axes(self, show: bool) -> Self;

    /// Enable or disable grid display.
    fn show_grid(self, show: bool) -> Self;

    /// Enable or disable hover reveal for clipped text.
    ///
    /// When enabled, truncated axis labels and chart titles show a
    /// tooltip with the full text on hover.
    fn hover_reveal(self, enabled: bool) -> Self;

    /// Set the tooltip configuration for hover reveal.
    ///
    /// Implicitly enables hover reveal.
    fn tooltip_config(self, config: crate::text::hover_reveal::TooltipConfig) -> Self;
}

/// Extended trait for chart builders that support advanced grid configuration.
///
/// `GridCapableBuilder` provides fine-grained control over grid line appearance
/// and behavior, extending beyond the basic show/hide functionality of
/// [`ConfigurableBuilder::show_grid`].
///
/// This trait is implemented by all chart builders: [`ScatterPlotBuilder`],
/// [`LineChartBuilder`], [`BoxPlotBuilder`], [`BarChartBuilder`],
/// [`AreaChartBuilder`], and [`HeatmapBuilder`].
///
/// # Quick Start
///
/// ```rust,ignore
/// use gup::chart_builder::ScatterPlotBuilder;
///
/// let chart = ScatterPlotBuilder::new()
///     .data(data)
///     .grid()                    // Enable with professional defaults
///     .grid_color("#cccccc")     // Customize color
///     .grid_opacity(0.5)         // Adjust transparency
///     .build()?;
/// ```
///
/// # Theme Presets
///
/// | Method | Best For |
/// |--------|----------|
/// | `.light_grid()` | Bright backgrounds |
/// | `.dark_grid()` | Dark mode |
/// | `.scientific_grid()` | Publications (includes minor grids) |
/// | `.business_grid()` | Dashboards (horizontal only) |
/// | `.minimal_grid()` | Design-focused |
/// | `.high_contrast_grid()` | Accessibility |
///
/// See `docs/GRID_SYSTEM.md` for comprehensive documentation.
pub trait GridCapableBuilder: ConfigurableBuilder {
    /// Configure major grid line appearance.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    /// use gup::GridLineConfig;
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .major_grid_style(
    ///         GridLineConfig::default()
    ///             .with_color([0.7, 0.7, 0.7, 1.0])
    ///             .with_line_width(1.0)
    ///     );
    /// ```
    fn major_grid_style(self, config: GridLineConfig) -> Self;

    /// Configure minor grid line appearance.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    /// use gup::GridLineConfig;
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .minor_grid_style(
    ///         GridLineConfig::default()
    ///             .with_color([0.9, 0.9, 0.9, 1.0])
    ///             .with_line_width(0.25)
    ///             .with_opacity(0.3)
    ///     );
    /// ```
    fn minor_grid_style(self, config: GridLineConfig) -> Self;

    /// Show only horizontal grid lines.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .horizontal_grid_only();
    /// ```
    fn horizontal_grid_only(self) -> Self;

    /// Show only vertical grid lines.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .vertical_grid_only();
    /// ```
    fn vertical_grid_only(self) -> Self;

    /// Enable minor grid lines with default styling.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .with_minor_grid();
    /// ```
    fn with_minor_grid(self) -> Self;

    /// Disable minor grid lines.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .without_minor_grid();
    /// ```
    fn without_minor_grid(self) -> Self;

    /// Set complete grid configuration.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    /// use gup::GridConfiguration;
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .grid_configuration(GridConfiguration::horizontal_only());
    /// ```
    fn grid_configuration(self, config: GridConfiguration) -> Self;

    // Enhanced convenience methods from GUP-097

    /// Enable grid rendering with professional defaults.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .grid(); // Simple one-line grid enabling
    /// ```
    fn grid(self) -> Self {
        self.grid_configuration(GridConfiguration::default())
            .show_grid(true)
    }

    /// Show only horizontal grid lines (convenience method).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .horizontal_grid();
    /// ```
    fn horizontal_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::horizontal_only())
            .show_grid(true)
    }

    /// Show only vertical grid lines (convenience method).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .vertical_grid();
    /// ```
    fn vertical_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::vertical_only())
            .show_grid(true)
    }

    /// Set grid line color.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .grid_color("#cccccc"); // Hex color
    /// ```
    fn grid_color(self, color: impl Into<Color>) -> Self {
        let color: Color = color.into();
        let config = GridConfiguration::default()
            .with_major_grid(GridLineConfig::default().with_color(color.to_rgba()));
        self.grid_configuration(config).show_grid(true)
    }

    /// Set grid line opacity.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .grid_opacity(0.5); // Semi-transparent
    /// ```
    fn grid_opacity(self, opacity: f32) -> Self {
        let config = GridConfiguration::default()
            .with_major_grid(GridLineConfig::default().with_opacity(opacity));
        self.grid_configuration(config).show_grid(true)
    }

    /// Set grid line width.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .grid_width(1.0); // Thicker lines
    /// ```
    fn grid_width(self, width: f32) -> Self {
        let config = GridConfiguration::default()
            .with_major_grid(GridLineConfig::default().with_line_width(width));
        self.grid_configuration(config).show_grid(true)
    }

    /// Apply light theme grid.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .light_grid(); // Professional light theme
    /// ```
    fn light_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::light_theme())
            .show_grid(true)
    }

    /// Apply dark theme grid.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .dark_grid(); // Professional dark theme
    /// ```
    fn dark_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::dark_theme())
            .show_grid(true)
    }

    /// Apply scientific/technical grid theme.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .scientific_grid(); // Scientific visualization grid
    /// ```
    fn scientific_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::scientific())
            .show_grid(true)
    }

    /// Apply business/dashboard grid theme.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .business_grid(); // Clean business chart grid
    /// ```
    fn business_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::business())
            .show_grid(true)
    }

    /// Apply minimal grid theme.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .minimal_grid(); // Very subtle grid
    /// ```
    fn minimal_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::minimal())
            .show_grid(true)
    }

    /// Apply high contrast grid theme.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::builders::{scatter, GridCapableBuilder};
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct DataPoint { x: f32, y: f32 }
    /// let chart = scatter::<DataPoint>()
    ///     .high_contrast_grid(); // Accessible high contrast
    /// ```
    fn high_contrast_grid(self) -> Self {
        self.grid_configuration(GridConfiguration::high_contrast())
            .show_grid(true)
    }
}

/// Helper trait for converting accessor values to attribute binding closures.
pub trait AccessorToShaderFunction<T> {
    /// Convert an accessor value to a position binding closure.
    #[cfg(not(target_arch = "wasm32"))]
    fn to_position_shader(
        &self,
        accessor: AccessorFunction<T>,
    ) -> Box<dyn Fn(&T) -> [f32; 2] + Send + Sync>;

    /// Convert an accessor value to a position binding closure.
    #[cfg(target_arch = "wasm32")]
    fn to_position_shader(&self, accessor: AccessorFunction<T>) -> Box<dyn Fn(&T) -> [f32; 2]>;

    /// Convert an accessor value to a colour binding closure.
    #[cfg(not(target_arch = "wasm32"))]
    fn to_color_shader(
        &self,
        accessor: AccessorFunction<T>,
    ) -> Box<dyn Fn(&T) -> [f32; 4] + Send + Sync>;

    /// Convert an accessor value to a colour binding closure.
    #[cfg(target_arch = "wasm32")]
    fn to_color_shader(&self, accessor: AccessorFunction<T>) -> Box<dyn Fn(&T) -> [f32; 4]>;
}

/// Type-erased accessor function for dynamic dispatch.
pub struct AccessorFunction<T> {
    #[cfg(not(target_arch = "wasm32"))]
    function: Box<dyn Fn(&T) -> AccessorValue + Send + Sync>,
    #[cfg(target_arch = "wasm32")]
    function: Box<dyn Fn(&T) -> AccessorValue>,
    field_name: Option<String>,
    _phantom: PhantomData<T>,
}

impl<T> AccessorFunction<T> {
    /// Create a new accessor function from a closure.
    pub fn new<F>(function: F) -> Self
    where
        F: Fn(&T) -> AccessorValue + MaybeSend + MaybeSync + 'static,
    {
        Self {
            function: Box::new(function),
            field_name: None,
            _phantom: PhantomData,
        }
    }

    /// Create a new accessor function from a field name.
    pub fn from_field(field_name: &str) -> Self {
        let field_name_owned = field_name.to_string();
        Self {
            function: Box::new(move |_data| {
                // In a real implementation, this would use reflection or a registry
                // For now, return a placeholder value
                AccessorValue::Float(0.0)
            }),
            field_name: Some(field_name_owned),
            _phantom: PhantomData,
        }
    }

    /// Apply the accessor function to extract a value.
    pub fn apply(&self, data: &T) -> AccessorValue {
        (self.function)(data)
    }

    /// Get the field name if this accessor is field-based.
    pub fn field_name(&self) -> Option<&str> {
        self.field_name.as_deref()
    }
}

impl<T> std::fmt::Debug for AccessorFunction<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessorFunction")
            .field("field_name", &self.field_name)
            .field("function", &"<closure>")
            .finish()
    }
}

impl<T> Clone for AccessorFunction<T> {
    fn clone(&self) -> Self {
        // Note: This is a simplified clone that loses the actual function
        // In a real implementation, we'd need to handle this differently
        AccessorFunction::from_field(self.field_name.as_deref().unwrap_or("unknown"))
    }
}

/// Convert various accessor types to internal AccessorFunction.
impl<T> From<FieldAccessor> for AccessorFunction<T> {
    fn from(field_accessor: FieldAccessor) -> Self {
        AccessorFunction::from_field(field_accessor.field_name())
    }
}

impl<T, F> From<F> for AccessorFunction<T>
where
    F: Fn(&T) -> AccessorValue + MaybeSend + MaybeSync + 'static,
{
    fn from(function: F) -> Self {
        AccessorFunction::new(function)
    }
}

/// Utility function to validate required accessors.
pub fn validate_required_accessors<T>(
    x_accessor: &Option<AccessorFunction<T>>,
    y_accessor: &Option<AccessorFunction<T>>,
) -> GupResult<()> {
    if x_accessor.is_none() {
        return Err(ChartBuilderError::MissingAccessor {
            attribute: "x".to_string(),
        }
        .into());
    }

    if y_accessor.is_none() {
        return Err(ChartBuilderError::MissingAccessor {
            attribute: "y".to_string(),
        }
        .into());
    }

    Ok(())
}

/// Utility function to apply accessor functions to a selection.
pub fn apply_accessors_to_selection<T, M>(
    selection: &mut Selection<T, M>,
    x_accessor: &Option<AccessorFunction<T>>,
    y_accessor: &Option<AccessorFunction<T>>,
    color_accessor: &Option<AccessorFunction<T>>,
    size_accessor: &Option<AccessorFunction<T>>,
) -> GupResult<()>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
    M::AttributeValue: Default + Clone,
{
    // Apply position mapping if both X and Y are provided
    if let (Some(x_acc), Some(y_acc)) = (x_accessor, y_accessor) {
        let _x_field = x_acc.field_name().unwrap_or("x").to_string();
        let _y_field = y_acc.field_name().unwrap_or("y").to_string();

        let position_shader = move |_data: &T| {
            // In a real implementation, this would use the actual accessor functions
            // For now, provide a placeholder that compiles
            [0.0f32, 0.0]
        };

        selection.attr("position", position_shader);
    }

    // Apply color mapping if provided
    if let Some(color_acc) = color_accessor {
        let _color_field = color_acc.field_name().unwrap_or("color").to_string();

        let color_shader = move |_data: &T| {
            // In a real implementation, this would use the actual accessor function
            // For now, provide a default color
            [1.0f32, 0.0, 0.0, 1.0]
        };

        selection.attr("color", color_shader);
    }

    // Size mapping would be similar but depends on the mark type
    if let Some(_size_acc) = size_accessor {
        // Size mapping implementation would go here
        // This depends on the specific mark type's attribute system
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::x;

    #[derive(Debug, Clone)]
    struct TestData {
        x: f32,
        y: f32,
        value: f32,
        category: String,
    }

    #[test]
    fn test_accessor_function_creation() {
        // Test closure-based accessor
        let closure_accessor =
            AccessorFunction::<TestData>::new(|data: &TestData| AccessorValue::Float(data.x));

        let data = TestData {
            x: 10.0,
            y: 20.0,
            value: 30.0,
            category: "A".to_string(),
        };

        let result = closure_accessor.apply(&data);
        assert_eq!(result, AccessorValue::Float(10.0));

        // Test field-based accessor
        let field_accessor = AccessorFunction::<TestData>::from_field("x");
        assert_eq!(field_accessor.field_name(), Some("x"));
    }

    #[test]
    fn test_field_accessor_conversion() {
        let field_accessor = x("revenue");
        let accessor_function: AccessorFunction<TestData> = field_accessor.into();
        assert_eq!(accessor_function.field_name(), Some("revenue"));
    }

    #[test]
    fn test_validate_required_accessors() {
        let x_accessor = Some(AccessorFunction::<TestData>::from_field("x"));
        let y_accessor = Some(AccessorFunction::<TestData>::from_field("y"));

        // Both provided - should succeed
        let result = validate_required_accessors(&x_accessor, &y_accessor);
        assert!(result.is_ok());

        // Missing X accessor - should fail
        let result = validate_required_accessors(&None, &y_accessor);
        assert!(result.is_err());
        if let Err(err) = result {
            let error_str = format!("{err}");
            assert!(error_str.contains("Missing required accessor"));
            assert!(error_str.contains("x"));
        }

        // Missing Y accessor - should fail
        let result = validate_required_accessors(&x_accessor, &None);
        assert!(result.is_err());
        if let Err(err) = result {
            let error_str = format!("{err}");
            assert!(error_str.contains("Missing required accessor"));
            assert!(error_str.contains("y"));
        }
    }

    #[test]
    fn test_accessor_function_clone() {
        let accessor = AccessorFunction::<TestData>::from_field("test_field");
        let cloned_accessor = accessor.clone();

        assert_eq!(accessor.field_name(), cloned_accessor.field_name());
        assert_eq!(cloned_accessor.field_name(), Some("test_field"));
    }

    #[test]
    fn test_accessor_value_application() {
        let data = TestData {
            x: 15.0,
            y: 25.0,
            value: 35.0,
            category: "B".to_string(),
        };

        // Test different accessor value types
        let float_accessor =
            AccessorFunction::<TestData>::new(|d: &TestData| AccessorValue::Float(d.value));
        assert_eq!(float_accessor.apply(&data), AccessorValue::Float(35.0));

        let position_accessor =
            AccessorFunction::<TestData>::new(|d: &TestData| AccessorValue::Position([d.x, d.y]));
        assert_eq!(
            position_accessor.apply(&data),
            AccessorValue::Position([15.0, 25.0])
        );

        let string_accessor = AccessorFunction::<TestData>::new(|d: &TestData| {
            AccessorValue::String(d.category.clone())
        });
        assert_eq!(
            string_accessor.apply(&data),
            AccessorValue::String("B".to_string())
        );
    }
}
