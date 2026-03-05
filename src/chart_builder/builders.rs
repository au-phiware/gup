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
pub mod composite;
pub mod density;
pub mod gpu_density;
pub mod heatmap;
pub mod line;
pub mod scatter;
pub mod violin;

pub use area::*;
pub use bar::*;
pub use boxplot::*;
pub use choropleth::*;
pub use composite::*;
pub use density::*;
pub use gpu_density::*;
pub use heatmap::*;
pub use line::*;
pub use scatter::*;
pub use violin::*;

use super::ChartBuilderError;
use super::accessor::{AccessorValue, FieldAccessor};
use crate::error::GupResult;
use crate::grid::{Color, GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::boxplot::{BoxPlotAttributes, BoxPlotInstance, BoxPlotOrientation};
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

    /// Set a custom tick label formatter for the X-axis (bottom / top).
    ///
    /// Accepts any type implementing [`LabelFormatter`], e.g.
    /// [`PercentFormatter`](crate::label::PercentFormatter),
    /// [`NumericFormatter`](crate::label::NumericFormatter), or
    /// [`DateTimeFormatter`](crate::label::DateTimeFormatter).
    fn x_tick_format(self, formatter: impl LabelFormatter) -> Self;

    /// Set a custom tick label formatter for the Y-axis (left / right).
    ///
    /// Accepts any type implementing [`LabelFormatter`], e.g.
    /// [`PercentFormatter`](crate::label::PercentFormatter),
    /// [`NumericFormatter`](crate::label::NumericFormatter), or
    /// [`DateTimeFormatter`](crate::label::DateTimeFormatter).
    fn y_tick_format(self, formatter: impl LabelFormatter) -> Self;
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

/// NDC bounds for the chart plotting area.
///
/// All four values are in normalised device coordinates (−1 … +1).
#[derive(Debug, Clone, Copy)]
pub struct NdcBounds {
    /// Left edge of the chart area in NDC.
    pub left: f32,
    /// Right edge of the chart area in NDC.
    pub right: f32,
    /// Top edge of the chart area in NDC (positive Y is up).
    pub top: f32,
    /// Bottom edge of the chart area in NDC (positive Y is up).
    pub bottom: f32,
}

/// Utility function to apply accessor functions to a selection.
///
/// When `x_scale` / `y_scale` are set in `config`, uses
/// [`AxisScale::scale_value`] to map each accessor value through the
/// scale, then converts the scale's output range to NDC using `ndc`.
/// Otherwise the data domain is auto-computed and linearly mapped to NDC.
///
/// Colour and size accessors are wired through directly; when absent,
/// sensible defaults (steel-blue fill, radius 0.012 NDC) are used.
pub fn apply_accessors_to_selection<T, M>(
    selection: &mut Selection<T, M>,
    x_accessor: Option<AccessorFunction<T>>,
    y_accessor: Option<AccessorFunction<T>>,
    color_accessor: Option<AccessorFunction<T>>,
    size_accessor: Option<AccessorFunction<T>>,
    config: &crate::chart_builder::ChartConfig,
    ndc: NdcBounds,
) -> GupResult<()>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
    M::AttributeValue: Default + Clone,
{
    // ── Position mapping ────────────────────────────────────────────────
    if let (Some(x_acc), Some(y_acc)) = (x_accessor, y_accessor) {
        let x_acc = std::sync::Arc::new(x_acc);
        let y_acc = std::sync::Arc::new(y_acc);

        match (&config.x_scale, &config.y_scale) {
            // ── Scales present: map through scale_value → range → NDC ──
            (Some(xs), Some(ys)) => {
                let xs = xs.clone();
                let ys = ys.clone();
                let x_rng_lo = xs.range_min();
                let x_rng_hi = xs.range_max();
                let y_rng_lo = ys.range_min();
                let y_rng_hi = ys.range_max();

                selection.attr("center", move |data: &T| {
                    let x_val = x_acc.apply(data).as_f32();
                    let y_val = y_acc.apply(data).as_f32();

                    let x_scaled = xs.scale_value(x_val);
                    let y_scaled = ys.scale_value(y_val);

                    let x_span = x_rng_hi - x_rng_lo;
                    let y_span = y_rng_hi - y_rng_lo;

                    let tx = if x_span.abs() < f32::EPSILON {
                        0.5
                    } else {
                        (x_scaled - x_rng_lo) / x_span
                    };
                    let ty = if y_span.abs() < f32::EPSILON {
                        0.5
                    } else {
                        (y_scaled - y_rng_lo) / y_span
                    };

                    let ndc_x = ndc.left + tx * (ndc.right - ndc.left);
                    let ndc_y = ndc.bottom + ty * (ndc.top - ndc.bottom);

                    [ndc_x, ndc_y]
                });
            }

            // ── No scales: auto-compute domain from data ────────────────
            _ => {
                let (x_min, x_max) = config
                    .x_scale
                    .as_ref()
                    .map(|s| (s.domain_min(), s.domain_max()))
                    .unwrap_or_else(|| auto_domain(selection.data(), &x_acc));
                let (y_min, y_max) = config
                    .y_scale
                    .as_ref()
                    .map(|s| (s.domain_min(), s.domain_max()))
                    .unwrap_or_else(|| auto_domain(selection.data(), &y_acc));

                selection.attr("center", move |data: &T| {
                    let x_val = x_acc.apply(data).as_f32();
                    let y_val = y_acc.apply(data).as_f32();

                    let x_span = x_max - x_min;
                    let y_span = y_max - y_min;

                    let tx = if x_span.abs() < f32::EPSILON {
                        0.5
                    } else {
                        (x_val - x_min) / x_span
                    };
                    let ty = if y_span.abs() < f32::EPSILON {
                        0.5
                    } else {
                        (y_val - y_min) / y_span
                    };

                    let ndc_x = ndc.left + tx * (ndc.right - ndc.left);
                    let ndc_y = ndc.bottom + ty * (ndc.top - ndc.bottom);

                    [ndc_x, ndc_y]
                });
            }
        }
    }

    // ── Colour mapping ──────────────────────────────────────────────────
    if let Some(color_acc) = color_accessor {
        let color_acc = std::sync::Arc::new(color_acc);
        selection.attr("color", move |data: &T| color_acc.apply(data).as_color());
    } else {
        // Default: steel-blue
        selection.attr("color", |_: &T| [0.27f32, 0.51, 0.71, 1.0]);
    }

    // ── Size / radius mapping ───────────────────────────────────────────
    if let Some(size_acc) = size_accessor {
        let size_acc = std::sync::Arc::new(size_acc);
        // Scale the accessor value into NDC units.
        let ndc_width = ndc.right - ndc.left;
        selection.attr("radius", move |data: &T| {
            size_acc.apply(data).as_f32() * ndc_width * 0.01
        });
    } else {
        // Default radius: ~1.2 % of the chart width in NDC.
        let default_radius = (ndc.right - ndc.left) * 0.012;
        selection.attr("radius", move |_: &T| default_radius);
    }

    Ok(())
}

/// Compute the (min, max) of accessor values across `data`, with a small
/// padding so points do not sit exactly on the axes.
fn auto_domain<T>(data: &[T], accessor: &AccessorFunction<T>) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for item in data {
        let v = accessor.apply(item).as_f32();
        lo = lo.min(v);
        hi = hi.max(v);
    }
    // Add 5 % padding so marks are not clipped by the axes.
    let span = hi - lo;
    if span.abs() < f32::EPSILON {
        // All values identical — give a ±1 range.
        (lo - 1.0, hi + 1.0)
    } else {
        let pad = span * 0.05;
        (lo - pad, hi + pad)
    }
}

/// Linearly map a value from one range to another.
///
/// Given `px` in the range `[rng_lo, rng_hi]`, returns the corresponding
/// value in `[ndc_lo, ndc_hi]`.  When the input span is near-zero the
/// midpoint of the output range is returned.
pub(crate) fn range_to_ndc(px: f32, rng_lo: f32, rng_hi: f32, ndc_lo: f32, ndc_hi: f32) -> f32 {
    let rng_span = rng_hi - rng_lo;
    if rng_span.abs() < f32::EPSILON {
        return (ndc_lo + ndc_hi) / 2.0;
    }
    ndc_lo + (px - rng_lo) / rng_span * (ndc_hi - ndc_lo)
}

/// Build a mapper that transforms [`BoxPlotAttributes`] from data space into
/// NDC-space [`BoxPlotInstance`] values.
///
/// The returned closure computes per-attribute position, statistical values
/// (whisker_min … whisker_max), width, and outlier coordinates in normalised
/// device coordinates.  Both [`BoxPlotBuilder`](boxplot::BoxPlotBuilder) and
/// [`ViolinPlotBuilder`](violin::ViolinPlotBuilder) delegate to this helper
/// so the mapping logic is defined in exactly one place.
///
/// The data domain is derived from `attrs` with 5 % padding on each axis so
/// marks are not clipped at the chart edges.
pub fn boxplot_ndc_mapper(
    attrs: &[BoxPlotAttributes],
    ndc: NdcBounds,
) -> impl Fn(&BoxPlotAttributes) -> BoxPlotInstance + use<> {
    // ── Determine data domain from all attributes ────────────────────────
    let (mut x_min, mut x_max) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut y_min, mut y_max) = (f32::INFINITY, f32::NEG_INFINITY);
    for a in attrs {
        let half_w = a.width * 0.5;
        match a.orientation {
            BoxPlotOrientation::Vertical => {
                x_min = x_min.min(a.position.x - half_w);
                x_max = x_max.max(a.position.x + half_w);
                y_min = y_min.min(a.min);
                y_max = y_max.max(a.max);
                for &o in &a.outliers {
                    y_min = y_min.min(o);
                    y_max = y_max.max(o);
                }
            }
            BoxPlotOrientation::Horizontal => {
                y_min = y_min.min(a.position.y - half_w);
                y_max = y_max.max(a.position.y + half_w);
                x_min = x_min.min(a.min);
                x_max = x_max.max(a.max);
                for &o in &a.outliers {
                    x_min = x_min.min(o);
                    x_max = x_max.max(o);
                }
            }
        }
    }
    // Add 5 % padding so marks are not clipped at the edges.
    let x_pad = (x_max - x_min).abs() * 0.05;
    let y_pad = (y_max - y_min).abs() * 0.05;
    x_min -= x_pad;
    x_max += x_pad;
    y_min -= y_pad;
    y_max += y_pad;

    let x_span = x_max - x_min;
    let y_span = y_max - y_min;

    // ── Return a closure that maps one set of attributes to NDC ──────────
    move |attrs: &BoxPlotAttributes| {
        let map_x = |v: f32| {
            let t = if x_span.abs() < f32::EPSILON {
                0.5
            } else {
                (v - x_min) / x_span
            };
            ndc.left + t * (ndc.right - ndc.left)
        };
        let map_y = |v: f32| {
            let t = if y_span.abs() < f32::EPSILON {
                0.5
            } else {
                (v - y_min) / y_span
            };
            ndc.bottom + t * (ndc.top - ndc.bottom)
        };

        let mut inst = BoxPlotInstance::from(attrs);
        match attrs.orientation {
            BoxPlotOrientation::Vertical => {
                inst.position = [map_x(attrs.position.x), 0.0];
                inst.whisker_min = map_y(attrs.min);
                inst.q1 = map_y(attrs.q1);
                inst.median = map_y(attrs.median);
                inst.q3 = map_y(attrs.q3);
                inst.whisker_max = map_y(attrs.max);
                inst.width = (attrs.width / x_span) * (ndc.right - ndc.left);
            }
            BoxPlotOrientation::Horizontal => {
                inst.position = [0.0, map_y(attrs.position.y)];
                inst.whisker_min = map_x(attrs.min);
                inst.q1 = map_x(attrs.q1);
                inst.median = map_x(attrs.median);
                inst.q3 = map_x(attrs.q3);
                inst.whisker_max = map_x(attrs.max);
                inst.width = (attrs.width / y_span) * (ndc.top - ndc.bottom);
            }
        }
        // Transform outlier values to NDC.
        for i in 0..attrs.outliers.len().min(32) {
            let vec_idx = i / 4;
            let comp_idx = i % 4;
            let val = attrs.outliers[i];
            inst.outliers[vec_idx][comp_idx] = match attrs.orientation {
                BoxPlotOrientation::Vertical => map_y(val),
                BoxPlotOrientation::Horizontal => map_x(val),
            };
        }
        inst
    }
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
