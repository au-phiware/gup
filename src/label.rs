// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label formatting and positioning system for professional data visualization.
//!
//! This module provides comprehensive label formatting that supports various number
//! formats, locale-aware formatting, and intelligent positioning with collision avoidance.
//! It integrates seamlessly with the axis system and chart builders to provide
//! professional-quality labels that enhance data interpretation.
//!
//! # Features
//!
//! * **Comprehensive Formatting** - Numeric, currency, percentage, scientific, and date/time
//! * **Locale Support** - Respects system locale for number and date formatting
//! * **Intelligent Positioning** - Automatic collision detection and avoidance
//! * **Performance** - Optimized for hundreds of labels without performance degradation
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::label::{LabelFormatter, NumericFormatter, LabelPositioner, AxisInfo};
//! use gup::text::{TextStyle, TextLayoutEngine};
//!
//! # fn example() -> gup::error::GupResult<()> {
//! // Create a currency formatter
//! let formatter = NumericFormatter::currency("USD", 2)?;
//!
//! // Format values
//! let formatted = formatter.format_value(1234.56);
//! assert_eq!(formatted, "$1,234.56");
//!
//! // Create label positioner for axis labels
//! let mut positioner = LabelPositioner::new();
//! let tick_positions = vec![0.0, 0.25, 0.5, 0.75, 1.0];
//! let axis_info = AxisInfo::horizontal(800.0);
//!
//! let layout = positioner.layout_labels(
//!     &tick_positions,
//!     &axis_info,
//!     &formatter,
//!     &Default::default(),
//! )?;
//! # Ok(())
//! # }
//! ```

pub mod formatter;
pub mod locale;
pub mod positioner;

pub use formatter::*;
pub use locale::*;
pub use positioner::*;

use crate::axis::{AxisBounds, AxisPosition};
use crate::shader_function::Vec2;
use crate::text::{TextAnchor, TextBounds, TextStyle};
use crate::{MaybeSend, MaybeSync};

/// Core trait for label formatters.
///
/// Label formatters convert raw numeric values into human-readable strings
/// that follow established conventions for different data types and locales.
pub trait LabelFormatter: MaybeSend + MaybeSync + std::fmt::Debug + 'static {
    /// Format a numeric value for display.
    fn format_value(&self, value: f64) -> String;

    /// Get preferred label spacing in pixels.
    fn preferred_spacing(&self) -> f32 {
        60.0 // Default spacing
    }

    /// Estimate label width for layout calculations.
    fn estimate_width(&self, value: f64) -> f32 {
        // Simple estimation based on character count and average width
        let formatted = self.format_value(value);
        formatted.len() as f32 * 8.0 // 8px per character average
    }

    /// Get the maximum expected width for optimization.
    fn max_width(&self) -> f32 {
        120.0 // Default maximum width
    }

    /// Check if this formatter supports the given data range.
    fn supports_range(&self, min_value: f64, max_value: f64) -> bool {
        let _ = (min_value, max_value);
        true // Most formatters support any range
    }
}

/// Information about axis configuration for label positioning.
#[derive(Debug, Clone)]
pub struct AxisInfo {
    /// Axis position (top, bottom, left, right)
    pub position: AxisPosition,
    /// Axis start point
    pub start: Vec2,
    /// Axis end point
    pub end: Vec2,
    /// Available space for labels
    pub available_space: f32,
    /// Axis length in pixels
    pub length: f32,
}

impl AxisInfo {
    /// Create horizontal axis info.
    pub fn horizontal(length: f32) -> Self {
        Self {
            position: AxisPosition::Bottom,
            start: Vec2 { x: 0.0, y: 0.0 },
            end: Vec2 { x: length, y: 0.0 },
            available_space: 40.0,
            length,
        }
    }

    /// Create vertical axis info.
    pub fn vertical(length: f32) -> Self {
        Self {
            position: AxisPosition::Left,
            start: Vec2 { x: 0.0, y: length },
            end: Vec2 { x: 0.0, y: 0.0 },
            available_space: 60.0,
            length,
        }
    }

    /// Create from axis bounds.
    pub fn from_bounds(bounds: &AxisBounds, position: AxisPosition) -> Self {
        let length = bounds.length();
        Self {
            position,
            start: bounds.start,
            end: bounds.end,
            available_space: bounds.available_margin,
            length,
        }
    }

    /// Check if this is a horizontal axis.
    pub fn is_horizontal(&self) -> bool {
        self.position.is_horizontal()
    }

    /// Check if this is a vertical axis.
    pub fn is_vertical(&self) -> bool {
        self.position.is_vertical()
    }

    /// Get the axis direction vector.
    pub fn direction(&self) -> Vec2 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let length = (dx * dx + dy * dy).sqrt();
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

/// Result of label layout operation.
#[derive(Debug, Clone)]
pub struct LabelLayout {
    /// Final positions for each label
    pub positions: Vec<LabelPosition>,
    /// Labels that were hidden due to space constraints
    pub hidden_labels: Vec<usize>,
    /// Total space required for labels
    pub margin_requirements: Margins,
    /// Whether any labels were rotated
    pub rotated: bool,
}

/// Position and styling for a single label.
#[derive(Debug, Clone)]
pub struct LabelPosition {
    /// Position in screen coordinates
    pub position: Vec2,
    /// Rotation angle in radians
    pub rotation: f32,
    /// Text anchor point
    pub anchor: TextAnchor,
    /// Formatted text
    pub text: String,
    /// Text styling
    pub style: TextStyle,
    /// Bounding box for collision detection
    pub bounds: TextBounds,
}

/// Margin requirements for labels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    /// Top margin in pixels.
    pub top: f32,
    /// Right margin in pixels.
    pub right: f32,
    /// Bottom margin in pixels.
    pub bottom: f32,
    /// Left margin in pixels.
    pub left: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }
}

impl Margins {
    /// Create uniform margins.
    pub fn uniform(margin: f32) -> Self {
        Self {
            top: margin,
            right: margin,
            bottom: margin,
            left: margin,
        }
    }

    /// Add margins together.
    pub fn add(&mut self, other: &Margins) {
        self.top += other.top;
        self.right += other.right;
        self.bottom += other.bottom;
        self.left += other.left;
    }

    /// Get maximum margin value.
    pub fn max(&self) -> f32 {
        self.top.max(self.right).max(self.bottom).max(self.left)
    }
}

/// Constraints for label layout.
#[derive(Debug, Clone)]
pub struct LabelConstraints {
    /// Maximum allowed rotation angle (radians)
    pub max_rotation: f32,
    /// Whether rotation is allowed
    pub allow_rotation: bool,
    /// Minimum spacing between labels
    pub min_spacing: f32,
    /// Maximum number of labels to show
    pub max_labels: Option<usize>,
    /// Preferred text style
    pub text_style: TextStyle,
    /// Whether to hide overlapping labels
    pub hide_overlapping: bool,
    /// Priority order for label hiding (higher values kept)
    pub label_priorities: Option<Vec<f32>>,
}

impl Default for LabelConstraints {
    fn default() -> Self {
        Self {
            max_rotation: std::f32::consts::PI / 4.0, // 45 degrees
            allow_rotation: false,
            min_spacing: 8.0,
            max_labels: None,
            text_style: TextStyle::axis_label(),
            hide_overlapping: true,
            label_priorities: None,
        }
    }
}

impl LabelConstraints {
    /// Create constraints for axis labels.
    pub fn axis_labels() -> Self {
        Self {
            allow_rotation: true,
            max_rotation: std::f32::consts::PI / 2.0, // 90 degrees
            min_spacing: 4.0,
            text_style: TextStyle::axis_label(),
            ..Default::default()
        }
    }

    /// Create constraints for dense label placement.
    pub fn dense() -> Self {
        Self {
            allow_rotation: true,
            max_rotation: std::f32::consts::PI / 4.0, // 45 degrees
            min_spacing: 2.0,
            max_labels: Some(20),
            hide_overlapping: true,
            ..Default::default()
        }
    }

    /// Create constraints that allow overlapping.
    pub fn allow_overlap() -> Self {
        Self {
            hide_overlapping: false,
            allow_rotation: false,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_info_creation() {
        let horizontal = AxisInfo::horizontal(800.0);
        assert!(horizontal.is_horizontal());
        assert!(!horizontal.is_vertical());
        assert_eq!(horizontal.length, 800.0);

        let vertical = AxisInfo::vertical(600.0);
        assert!(vertical.is_vertical());
        assert!(!vertical.is_horizontal());
        assert_eq!(vertical.length, 600.0);
    }

    #[test]
    fn test_axis_info_direction() {
        let horizontal = AxisInfo::horizontal(100.0);
        let direction = horizontal.direction();
        assert!((direction.x - 1.0).abs() < 0.001);
        assert!((direction.y - 0.0).abs() < 0.001);

        let normal = horizontal.normal();
        assert!((normal.x - 0.0).abs() < 0.001);
        assert!((normal.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_margins_operations() {
        let mut margins1 = Margins::uniform(10.0);
        let margins2 = Margins {
            top: 5.0,
            right: 5.0,
            bottom: 5.0,
            left: 5.0,
        };

        margins1.add(&margins2);
        assert_eq!(margins1.top, 15.0);
        assert_eq!(margins1.max(), 15.0);
    }

    #[test]
    fn test_label_constraints_presets() {
        let axis_constraints = LabelConstraints::axis_labels();
        assert!(axis_constraints.allow_rotation);
        assert_eq!(axis_constraints.max_rotation, std::f32::consts::PI / 2.0);

        let dense_constraints = LabelConstraints::dense();
        assert!(dense_constraints.hide_overlapping);
        assert_eq!(dense_constraints.max_labels, Some(20));

        let overlap_constraints = LabelConstraints::allow_overlap();
        assert!(!overlap_constraints.hide_overlapping);
        assert!(!overlap_constraints.allow_rotation);
    }
}
