// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integrated scale and axis system for automatic professional data visualization.
//!
//! This module provides the core scale system that integrates seamlessly with the
//! axis, tick generation, grid, and label systems to enable automatic, professional-quality
//! visualizations without manual configuration.
//!
//! # Key Features
//!
//! * **Automatic Scale Detection** - Analyzes data to select appropriate scale types
//! * **GPU Shader Integration** - Scales compile to WGSL shader functions
//! * **Axis System Coordination** - Seamless integration with tick generation and layout
//! * **Performance Optimized** - <2ms complete axis rendering for typical charts
//!

use crate::error::GupResult;
use crate::label::{LabelFormatter, formatter::NumericFormatter};
use crate::tick_generator::{
    LinearScale, LinearTickGenerator, LogarithmicScale, LogarithmicTickGenerator,
    Scale as TickScale, TickGenerator, TimeScale, TimeTickGenerator,
};
use crate::{MaybeSend, MaybeSync};
use std::fmt::Debug;

/// Identifier for different axes in a chart system.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum AxisId {
    /// The horizontal axis.
    XAxis,
    /// The vertical axis.
    YAxis,
    /// The colour-encoding axis.
    ColorAxis,
    /// The size-encoding axis.
    SizeAxis,
}

/// Data value types for scale analysis.
#[derive(Debug, Clone)]
pub enum DataValue {
    /// A numeric value.
    Numeric(f64),
    /// A temporal value as a Unix timestamp in milliseconds.
    Temporal(i64),
    /// A categorical (string) value.
    Categorical(String),
}

/// Characteristics of a data field for scale selection.
#[derive(Debug, Clone)]
pub struct DataCharacteristics {
    /// The type of data (numeric, temporal, or categorical).
    pub data_type: DataType,
    /// The minimum observed value, if available.
    pub min_value: Option<f64>,
    /// The maximum observed value, if available.
    pub max_value: Option<f64>,
    /// Distribution characteristics of the data.
    pub distribution: Distribution,
    /// Time range information, present for temporal data.
    pub temporal_range: Option<TemporalRange>,
    /// Distinct category labels, present for categorical data.
    pub categories: Option<Vec<String>>,
    /// The scale type recommended for this data.
    pub recommended_scale: ScaleType,
    /// The number of data samples analysed.
    pub sample_count: usize,
    /// Whether the data contains zero.
    pub has_zero: bool,
    /// Whether the data contains negative values.
    pub has_negative: bool,
}

/// Types of data for scale selection.
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    /// Continuous numeric data.
    Numeric,
    /// Time-based data.
    Temporal,
    /// Discrete categorical data.
    Categorical,
}

/// Distribution characteristics of numeric data.
#[derive(Debug, Clone)]
pub struct Distribution {
    /// Whether the data appears to follow a logarithmic distribution.
    pub is_logarithmic: bool,
    /// The number of orders of magnitude the data spans.
    pub span_orders_of_magnitude: f64,
    /// An estimate of data density.
    pub density_estimate: f64,
}

/// Time range information for temporal data.
#[derive(Debug, Clone)]
pub struct TemporalRange {
    /// The start of the range as a Unix timestamp.
    pub start_timestamp: i64,
    /// The end of the range as a Unix timestamp.
    pub end_timestamp: i64,
    /// The duration of the range in milliseconds.
    pub duration_ms: i64,
}

/// Recommended scale types with configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleType {
    /// A linear scale.
    Linear {
        /// Whether to round the domain to nice values.
        nice_domain: bool,
    },
    /// A logarithmic scale.
    Logarithmic {
        /// The logarithm base.
        base: f64,
    },
    /// A temporal scale.
    Temporal {
        /// The time unit for tick generation.
        unit: TimeUnit,
    },
    /// An ordinal (categorical) scale.
    Ordinal {
        /// The ordered category labels.
        categories: Vec<String>,
    },
    /// A band scale for bar-chart-style positioning.
    Band {
        /// The ordered category labels.
        categories: Vec<String>,
        /// Inner padding between bands as a fraction.
        padding: f32,
    },
}

/// Time units for temporal scales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    /// Millisecond resolution.
    Millisecond,
    /// Second resolution.
    Second,
    /// Minute resolution.
    Minute,
    /// Hour resolution.
    Hour,
    /// Day resolution.
    Day,
    /// Week resolution.
    Week,
    /// Month resolution.
    Month,
    /// Year resolution.
    Year,
}

/// Enhanced scale trait that integrates with the complete axis system.
pub trait Scale: TickScale + MaybeSend + MaybeSync + Debug + 'static {
    /// Map data value to coordinate space (0.0 to 1.0)
    fn scale_value(&self, value: f64) -> f64;

    /// Inverse mapping from coordinate to data value
    fn invert_value(&self, coordinate: f64) -> f64;

    /// Get domain (input range) of this scale
    fn domain(&self) -> (f64, f64);

    /// Set domain, returns new scale instance
    fn with_domain(self, domain: (f64, f64)) -> Self
    where
        Self: Sized;

    /// Get range (output range) of this scale
    fn range(&self) -> (f32, f32);

    /// Set range, returns new scale instance
    fn with_range(self, range: (f32, f32)) -> Self
    where
        Self: Sized;

    /// Get WGSL shader code for GPU processing
    fn generate_wgsl(&self) -> String;

    /// Generate appropriate tick positions for this scale
    fn generate_ticks(&self, target_count: Option<usize>) -> Vec<f64>;

    /// Get appropriate label formatter for this scale type
    fn default_formatter(&self) -> Box<dyn LabelFormatter>;

    /// Get the scale type identifier
    fn scale_type(&self) -> &'static str;

    /// Check if this scale supports the given data range
    fn supports_range(&self, min_value: f64, max_value: f64) -> bool;
}

/// Linear scale with enhanced integration capabilities.
#[derive(Debug, Clone)]
pub struct IntegratedLinearScale {
    inner: LinearScale,
    range: (f32, f32),
    #[allow(dead_code)]
    nice: bool,
}

impl IntegratedLinearScale {
    /// Create a new integrated linear scale.
    pub fn new(domain_min: f64, domain_max: f64) -> Self {
        Self {
            inner: LinearScale::new(domain_min, domain_max),
            range: (0.0, 1.0),
            nice: true,
        }
    }

    /// Create with custom range.
    pub fn with_range(domain_min: f64, domain_max: f64, range: (f32, f32)) -> Self {
        Self {
            inner: LinearScale::new(domain_min, domain_max),
            range,
            nice: true,
        }
    }
}

impl TickScale for IntegratedLinearScale {
    fn domain_min(&self) -> f64 {
        self.inner.domain_min()
    }

    fn domain_max(&self) -> f64 {
        self.inner.domain_max()
    }

    fn normalize(&self, value: f64) -> f64 {
        self.inner.normalize(value)
    }

    fn denormalize(&self, position: f64) -> f64 {
        self.inner.denormalize(position)
    }
}

impl Scale for IntegratedLinearScale {
    fn scale_value(&self, value: f64) -> f64 {
        self.normalize(value)
    }

    fn invert_value(&self, coordinate: f64) -> f64 {
        self.denormalize(coordinate)
    }

    fn domain(&self) -> (f64, f64) {
        (self.domain_min(), self.domain_max())
    }

    fn with_domain(mut self, domain: (f64, f64)) -> Self {
        self.inner = LinearScale::new(domain.0, domain.1);
        self
    }

    fn range(&self) -> (f32, f32) {
        self.range
    }

    fn with_range(mut self, range: (f32, f32)) -> Self {
        self.range = range;
        self
    }

    fn generate_wgsl(&self) -> String {
        format!(
            r#"
fn linear_scale(value: f32) -> f32 {{
    let domain_min = {:.6}f;
    let domain_max = {:.6}f;
    let range_min = {:.6}f;
    let range_max = {:.6}f;

    let normalized = (value - domain_min) / (domain_max - domain_min);
    return range_min + normalized * (range_max - range_min);
}}
"#,
            self.domain_min() as f32,
            self.domain_max() as f32,
            self.range.0,
            self.range.1
        )
    }

    fn generate_ticks(&self, target_count: Option<usize>) -> Vec<f64> {
        let generator = LinearTickGenerator::default();
        generator.generate_major_ticks(self, 800.0, target_count)
    }

    fn default_formatter(&self) -> Box<dyn LabelFormatter> {
        Box::new(NumericFormatter::default())
    }

    fn scale_type(&self) -> &'static str {
        "linear"
    }

    fn supports_range(&self, min_value: f64, max_value: f64) -> bool {
        min_value.is_finite() && max_value.is_finite() && min_value < max_value
    }
}

// Note: Shader function structs removed in favor of direct WGSL generation

/// Factory for creating scales from data characteristics.
#[derive(Debug)]
pub struct ScaleFactory;

impl ScaleFactory {
    /// Create a new scale factory.
    pub fn new() -> Self {
        Self
    }

    /// Create a scale from data characteristics.
    pub fn create_scale_from_characteristics(
        &self,
        characteristics: &DataCharacteristics,
    ) -> GupResult<Box<dyn Scale>> {
        match &characteristics.recommended_scale {
            ScaleType::Linear { nice_domain: _ } => {
                let min_val = characteristics.min_value.unwrap_or(0.0);
                let max_val = characteristics.max_value.unwrap_or(1.0);

                // Apply nice rounding if requested
                let (domain_min, domain_max) = if characteristics.recommended_scale
                    == (ScaleType::Linear { nice_domain: true })
                {
                    self.nice_domain(min_val, max_val)
                } else {
                    (min_val, max_val)
                };

                Ok(Box::new(IntegratedLinearScale::new(domain_min, domain_max)))
            }
            ScaleType::Logarithmic { base } => {
                let min_val = characteristics.min_value.unwrap_or(1.0).max(1e-10);
                let max_val = characteristics.max_value.unwrap_or(10.0);

                Ok(Box::new(IntegratedLogarithmicScale::new(
                    min_val, max_val, *base,
                )))
            }
            ScaleType::Temporal { unit: _ } => {
                let min_val = characteristics.min_value.unwrap_or(0.0);
                let max_val = characteristics.max_value.unwrap_or(1.0);

                Ok(Box::new(IntegratedTimeScale::new(min_val, max_val)))
            }
            ScaleType::Ordinal { categories } => {
                Ok(Box::new(IntegratedOrdinalScale::new(categories.clone())))
            }
            ScaleType::Band {
                categories,
                padding,
            } => Ok(Box::new(IntegratedOrdinalScale::new_with_padding(
                categories.clone(),
                *padding,
            ))),
        }
    }

    /// Apply nice domain rounding to numeric ranges.
    fn nice_domain(&self, min_val: f64, max_val: f64) -> (f64, f64) {
        if min_val >= max_val {
            return (min_val, max_val);
        }

        let range = max_val - min_val;
        let magnitude = 10f64.powf(range.log10().floor());
        let normalized_range = range / magnitude;

        let nice_range = if normalized_range <= 1.0 {
            1.0
        } else if normalized_range <= 2.0 {
            2.0
        } else if normalized_range <= 5.0 {
            5.0
        } else {
            10.0
        } * magnitude;

        let nice_min = (min_val / magnitude).floor() * magnitude;
        let nice_max = nice_min + nice_range;

        (nice_min, nice_max)
    }
}

impl Default for ScaleFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Integrated logarithmic scale implementation.
#[derive(Debug, Clone)]
pub struct IntegratedLogarithmicScale {
    inner: LogarithmicScale,
    range: (f32, f32),
}

impl IntegratedLogarithmicScale {
    /// Creates a new logarithmic scale with the given domain and base.
    pub fn new(domain_min: f64, domain_max: f64, base: f64) -> Self {
        Self {
            inner: LogarithmicScale::new(domain_min, domain_max, base),
            range: (0.0, 1.0),
        }
    }
}

impl TickScale for IntegratedLogarithmicScale {
    fn domain_min(&self) -> f64 {
        self.inner.domain_min()
    }

    fn domain_max(&self) -> f64 {
        self.inner.domain_max()
    }

    fn normalize(&self, value: f64) -> f64 {
        self.inner.normalize(value)
    }

    fn denormalize(&self, position: f64) -> f64 {
        self.inner.denormalize(position)
    }

    fn is_logarithmic(&self) -> bool {
        true
    }
}

impl Scale for IntegratedLogarithmicScale {
    fn scale_value(&self, value: f64) -> f64 {
        self.normalize(value)
    }

    fn invert_value(&self, coordinate: f64) -> f64 {
        self.denormalize(coordinate)
    }

    fn domain(&self) -> (f64, f64) {
        (self.domain_min(), self.domain_max())
    }

    fn with_domain(mut self, domain: (f64, f64)) -> Self {
        self.inner = LogarithmicScale::new(domain.0, domain.1, 10.0);
        self
    }

    fn range(&self) -> (f32, f32) {
        self.range
    }

    fn with_range(mut self, range: (f32, f32)) -> Self {
        self.range = range;
        self
    }

    fn generate_wgsl(&self) -> String {
        format!(
            r#"
fn logarithmic_scale(value: f32) -> f32 {{
    let domain_min = {:.6}f;
    let domain_max = {:.6}f;
    let range_min = {:.6}f;
    let range_max = {:.6}f;

    let log_value = log(value);
    let log_min = log(domain_min);
    let log_max = log(domain_max);

    let normalized = (log_value - log_min) / (log_max - log_min);
    return range_min + normalized * (range_max - range_min);
}}
"#,
            self.domain_min() as f32,
            self.domain_max() as f32,
            self.range.0,
            self.range.1
        )
    }

    fn generate_ticks(&self, target_count: Option<usize>) -> Vec<f64> {
        let generator = LogarithmicTickGenerator::default();
        generator.generate_major_ticks(self, 800.0, target_count)
    }

    fn default_formatter(&self) -> Box<dyn LabelFormatter> {
        Box::new(NumericFormatter::default())
    }

    fn scale_type(&self) -> &'static str {
        "logarithmic"
    }

    fn supports_range(&self, min_value: f64, max_value: f64) -> bool {
        min_value > 0.0 && max_value > 0.0 && min_value < max_value
    }
}

/// Integrated time scale implementation.
#[derive(Debug, Clone)]
pub struct IntegratedTimeScale {
    inner: TimeScale,
    range: (f32, f32),
}

impl IntegratedTimeScale {
    /// Creates a new time scale with the given domain.
    pub fn new(domain_min: f64, domain_max: f64) -> Self {
        Self {
            inner: TimeScale::new(domain_min, domain_max),
            range: (0.0, 1.0),
        }
    }
}

impl TickScale for IntegratedTimeScale {
    fn domain_min(&self) -> f64 {
        self.inner.domain_min()
    }

    fn domain_max(&self) -> f64 {
        self.inner.domain_max()
    }

    fn normalize(&self, value: f64) -> f64 {
        self.inner.normalize(value)
    }

    fn denormalize(&self, position: f64) -> f64 {
        self.inner.denormalize(position)
    }

    fn is_time(&self) -> bool {
        true
    }
}

impl Scale for IntegratedTimeScale {
    fn scale_value(&self, value: f64) -> f64 {
        self.normalize(value)
    }

    fn invert_value(&self, coordinate: f64) -> f64 {
        self.denormalize(coordinate)
    }

    fn domain(&self) -> (f64, f64) {
        (self.domain_min(), self.domain_max())
    }

    fn with_domain(mut self, domain: (f64, f64)) -> Self {
        self.inner = TimeScale::new(domain.0, domain.1);
        self
    }

    fn range(&self) -> (f32, f32) {
        self.range
    }

    fn with_range(mut self, range: (f32, f32)) -> Self {
        self.range = range;
        self
    }

    fn generate_wgsl(&self) -> String {
        format!(
            r#"
fn temporal_scale(value: f32) -> f32 {{
    let domain_min = {:.6}f;
    let domain_max = {:.6}f;
    let range_min = {:.6}f;
    let range_max = {:.6}f;

    let normalized = (value - domain_min) / (domain_max - domain_min);
    return range_min + normalized * (range_max - range_min);
}}
"#,
            self.domain_min() as f32,
            self.domain_max() as f32,
            self.range.0,
            self.range.1
        )
    }

    fn generate_ticks(&self, target_count: Option<usize>) -> Vec<f64> {
        let generator = TimeTickGenerator::default();
        generator.generate_major_ticks(self, 800.0, target_count)
    }

    fn default_formatter(&self) -> Box<dyn LabelFormatter> {
        Box::new(NumericFormatter::default()) // Placeholder until DateTimeFormatter is available
    }

    fn scale_type(&self) -> &'static str {
        "temporal"
    }

    fn supports_range(&self, min_value: f64, max_value: f64) -> bool {
        min_value.is_finite() && max_value.is_finite() && min_value < max_value
    }
}

/// Integrated ordinal scale implementation.
#[derive(Debug, Clone)]
pub struct IntegratedOrdinalScale {
    categories: Vec<String>,
    range: (f32, f32),
    #[allow(dead_code)]
    padding: f32,
}

impl IntegratedOrdinalScale {
    /// Creates a new ordinal scale with the given categories.
    pub fn new(categories: Vec<String>) -> Self {
        Self {
            categories,
            range: (0.0, 1.0),
            padding: 0.1,
        }
    }

    /// Creates a new ordinal scale with the given categories and padding.
    pub fn new_with_padding(categories: Vec<String>, padding: f32) -> Self {
        Self {
            categories,
            range: (0.0, 1.0),
            padding,
        }
    }

    #[allow(dead_code)]
    fn category_to_position(&self, category: &str) -> f64 {
        if let Some(index) = self.categories.iter().position(|c| c == category) {
            let band_width = 1.0 / self.categories.len() as f64;
            index as f64 * band_width + band_width * 0.5
        } else {
            0.5 // Default to center if category not found
        }
    }
}

impl TickScale for IntegratedOrdinalScale {
    fn domain_min(&self) -> f64 {
        0.0
    }

    fn domain_max(&self) -> f64 {
        self.categories.len() as f64
    }

    fn normalize(&self, value: f64) -> f64 {
        value / self.categories.len() as f64
    }

    fn denormalize(&self, position: f64) -> f64 {
        position * self.categories.len() as f64
    }
}

impl Scale for IntegratedOrdinalScale {
    fn scale_value(&self, value: f64) -> f64 {
        // For ordinal scales, value should be treated as category index
        let index = value as usize;
        if index < self.categories.len() {
            self.normalize(value)
        } else {
            0.5
        }
    }

    fn invert_value(&self, coordinate: f64) -> f64 {
        self.denormalize(coordinate)
    }

    fn domain(&self) -> (f64, f64) {
        (0.0, self.categories.len() as f64)
    }

    fn with_domain(self, _domain: (f64, f64)) -> Self {
        // Domain for ordinal scales is determined by categories
        self
    }

    fn range(&self) -> (f32, f32) {
        self.range
    }

    fn with_range(mut self, range: (f32, f32)) -> Self {
        self.range = range;
        self
    }

    fn generate_wgsl(&self) -> String {
        format!(
            r#"
fn ordinal_scale(value: f32) -> f32 {{
    let category_count = {:.1}f;
    let range_min = {:.6}f;
    let range_max = {:.6}f;

    let normalized = value / category_count;
    return range_min + normalized * (range_max - range_min);
}}
"#,
            self.categories.len() as f32,
            self.range.0,
            self.range.1
        )
    }

    fn generate_ticks(&self, target_count: Option<usize>) -> Vec<f64> {
        let _target_count = target_count; // Available for future use
        (0..self.categories.len()).map(|i| i as f64).collect()
    }

    fn default_formatter(&self) -> Box<dyn LabelFormatter> {
        Box::new(NumericFormatter::default())
    }

    fn scale_type(&self) -> &'static str {
        "ordinal"
    }

    fn supports_range(&self, _min_value: f64, _max_value: f64) -> bool {
        true // Ordinal scales support any range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_factory_linear() {
        let factory = ScaleFactory::new();
        let characteristics = DataCharacteristics {
            data_type: DataType::Numeric,
            min_value: Some(0.0),
            max_value: Some(100.0),
            distribution: Distribution {
                is_logarithmic: false,
                span_orders_of_magnitude: 2.0,
                density_estimate: 1.0,
            },
            temporal_range: None,
            categories: None,
            recommended_scale: ScaleType::Linear { nice_domain: true },
            sample_count: 100,
            has_zero: true,
            has_negative: false,
        };

        let scale = factory
            .create_scale_from_characteristics(&characteristics)
            .unwrap();
        assert_eq!(scale.scale_type(), "linear");
        assert_eq!(scale.domain(), (0.0, 100.0));
    }

    #[test]
    fn test_integrated_linear_scale() {
        let scale = IntegratedLinearScale::new(0.0, 100.0);

        assert_eq!(scale.scale_value(0.0), 0.0);
        assert_eq!(scale.scale_value(50.0), 0.5);
        assert_eq!(scale.scale_value(100.0), 1.0);

        assert_eq!(scale.invert_value(0.0), 0.0);
        assert_eq!(scale.invert_value(0.5), 50.0);
        assert_eq!(scale.invert_value(1.0), 100.0);
    }

    #[test]
    fn test_integrated_logarithmic_scale() {
        let scale = IntegratedLogarithmicScale::new(1.0, 1000.0, 10.0);

        // Log scale should map powers of 10 evenly
        assert!((scale.scale_value(1.0) - 0.0).abs() < 0.001);
        assert!((scale.scale_value(10.0) - 1.0 / 3.0).abs() < 0.001);
        assert!((scale.scale_value(100.0) - 2.0 / 3.0).abs() < 0.001);
        assert!((scale.scale_value(1000.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_integrated_ordinal_scale() {
        let categories = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let scale = IntegratedOrdinalScale::new(categories);

        assert_eq!(scale.domain(), (0.0, 3.0));
        assert_eq!(scale.scale_value(0.0), 0.0);
        assert_eq!(scale.scale_value(1.0), 1.0 / 3.0);
        assert_eq!(scale.scale_value(2.0), 2.0 / 3.0);
    }

    #[test]
    fn test_scale_shader_function_generation() {
        let scale = IntegratedLinearScale::new(0.0, 100.0);
        let wgsl = scale.generate_wgsl();

        assert!(wgsl.contains("linear_scale"));
        assert!(wgsl.contains("domain_min"));
        assert!(wgsl.contains("domain_max"));
    }

    #[test]
    fn test_scale_tick_generation() {
        let scale = IntegratedLinearScale::new(0.0, 100.0);
        let ticks = scale.generate_ticks(Some(5));

        assert!(!ticks.is_empty());
        assert!(ticks.len() <= 15); // Reasonable tick count
        assert!(ticks[0] >= 0.0);
        assert!(ticks[ticks.len() - 1] <= 100.0);
    }

    #[test]
    fn test_scale_default_formatter() {
        let linear_scale = IntegratedLinearScale::new(0.0, 100.0);
        let formatter = linear_scale.default_formatter();
        assert_eq!(formatter.format_value(50.0), "50.00");

        let log_scale = IntegratedLogarithmicScale::new(1.0, 1000.0, 10.0);
        let log_formatter = log_scale.default_formatter();
        assert!(log_formatter.format_value(100.0).contains("1"));
    }

    #[test]
    fn test_nice_domain_calculation() {
        let factory = ScaleFactory::new();

        let (nice_min, nice_max) = factory.nice_domain(23.7, 87.3);
        assert!(nice_min <= 23.7);
        assert!(nice_max >= 87.3);
        assert!(nice_min % 10.0 == 0.0 || nice_min % 5.0 == 0.0); // Should be a "nice" number
    }
}
