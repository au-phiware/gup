// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label formatting extension for chart builders.

use crate::chart_builder::{ChartBuilder, ComposedChart};
use crate::error::GupResult;
use crate::label::{DateTimeFormatter, LabelConstraints, LabelFormatter, NumericFormatter};
use crate::text::TextStyle;
use crate::{MaybeSend, MaybeSync};

/// Extension trait for chart builders that adds label formatting capabilities.
pub trait LabelCapableBuilder<T>: ChartBuilder<T> {
    /// Set number format for axis labels.
    fn number_format(self, formatter: Box<dyn LabelFormatter>) -> Self;

    /// Set date format pattern for time-based axes.
    fn date_format(self, pattern: &str) -> Self;

    /// Set currency formatting with currency code.
    fn currency_format(self, currency_code: &str, precision: usize) -> GupResult<Self>
    where
        Self: Sized;

    /// Set percentage formatting.
    fn percentage_format(self, precision: usize, multiply_by_100: bool) -> Self;

    /// Set scientific notation formatting.
    fn scientific_format(self, precision: usize) -> Self;

    /// Set SI units formatting (K, M, B, T).
    fn si_units_format(self, precision: usize) -> Self;

    /// Allow label rotation for space constraints.
    fn allow_label_rotation(self, allow: bool) -> Self;

    /// Set maximum rotation angle in degrees.
    fn max_rotation_degrees(self, degrees: f32) -> Self;

    /// Set label text style.
    fn label_style(self, style: TextStyle) -> Self;

    /// Custom label formatter function.
    fn custom_labels<F>(self, formatter: F) -> Self
    where
        F: Fn(f64) -> String + MaybeSend + MaybeSync + 'static,
        Self: Sized;

    /// Set minimum spacing between labels.
    fn label_spacing(self, spacing: f32) -> Self;

    /// Set maximum number of labels.
    fn max_labels(self, max: usize) -> Self;

    /// Hide overlapping labels instead of adjusting positions.
    fn hide_overlapping_labels(self, hide: bool) -> Self;
}

/// Configuration for axis label formatting.
#[derive(Debug)]
pub struct AxisLabelConfig {
    /// X-axis label formatter
    pub x_formatter: Option<Box<dyn LabelFormatter>>,
    /// Y-axis label formatter  
    pub y_formatter: Option<Box<dyn LabelFormatter>>,
    /// X-axis label constraints
    pub x_constraints: LabelConstraints,
    /// Y-axis label constraints
    pub y_constraints: LabelConstraints,
}

impl Default for AxisLabelConfig {
    fn default() -> Self {
        Self {
            x_formatter: None,
            y_formatter: None,
            x_constraints: LabelConstraints::axis_labels(),
            y_constraints: LabelConstraints::axis_labels(),
        }
    }
}

impl AxisLabelConfig {
    /// Create new axis label configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set X-axis formatter.
    pub fn with_x_formatter(mut self, formatter: Box<dyn LabelFormatter>) -> Self {
        self.x_formatter = Some(formatter);
        self
    }

    /// Set Y-axis formatter.
    pub fn with_y_formatter(mut self, formatter: Box<dyn LabelFormatter>) -> Self {
        self.y_formatter = Some(formatter);
        self
    }

    /// Set X-axis constraints.
    pub fn with_x_constraints(mut self, constraints: LabelConstraints) -> Self {
        self.x_constraints = constraints;
        self
    }

    /// Set Y-axis constraints.
    pub fn with_y_constraints(mut self, constraints: LabelConstraints) -> Self {
        self.y_constraints = constraints;
        self
    }

    /// Allow rotation for both axes.
    pub fn allow_rotation(mut self) -> Self {
        self.x_constraints.allow_rotation = true;
        self.y_constraints.allow_rotation = true;
        self
    }

    /// Set dense label constraints for both axes.
    pub fn dense_labels(mut self) -> Self {
        self.x_constraints = LabelConstraints::dense();
        self.y_constraints = LabelConstraints::dense();
        self
    }
}

/// Enhanced composed chart with label formatting support.
#[derive(Debug)]
pub struct LabeledChart<T, M>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// Base composed chart
    pub chart: ComposedChart<T, M>,
    /// Label configuration
    pub label_config: AxisLabelConfig,
}

impl<T, M> LabeledChart<T, M>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// Create a new labeled chart.
    pub fn new(chart: ComposedChart<T, M>) -> Self {
        Self {
            chart,
            label_config: AxisLabelConfig::default(),
        }
    }

    /// Set label configuration.
    pub fn with_labels(mut self, config: AxisLabelConfig) -> Self {
        self.label_config = config;
        self
    }

    /// Set X-axis number formatting.
    pub fn x_number_format(mut self, precision: usize) -> Self {
        self.label_config.x_formatter = Some(Box::new(NumericFormatter::new(precision)));
        self
    }

    /// Set Y-axis number formatting.
    pub fn y_number_format(mut self, precision: usize) -> Self {
        self.label_config.y_formatter = Some(Box::new(NumericFormatter::new(precision)));
        self
    }

    /// Set X-axis currency formatting.
    pub fn x_currency_format(mut self, currency_code: &str, precision: usize) -> GupResult<Self> {
        let formatter = NumericFormatter::currency(currency_code, precision)?;
        self.label_config.x_formatter = Some(Box::new(formatter));
        Ok(self)
    }

    /// Set Y-axis currency formatting.
    pub fn y_currency_format(mut self, currency_code: &str, precision: usize) -> GupResult<Self> {
        let formatter = NumericFormatter::currency(currency_code, precision)?;
        self.label_config.y_formatter = Some(Box::new(formatter));
        Ok(self)
    }

    /// Set X-axis date formatting.
    pub fn x_date_format(mut self, pattern: &str) -> Self {
        self.label_config.x_formatter = Some(Box::new(DateTimeFormatter::new(pattern)));
        self
    }

    /// Set Y-axis date formatting.
    pub fn y_date_format(mut self, pattern: &str) -> Self {
        self.label_config.y_formatter = Some(Box::new(DateTimeFormatter::new(pattern)));
        self
    }

    /// Allow label rotation for space constraints.
    pub fn allow_label_rotation(mut self) -> Self {
        self.label_config = self.label_config.allow_rotation();
        self
    }

    /// Use dense label layout.
    pub fn dense_labels(mut self) -> Self {
        self.label_config = self.label_config.dense_labels();
        self
    }

    /// Get the underlying chart.
    pub fn chart(&self) -> &ComposedChart<T, M> {
        &self.chart
    }

    /// Get the chart mutably.
    pub fn chart_mut(&mut self) -> &mut ComposedChart<T, M> {
        &mut self.chart
    }

    /// Get label configuration.
    pub fn label_config(&self) -> &AxisLabelConfig {
        &self.label_config
    }

    /// Render the labeled chart (placeholder implementation).
    pub fn render(&mut self, context: &mut crate::RenderContext) -> GupResult<()> {
        // In a complete implementation, this would:
        // 1. Calculate axis scales from the data
        // 2. Generate tick positions using the tick generator
        // 3. Format labels using the configured formatters
        // 4. Position labels using collision detection
        // 5. Render the chart with properly formatted labels

        // For now, delegate to the base chart
        self.chart.render(context)
    }
}

// Helper functions for common formatting scenarios

/// Create a currency formatter for the given currency code.
pub fn currency(currency_code: &str, precision: usize) -> GupResult<Box<dyn LabelFormatter>> {
    Ok(Box::new(NumericFormatter::currency(
        currency_code,
        precision,
    )?))
}

/// Create a percentage formatter.
pub fn percentage(precision: usize, multiply_by_100: bool) -> Box<dyn LabelFormatter> {
    Box::new(NumericFormatter::percentage(precision, multiply_by_100))
}

/// Create a scientific notation formatter.
pub fn scientific(precision: usize) -> Box<dyn LabelFormatter> {
    Box::new(NumericFormatter::scientific(precision))
}

/// Create a SI units formatter.
pub fn si_units(precision: usize) -> Box<dyn LabelFormatter> {
    Box::new(NumericFormatter::si_units(precision))
}

/// Create a date formatter.
pub fn date_format(pattern: &str) -> Box<dyn LabelFormatter> {
    Box::new(DateTimeFormatter::new(pattern))
}

/// Create a custom formatter.
pub fn custom_format<F>(formatter: F) -> Box<dyn LabelFormatter>
where
    F: Fn(f64) -> String + MaybeSend + MaybeSync + 'static,
{
    Box::new(crate::label::formatter::CustomFormatter::new(formatter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_label_config_creation() {
        let config = AxisLabelConfig::new().allow_rotation().dense_labels();

        assert!(config.x_constraints.allow_rotation);
        assert!(config.y_constraints.allow_rotation);
        assert_eq!(config.x_constraints.max_labels, Some(20));
    }

    #[test]
    fn test_helper_formatters() {
        let currency_formatter = currency("USD", 2).unwrap();
        assert_eq!(currency_formatter.format_value(1234.56), "$1,234.56");

        let percentage_formatter = percentage(1, true);
        assert_eq!(percentage_formatter.format_value(0.1234), "12.3%");

        let scientific_formatter = scientific(2);
        assert!(scientific_formatter.format_value(1234567.0).contains("e"));

        let si_formatter = si_units(1);
        assert_eq!(si_formatter.format_value(1500.0), "1.50K");

        let date_formatter = date_format("%Y-%m-%d");
        assert!(
            date_formatter
                .format_value(1672531200000.0)
                .contains("2023")
        );
    }

    #[test]
    fn test_custom_formatter() {
        let custom = custom_format(|value| format!("Value: {value:.1}"));
        assert_eq!(custom.format_value(123.456), "Value: 123.5");
    }

    #[tokio::test]
    async fn test_labeled_chart_creation() {
        // This test would require a proper chart setup, which is complex
        // For now, we'll just test the type relationships
        let config = AxisLabelConfig::new().allow_rotation();
        assert!(config.x_constraints.allow_rotation);
        assert!(config.y_constraints.allow_rotation);
    }
}
