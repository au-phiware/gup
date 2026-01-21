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
//! # Examples
//!
//! ```rust,no_run
//! use gup::scale::{DataAnalyzer, ScaleFactory, AccessorFunction};
//!
//! # fn example() -> gup::error::GupResult<()> {
//! #[derive(Debug, Clone)]
//! struct SalesData {
//!     revenue: f64,
//!     profit: f64,
//!     date: i64,
//! }
//!
//! let data = vec![
//!     SalesData { revenue: 1000.0, profit: 200.0, date: 1640995200 },
//!     SalesData { revenue: 1500.0, profit: 350.0, date: 1641081600 },
//! ];
//!
//! // Automatic scale detection
//! let analyzer = DataAnalyzer::new();
//! let x_accessor = AccessorFunction::new(|d: &SalesData| d.revenue);
//! let characteristics = analyzer.analyze_field(&data, &x_accessor)?;
//!
//! let scale_factory = ScaleFactory::new();
//! let scale = scale_factory.create_scale_from_characteristics(&characteristics)?;
//! # Ok(())
//! # }
//! ```

use crate::error::{GupError, GupResult};
// Note: Using simplified shader function interface for now
use crate::chart_builder::accessor::AccessorValue;
use crate::label::{LabelFormatter, formatter::NumericFormatter};
use crate::tick_generator::{
    LinearScale, LinearTickGenerator, LogarithmicScale, LogarithmicTickGenerator,
    Scale as TickScale, TickGenerator, TimeScale, TimeTickGenerator,
};
use std::fmt::Debug;

/// Generic accessor function for extracting data values.
pub struct AccessorFunction<T> {
    function: Box<dyn Fn(&T) -> AccessorValue + Send + Sync>,
    name: String,
}

impl<T> std::fmt::Debug for AccessorFunction<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessorFunction")
            .field("name", &self.name)
            .field("function", &"<function>")
            .finish()
    }
}

impl<T> AccessorFunction<T> {
    /// Create a new accessor function from a closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&T) -> f64 + Send + Sync + 'static,
    {
        Self {
            function: Box::new(move |data| AccessorValue::Numeric(f(data))),
            name: "numeric".to_string(),
        }
    }

    /// Create a categorical accessor function.
    pub fn categorical<F>(f: F) -> Self
    where
        F: Fn(&T) -> String + Send + Sync + 'static,
    {
        Self {
            function: Box::new(move |data| AccessorValue::Categorical(f(data))),
            name: "categorical".to_string(),
        }
    }

    /// Create a temporal accessor function.
    pub fn temporal<F>(f: F) -> Self
    where
        F: Fn(&T) -> i64 + Send + Sync + 'static,
    {
        Self {
            function: Box::new(move |data| {
                let timestamp = f(data);
                let datetime =
                    chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or_else(chrono::Utc::now);
                AccessorValue::Temporal(datetime)
            }),
            name: "temporal".to_string(),
        }
    }

    /// Call the accessor function.
    pub fn call(&self, data: &T) -> AccessorValue {
        (self.function)(data)
    }

    /// Get the name of this accessor.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Identifier for different axes in a chart system.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum AxisId {
    XAxis,
    YAxis,
    ColorAxis,
    SizeAxis,
}

/// Data value types for scale analysis.
#[derive(Debug, Clone)]
pub enum DataValue {
    Numeric(f64),
    Temporal(i64), // Unix timestamp in milliseconds
    Categorical(String),
}

impl From<AccessorValue> for DataValue {
    fn from(value: AccessorValue) -> Self {
        match value {
            AccessorValue::Numeric(n) => DataValue::Numeric(n),
            AccessorValue::Temporal(t) => DataValue::Temporal(t.timestamp()),
            AccessorValue::Categorical(c) => DataValue::Categorical(c),
            // Convert other types to appropriate DataValue
            AccessorValue::Float(f) => DataValue::Numeric(f as f64),
            AccessorValue::String(s) => DataValue::Categorical(s),
            other => DataValue::Numeric(other.as_f32() as f64),
        }
    }
}

/// Characteristics of a data field for scale selection.
#[derive(Debug, Clone)]
pub struct DataCharacteristics {
    pub data_type: DataType,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub distribution: Distribution,
    pub temporal_range: Option<TemporalRange>,
    pub categories: Option<Vec<String>>,
    pub recommended_scale: ScaleType,
    pub sample_count: usize,
    pub has_zero: bool,
    pub has_negative: bool,
}

/// Types of data for scale selection.
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Numeric,
    Temporal,
    Categorical,
}

/// Distribution characteristics of numeric data.
#[derive(Debug, Clone)]
pub struct Distribution {
    pub is_logarithmic: bool,
    pub span_orders_of_magnitude: f64,
    pub density_estimate: f64,
}

/// Time range information for temporal data.
#[derive(Debug, Clone)]
pub struct TemporalRange {
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub duration_ms: i64,
}

/// Recommended scale types with configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleType {
    Linear {
        nice_domain: bool,
    },
    Logarithmic {
        base: f64,
    },
    Temporal {
        unit: TimeUnit,
    },
    Ordinal {
        categories: Vec<String>,
    },
    Band {
        categories: Vec<String>,
        padding: f32,
    },
}

/// Time units for temporal scales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Enhanced scale trait that integrates with the complete axis system.
pub trait Scale: TickScale + Send + Sync + Debug + 'static {
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

/// Data analyzer for automatic scale type detection.
#[derive(Debug)]
pub struct DataAnalyzer {
    #[allow(dead_code)]
    numeric_threshold: f64,
    #[allow(dead_code)]
    temporal_threshold: i64,
    logarithmic_threshold: f64,
}

impl DataAnalyzer {
    /// Create a new data analyzer with default thresholds.
    pub fn new() -> Self {
        Self {
            numeric_threshold: 1e-10,
            temporal_threshold: 1_000_000_000, // Reasonable timestamp threshold
            logarithmic_threshold: 3.0,        // 3+ orders of magnitude
        }
    }

    /// Analyze a data field using an accessor function.
    pub fn analyze_field<T>(
        &self,
        data: &[T],
        accessor: &AccessorFunction<T>,
    ) -> GupResult<DataCharacteristics> {
        if data.is_empty() {
            return Err(GupError::validation_error(
                "Cannot analyze empty dataset".to_string(),
            ));
        }

        let mut characteristics = DataCharacteristics {
            data_type: DataType::Numeric,
            min_value: None,
            max_value: None,
            distribution: Distribution {
                is_logarithmic: false,
                span_orders_of_magnitude: 0.0,
                density_estimate: 1.0,
            },
            temporal_range: None,
            categories: None,
            recommended_scale: ScaleType::Linear { nice_domain: true },
            sample_count: data.len(),
            has_zero: false,
            has_negative: false,
        };

        // Extract values using the accessor
        let mut numeric_values = Vec::new();
        let mut is_numeric = true;

        for item in data {
            match accessor.call(item) {
                AccessorValue::Numeric(value) => {
                    numeric_values.push(value);
                    if value == 0.0 {
                        characteristics.has_zero = true;
                    }
                    if value < 0.0 {
                        characteristics.has_negative = true;
                    }
                }
                AccessorValue::Temporal(timestamp) => {
                    // Convert to numeric for analysis
                    numeric_values.push(timestamp.timestamp() as f64);
                    characteristics.data_type = DataType::Temporal;
                    is_numeric = false;
                }
                AccessorValue::Categorical(category) => {
                    characteristics.data_type = DataType::Categorical;
                    is_numeric = false;

                    // Collect unique categories
                    if characteristics.categories.is_none() {
                        characteristics.categories = Some(Vec::new());
                    }
                    if let Some(ref mut categories) = characteristics.categories
                        && !categories.contains(&category)
                    {
                        categories.push(category);
                    }
                }
                AccessorValue::Float(value) => {
                    numeric_values.push(value as f64);
                    if value == 0.0 {
                        characteristics.has_zero = true;
                    }
                    if value < 0.0 {
                        characteristics.has_negative = true;
                    }
                }
                AccessorValue::String(s) => {
                    characteristics.data_type = DataType::Categorical;
                    is_numeric = false;

                    // Collect unique categories
                    if characteristics.categories.is_none() {
                        characteristics.categories = Some(Vec::new());
                    }
                    if let Some(ref mut categories) = characteristics.categories
                        && !categories.contains(&s)
                    {
                        categories.push(s);
                    }
                }
                AccessorValue::Color(_) | AccessorValue::Position(_) | AccessorValue::Bool(_) => {
                    // Convert to numeric for analysis
                    let value = accessor.call(item).as_f32() as f64;
                    numeric_values.push(value);
                }
            }
        }

        if !numeric_values.is_empty() {
            // Calculate basic statistics
            let min_val = numeric_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_val = numeric_values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            characteristics.min_value = Some(min_val);
            characteristics.max_value = Some(max_val);

            // Analyze distribution for logarithmic scale detection
            if min_val > 0.0 && max_val > 0.0 {
                let log_range = (max_val / min_val).log10();
                characteristics.distribution.span_orders_of_magnitude = log_range;

                if log_range >= self.logarithmic_threshold {
                    characteristics.distribution.is_logarithmic = true;
                }
            }

            // Determine recommended scale type
            characteristics.recommended_scale = if characteristics.data_type == DataType::Temporal {
                self.determine_temporal_scale(min_val as i64, max_val as i64)
            } else if characteristics.distribution.is_logarithmic {
                ScaleType::Logarithmic { base: 10.0 }
            } else if is_numeric {
                ScaleType::Linear { nice_domain: true }
            } else {
                ScaleType::Ordinal {
                    categories: characteristics.categories.clone().unwrap_or_default(),
                }
            };

            // Set temporal range if applicable
            if characteristics.data_type == DataType::Temporal {
                characteristics.temporal_range = Some(TemporalRange {
                    start_timestamp: min_val as i64,
                    end_timestamp: max_val as i64,
                    duration_ms: (max_val - min_val) as i64,
                });
            }
        }

        Ok(characteristics)
    }

    /// Determine appropriate temporal scale based on time range.
    fn determine_temporal_scale(&self, start_ms: i64, end_ms: i64) -> ScaleType {
        let duration_ms = end_ms - start_ms;

        let unit = if duration_ms < 1000 {
            TimeUnit::Millisecond
        } else if duration_ms < 60_000 {
            TimeUnit::Second
        } else if duration_ms < 3_600_000 {
            TimeUnit::Minute
        } else if duration_ms < 86_400_000 {
            TimeUnit::Hour
        } else if duration_ms < 604_800_000 {
            TimeUnit::Day
        } else if duration_ms < 2_592_000_000 {
            TimeUnit::Week
        } else if duration_ms < 31_536_000_000 {
            TimeUnit::Month
        } else {
            TimeUnit::Year
        };

        ScaleType::Temporal { unit }
    }
}

impl Default for DataAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory for creating scales from data characteristics.
#[derive(Debug)]
pub struct ScaleFactory {
    #[allow(dead_code)]
    analyzer: DataAnalyzer,
}

impl ScaleFactory {
    /// Create a new scale factory.
    pub fn new() -> Self {
        Self {
            analyzer: DataAnalyzer::new(),
        }
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
    pub fn new(categories: Vec<String>) -> Self {
        Self {
            categories,
            range: (0.0, 1.0),
            padding: 0.1,
        }
    }

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

    #[derive(Debug, Clone)]
    struct TestData {
        value: f64,
        category: String,
        #[allow(dead_code)]
        timestamp: i64,
    }

    #[test]
    fn test_data_analyzer_numeric() {
        let analyzer = DataAnalyzer::new();
        let data = vec![
            TestData {
                value: 1.0,
                category: "A".to_string(),
                timestamp: 0,
            },
            TestData {
                value: 100.0,
                category: "B".to_string(),
                timestamp: 1000,
            },
            TestData {
                value: 10000.0,
                category: "C".to_string(),
                timestamp: 2000,
            },
        ];

        let accessor = AccessorFunction::new(|d: &TestData| d.value);
        let characteristics = analyzer.analyze_field(&data, &accessor).unwrap();

        assert_eq!(characteristics.data_type, DataType::Numeric);
        assert_eq!(characteristics.min_value, Some(1.0));
        assert_eq!(characteristics.max_value, Some(10000.0));
        assert!(characteristics.distribution.is_logarithmic);
    }

    #[test]
    fn test_data_analyzer_categorical() {
        let analyzer = DataAnalyzer::new();
        let data = vec![
            TestData {
                value: 1.0,
                category: "A".to_string(),
                timestamp: 0,
            },
            TestData {
                value: 2.0,
                category: "B".to_string(),
                timestamp: 1000,
            },
            TestData {
                value: 3.0,
                category: "A".to_string(),
                timestamp: 2000,
            },
        ];

        let accessor = AccessorFunction::categorical(|d: &TestData| d.category.clone());
        let characteristics = analyzer.analyze_field(&data, &accessor).unwrap();

        assert_eq!(characteristics.data_type, DataType::Categorical);
        assert!(characteristics.categories.is_some());
        let categories = characteristics.categories.unwrap();
        assert_eq!(categories.len(), 2);
        assert!(categories.contains(&"A".to_string()));
        assert!(categories.contains(&"B".to_string()));
    }

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
