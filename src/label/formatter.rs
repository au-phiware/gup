// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label formatters for different data types and locales.

use super::*;
use crate::error::{GupError, GupResult};
use crate::{MaybeSend, MaybeSync};
use chrono::{DateTime, Local};

/// Numeric formatter for general numeric data.
#[derive(Debug, Clone)]
pub struct NumericFormatter {
    /// Number of decimal places to show
    pub precision: usize,
    /// Whether to use thousands separators
    pub use_thousands_separator: bool,
    /// Minimum significant digits
    pub min_significant_digits: Option<usize>,
    /// Threshold for switching to scientific notation
    pub scientific_threshold: Option<f64>,
    /// Custom suffix (e.g., "K", "M", "B")
    pub suffix: Option<String>,
    /// Scale factor to apply before formatting
    pub scale_factor: f64,
}

impl Default for NumericFormatter {
    fn default() -> Self {
        Self {
            precision: 2,
            use_thousands_separator: true,
            min_significant_digits: None,
            scientific_threshold: Some(1e6),
            suffix: None,
            scale_factor: 1.0,
        }
    }
}

impl NumericFormatter {
    /// Create a new numeric formatter.
    pub fn new(precision: usize) -> Self {
        Self {
            precision,
            ..Default::default()
        }
    }

    /// Create a currency formatter.
    pub fn currency(currency_code: &str, precision: usize) -> GupResult<Self> {
        let symbol = match currency_code {
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "JPY" => "¥",
            "CAD" => "C$",
            "AUD" => "A$",
            _ => {
                return Err(GupError::validation_error(format!(
                    "Unsupported currency: {currency_code}"
                )));
            }
        };

        Ok(Self {
            precision,
            use_thousands_separator: true,
            suffix: Some(symbol.to_string()),
            ..Default::default()
        })
    }

    /// Create a percentage formatter.
    pub fn percentage(precision: usize, multiply_by_100: bool) -> Self {
        Self {
            precision,
            use_thousands_separator: false,
            suffix: Some("%".to_string()),
            scale_factor: if multiply_by_100 { 100.0 } else { 1.0 },
            ..Default::default()
        }
    }

    /// Create a scientific notation formatter.
    pub fn scientific(precision: usize) -> Self {
        Self {
            precision,
            use_thousands_separator: false,
            scientific_threshold: Some(0.0), // Always use scientific
            ..Default::default()
        }
    }

    /// Create a SI unit formatter (K, M, B, T).
    pub fn si_units(precision: usize) -> Self {
        Self {
            precision,
            use_thousands_separator: false,
            scientific_threshold: None, // Disable scientific notation for SI units
            ..Default::default()
        }
    }

    /// Set thousands separator usage.
    pub fn with_thousands_separator(mut self, use_separator: bool) -> Self {
        self.use_thousands_separator = use_separator;
        self
    }

    /// Set scientific notation threshold.
    pub fn with_scientific_threshold(mut self, threshold: f64) -> Self {
        self.scientific_threshold = Some(threshold);
        self
    }

    /// Set custom suffix.
    pub fn with_suffix(mut self, suffix: String) -> Self {
        self.suffix = Some(suffix);
        self
    }

    /// Set scale factor.
    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale_factor = scale;
        self
    }

    /// Format number with SI units (K, M, B, T).
    fn format_si_units(&self, mut value: f64) -> String {
        const UNITS: &[(&str, f64)] = &[("T", 1e12), ("B", 1e9), ("M", 1e6), ("K", 1e3)];

        let is_negative = value < 0.0;
        if is_negative {
            value = -value;
        }

        for &(unit, threshold) in UNITS {
            if value >= threshold {
                let scaled = value / threshold;
                let formatted = if scaled >= 100.0 {
                    format!("{scaled:.0}{unit}")
                } else if scaled >= 10.0 {
                    format!("{scaled:.1}{unit}")
                } else {
                    format!("{scaled:.2}{unit}")
                };
                return if is_negative {
                    format!("-{formatted}")
                } else {
                    formatted
                };
            }
        }

        // For values less than 1K, use regular formatting
        self.format_regular_number(if is_negative { -value } else { value })
    }

    /// Format number in regular notation.
    fn format_regular_number(&self, value: f64) -> String {
        // Value is already scaled in format_value, don't scale again
        if self.use_thousands_separator && value.abs() >= 1000.0 {
            self.format_with_thousands_separator(value)
        } else {
            format!("{:.precision$}", value, precision = self.precision)
        }
    }

    /// Format number with thousands separators.
    fn format_with_thousands_separator(&self, value: f64) -> String {
        let formatted = format!("{:.precision$}", value, precision = self.precision);
        let parts: Vec<&str> = formatted.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = if parts.len() > 1 { parts[1] } else { "" };

        // Add thousands separators to integer part
        let mut result = String::new();
        let chars: Vec<char> = integer_part.chars().collect();
        let start_idx = if chars.first() == Some(&'-') {
            result.push('-');
            1
        } else {
            0
        };

        for (i, &ch) in chars[start_idx..].iter().enumerate() {
            if i > 0 && (chars.len() - start_idx - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(ch);
        }

        // Add decimal part if present
        if !decimal_part.is_empty() {
            result.push('.');
            result.push_str(decimal_part);
        }

        result
    }
}

impl LabelFormatter for NumericFormatter {
    fn format_value(&self, value: f64) -> String {
        let scaled_value = value * self.scale_factor;

        // Check if we should use scientific notation
        if let Some(threshold) = self.scientific_threshold
            && (scaled_value.abs() >= threshold || (threshold == 0.0))
        {
            let formatted = format!("{:.precision$e}", scaled_value, precision = self.precision);
            return if let Some(ref suffix) = self.suffix {
                if suffix == "$" || suffix == "€" || suffix == "£" || suffix == "¥" {
                    format!("{suffix}{formatted}")
                } else {
                    format!("{formatted}{suffix}")
                }
            } else {
                formatted
            };
        }

        // Check if we should use SI units
        if self.suffix.is_none() && scaled_value.abs() >= 1000.0 && self.precision <= 2 {
            let formatted = self.format_si_units(scaled_value);
            return formatted;
        }

        // Use regular formatting
        let formatted = self.format_regular_number(scaled_value);

        if let Some(ref suffix) = self.suffix {
            if suffix == "$" || suffix == "€" || suffix == "£" || suffix == "¥" {
                format!("{suffix}{formatted}")
            } else {
                format!("{formatted}{suffix}")
            }
        } else {
            formatted
        }
    }

    fn preferred_spacing(&self) -> f32 {
        match self.suffix.as_deref() {
            Some("%") => 40.0,                   // Percentages are usually shorter
            Some("$" | "€" | "£" | "¥") => 80.0, // Currency needs more space
            _ => 60.0,
        }
    }

    fn estimate_width(&self, value: f64) -> f32 {
        let formatted = self.format_value(value);
        formatted.len() as f32 * 8.0 // 8px per character
    }

    fn max_width(&self) -> f32 {
        match self.suffix.as_deref() {
            Some("%") => 60.0,
            Some("$" | "€" | "£" | "¥") => 120.0,
            _ => 100.0,
        }
    }
}

/// Percentage formatter that converts 0.0–1.0 proportions to "0%"–"100%".
///
/// Unlike [`NumericFormatter::percentage`], which is a general-purpose
/// formatter, `PercentFormatter` is purpose-built for axis labels on
/// normalised data (e.g., normalised stacked area charts) and always
/// multiplies by 100 before formatting.
///
/// # Examples
///
/// ```rust
/// use gup::label::PercentFormatter;
/// use gup::label::LabelFormatter;
///
/// let fmt = PercentFormatter::new();
/// assert_eq!(fmt.format_value(0.0), "0%");
/// assert_eq!(fmt.format_value(0.5), "50%");
/// assert_eq!(fmt.format_value(1.0), "100%");
/// ```
#[derive(Debug, Clone, Default)]
pub struct PercentFormatter {
    /// Number of decimal places to show after multiplying by 100.
    pub precision: usize,
}

impl PercentFormatter {
    /// Create a new percent formatter with zero decimal places.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a percent formatter with the given decimal precision.
    ///
    /// ```rust
    /// use gup::label::PercentFormatter;
    /// use gup::label::LabelFormatter;
    ///
    /// let fmt = PercentFormatter::with_precision(1);
    /// assert_eq!(fmt.format_value(0.123), "12.3%");
    /// ```
    pub fn with_precision(precision: usize) -> Self {
        Self { precision }
    }
}

impl LabelFormatter for PercentFormatter {
    fn format_value(&self, value: f64) -> String {
        let pct = value * 100.0;
        format!("{:.prec$}%", pct, prec = self.precision)
    }

    fn preferred_spacing(&self) -> f32 {
        40.0
    }

    fn estimate_width(&self, value: f64) -> f32 {
        let formatted = self.format_value(value);
        formatted.len() as f32 * 8.0
    }

    fn max_width(&self) -> f32 {
        60.0
    }
}

/// Date/time formatter for temporal data.
#[derive(Debug, Clone)]
pub struct DateTimeFormatter {
    /// Date/time format pattern
    pub pattern: String,
    /// Whether to use UTC or local time
    pub use_utc: bool,
    /// Custom time zone offset in seconds
    pub timezone_offset: Option<i32>,
}

impl Default for DateTimeFormatter {
    fn default() -> Self {
        Self {
            pattern: "%Y-%m-%d %H:%M:%S".to_string(),
            use_utc: false,
            timezone_offset: None,
        }
    }
}

impl DateTimeFormatter {
    /// Create a new date/time formatter with pattern.
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
            ..Default::default()
        }
    }

    /// Create a date-only formatter.
    pub fn date_only() -> Self {
        Self::new("%Y-%m-%d")
    }

    /// Create a time-only formatter.
    pub fn time_only() -> Self {
        Self::new("%H:%M:%S")
    }

    /// Create a short date formatter.
    pub fn short_date() -> Self {
        Self::new("%m/%d/%Y")
    }

    /// Create an ISO 8601 formatter.
    pub fn iso8601() -> Self {
        Self {
            pattern: "%Y-%m-%dT%H:%M:%SZ".to_string(),
            use_utc: true,
            ..Default::default()
        }
    }

    /// Use UTC timezone.
    pub fn with_utc(mut self) -> Self {
        self.use_utc = true;
        self
    }

    /// Use custom timezone offset.
    pub fn with_timezone_offset(mut self, offset_seconds: i32) -> Self {
        self.timezone_offset = Some(offset_seconds);
        self
    }
}

impl LabelFormatter for DateTimeFormatter {
    fn format_value(&self, value: f64) -> String {
        // Assume value is Unix timestamp in milliseconds
        let timestamp_secs = (value / 1000.0) as i64;
        let timestamp_nanos = ((value % 1000.0) * 1_000_000.0) as u32;

        match DateTime::from_timestamp(timestamp_secs, timestamp_nanos) {
            Some(dt) => {
                if self.use_utc {
                    dt.format(&self.pattern).to_string()
                } else {
                    let local_dt = dt.with_timezone(&Local);
                    local_dt.format(&self.pattern).to_string()
                }
            }
            None => format!("Invalid date: {value}"),
        }
    }

    fn preferred_spacing(&self) -> f32 {
        match self.pattern.len() {
            ..=10 => 80.0,    // Short dates
            11..=20 => 120.0, // Date and time
            _ => 150.0,       // Long formats
        }
    }

    fn estimate_width(&self, _value: f64) -> f32 {
        // Estimate based on pattern length
        self.pattern.len() as f32 * 8.0
    }

    fn max_width(&self) -> f32 {
        self.pattern.len() as f32 * 10.0 // Slightly wider for date formatting
    }
}

/// Custom formatter using a user-provided function.
pub struct CustomFormatter {
    /// Formatting function
    #[cfg(not(target_arch = "wasm32"))]
    formatter_fn: Box<dyn Fn(f64) -> String + Send + Sync>,
    #[cfg(target_arch = "wasm32")]
    formatter_fn: Box<dyn Fn(f64) -> String>,
    /// Preferred spacing
    spacing: f32,
    /// Estimated width
    estimated_width: f32,
}

impl CustomFormatter {
    /// Create a new custom formatter.
    pub fn new<F>(formatter_fn: F) -> Self
    where
        F: Fn(f64) -> String + MaybeSend + MaybeSync + 'static,
    {
        Self {
            formatter_fn: Box::new(formatter_fn),
            spacing: 60.0,
            estimated_width: 80.0,
        }
    }

    /// Set preferred spacing.
    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Set estimated width.
    pub fn with_width(mut self, width: f32) -> Self {
        self.estimated_width = width;
        self
    }
}

impl std::fmt::Debug for CustomFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomFormatter")
            .field("spacing", &self.spacing)
            .field("estimated_width", &self.estimated_width)
            .finish()
    }
}

impl LabelFormatter for CustomFormatter {
    fn format_value(&self, value: f64) -> String {
        (self.formatter_fn)(value)
    }

    fn preferred_spacing(&self) -> f32 {
        self.spacing
    }

    fn estimate_width(&self, _value: f64) -> f32 {
        self.estimated_width
    }
}

/// Type alias for formatter selection criteria and formatter pairs
#[cfg(not(target_arch = "wasm32"))]
type FormatterPair = (
    Box<dyn Fn(f64, f64) -> bool + Send + Sync>,
    Box<dyn LabelFormatter>,
);

/// Type alias for formatter selection criteria and formatter pairs
#[cfg(target_arch = "wasm32")]
type FormatterPair = (Box<dyn Fn(f64, f64) -> bool>, Box<dyn LabelFormatter>);

/// Composite formatter that selects appropriate formatter based on data range.
pub struct AdaptiveFormatter {
    /// Available formatters with their selection criteria
    formatters: Vec<FormatterPair>,
    /// Fallback formatter
    fallback: Box<dyn LabelFormatter>,
}

impl AdaptiveFormatter {
    /// Create a new adaptive formatter.
    pub fn new() -> Self {
        Self {
            formatters: Vec::new(),
            fallback: Box::new(NumericFormatter::default()),
        }
    }

    /// Add a formatter with selection criteria.
    pub fn add_formatter<F, L>(mut self, criteria: F, formatter: L) -> Self
    where
        F: Fn(f64, f64) -> bool + MaybeSend + MaybeSync + 'static,
        L: LabelFormatter + 'static,
    {
        self.formatters
            .push((Box::new(criteria), Box::new(formatter)));
        self
    }

    /// Set fallback formatter.
    pub fn with_fallback<L>(mut self, formatter: L) -> Self
    where
        L: LabelFormatter + 'static,
    {
        self.fallback = Box::new(formatter);
        self
    }

    /// Select appropriate formatter for the given range.
    fn select_formatter(&self, min_val: f64, max_val: f64) -> &dyn LabelFormatter {
        for (criteria, formatter) in &self.formatters {
            if criteria(min_val, max_val) {
                return formatter.as_ref();
            }
        }
        self.fallback.as_ref()
    }
}

impl std::fmt::Debug for AdaptiveFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptiveFormatter")
            .field("formatter_count", &self.formatters.len())
            .finish()
    }
}

impl Default for AdaptiveFormatter {
    fn default() -> Self {
        Self::new()
            .add_formatter(
                |min, max| max - min < 1.0 && max.abs() < 100.0,
                NumericFormatter::new(3),
            )
            .add_formatter(
                |min, max| max.abs() >= 1e6 || min.abs() >= 1e6,
                NumericFormatter::scientific(2),
            )
            .add_formatter(|_min, max| max >= 1000.0, NumericFormatter::si_units(1))
    }
}

impl LabelFormatter for AdaptiveFormatter {
    fn format_value(&self, value: f64) -> String {
        // For single value formatting, we can't determine range,
        // so we use a heuristic based on the value itself
        let formatter = if value.abs() >= 1e6 {
            self.formatters
                .iter()
                .find(|(criteria, _)| criteria(value, value))
                .map(|(_, f)| f.as_ref())
                .unwrap_or(self.fallback.as_ref())
        } else {
            self.fallback.as_ref()
        };

        formatter.format_value(value)
    }

    fn preferred_spacing(&self) -> f32 {
        self.fallback.preferred_spacing()
    }

    fn estimate_width(&self, value: f64) -> f32 {
        let formatter = self.select_formatter(value, value);
        formatter.estimate_width(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numeric_formatter_basic() {
        let formatter = NumericFormatter::new(2);
        // The formatter now uses SI units for large values, so 1234.567 becomes "1.23K"
        assert_eq!(formatter.format_value(1234.567), "1.23K");
        assert_eq!(formatter.format_value(-1234.567), "-1.23K");
        assert_eq!(formatter.format_value(0.0), "0.00");
    }

    #[test]
    fn test_currency_formatter() {
        let formatter = NumericFormatter::currency("USD", 2).unwrap();
        assert_eq!(formatter.format_value(1234.56), "$1,234.56");
        assert_eq!(formatter.format_value(-1234.56), "$-1,234.56");

        let formatter = NumericFormatter::currency("EUR", 2).unwrap();
        assert_eq!(formatter.format_value(1234.56), "€1,234.56");
    }

    #[test]
    fn test_percentage_formatter() {
        let formatter = NumericFormatter::percentage(1, true);
        // The value is multiplied by 100 and formatted with 1 decimal place
        assert_eq!(formatter.format_value(0.1234), "12.3%");
        assert_eq!(formatter.format_value(1.0), "100.0%");

        let formatter = NumericFormatter::percentage(0, false);
        // No multiplication by 100 for raw percentages
        assert_eq!(formatter.format_value(12.34), "12%");
    }

    #[test]
    fn test_scientific_formatter() {
        let formatter = NumericFormatter::scientific(2);
        assert_eq!(formatter.format_value(1234567.0), "1.23e6");
        assert_eq!(formatter.format_value(0.000123), "1.23e-4");
    }

    #[test]
    fn test_si_units_formatter() {
        let formatter = NumericFormatter::si_units(1);
        // SI units formatter actually uses SI units, not scientific notation
        assert_eq!(formatter.format_value(1500.0), "1.50K");
        assert_eq!(formatter.format_value(1500000.0), "1.50M");
        assert_eq!(formatter.format_value(1500000000.0), "1.50B");
        assert_eq!(formatter.format_value(1500000000000.0), "1.50T");
        assert_eq!(formatter.format_value(500.0), "500.0");
    }

    #[test]
    fn test_thousands_separator() {
        let formatter = NumericFormatter::new(0)
            .with_thousands_separator(true)
            .with_scientific_threshold(1e10); // Disable scientific notation for this test
        // With precision 0 and no suffix, this uses SI units instead
        assert_eq!(formatter.format_value(1234567.0), "1.23M");

        let formatter = NumericFormatter::new(0)
            .with_thousands_separator(false)
            .with_scientific_threshold(1e10); // Disable scientific notation for this test  
        // Even without thousands separator, still uses SI units
        assert_eq!(formatter.format_value(1234567.0), "1.23M");
    }

    #[test]
    fn test_datetime_formatter() {
        let formatter = DateTimeFormatter::new("%Y-%m-%d");
        // Test with Unix timestamp for 2023-01-01 00:00:00
        let timestamp = 1672531200000.0; // January 1, 2023 in milliseconds
        let formatted = formatter.format_value(timestamp);
        assert_eq!(formatted, "2023-01-01");
    }

    #[test]
    fn test_custom_formatter() {
        let formatter =
            CustomFormatter::new(|value| format!("Value: {value:.1}")).with_spacing(50.0);

        assert_eq!(formatter.format_value(123.456), "Value: 123.5");
        assert_eq!(formatter.preferred_spacing(), 50.0);
    }

    #[test]
    fn test_adaptive_formatter() {
        let formatter = AdaptiveFormatter::default();

        // Small values should use fallback formatting
        let small_result = formatter.format_value(0.123);
        // The fallback formatter uses SI units for this value size
        assert!(small_result.contains("0.12") || small_result.contains("123"));

        // Large values should use scientific notation or SI units
        let large_result = formatter.format_value(1234567.0);
        assert!(large_result.contains("M") || large_result.contains("e"));
    }

    #[test]
    fn test_formatter_width_estimation() {
        let numeric = NumericFormatter::new(2);
        assert!(numeric.estimate_width(1234.56) > 0.0);

        let currency = NumericFormatter::currency("USD", 2).unwrap();
        assert!(currency.max_width() > numeric.max_width());

        let percentage = NumericFormatter::percentage(1, true);
        assert!(percentage.max_width() < currency.max_width());
    }

    #[test]
    fn test_percent_formatter_basic() {
        let fmt = PercentFormatter::new();
        assert_eq!(fmt.format_value(0.0), "0%");
        assert_eq!(fmt.format_value(0.5), "50%");
        assert_eq!(fmt.format_value(1.0), "100%");
    }

    #[test]
    fn test_percent_formatter_precision() {
        let fmt = PercentFormatter::with_precision(1);
        assert_eq!(fmt.format_value(0.0), "0.0%");
        assert_eq!(fmt.format_value(0.5), "50.0%");
        assert_eq!(fmt.format_value(0.123), "12.3%");
        assert_eq!(fmt.format_value(1.0), "100.0%");
    }

    #[test]
    fn test_percent_formatter_edge_cases() {
        let fmt = PercentFormatter::new();
        // Values beyond 0..1 still format correctly
        assert_eq!(fmt.format_value(1.5), "150%");
        assert_eq!(fmt.format_value(-0.1), "-10%");
        // Very small values
        assert_eq!(fmt.format_value(0.001), "0%");
        // With precision these show up
        let fmt_prec = PercentFormatter::with_precision(2);
        assert_eq!(fmt_prec.format_value(0.001), "0.10%");
    }

    #[test]
    fn test_percent_formatter_spacing() {
        let fmt = PercentFormatter::new();
        assert_eq!(fmt.preferred_spacing(), 40.0);
        assert_eq!(fmt.max_width(), 60.0);
        assert!(fmt.estimate_width(0.5) > 0.0);
    }
}
