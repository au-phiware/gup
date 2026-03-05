// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! 3D axis lines, tick marks, and ground-plane grid.
//!
//! [`Axis3D`] generates axis-line [`Line3DInstance`]s along the X, Y, and Z
//! world-space axes with configurable length, colour, and tick marks.
//!
//! [`Grid3D`] generates a ground-plane (XZ) grid as [`Line3DInstance`]s.
//!
//! Both types integrate with the [`Camera`](crate::camera::Camera) uniform
//! from the 3D rendering pipeline — the generated instances are simply fed
//! into the same Line3D storage buffer used by any other Line3D rendering.
//!
//! # Example
//!
//! ```rust
//! use gup::axis3d::{Axis3D, Axis3DConfig, Grid3D, Grid3DConfig};
//! use gup::mark::line3d::Line3DInstance;
//!
//! let axis = Axis3D::new(Axis3DConfig::default());
//! let grid = Grid3D::new(Grid3DConfig::default());
//!
//! let mut instances: Vec<Line3DInstance> = Vec::new();
//! instances.extend(grid.generate_instances());
//! instances.extend(axis.generate_instances());
//! // Upload `instances` to a wgpu storage buffer for rendering.
//! ```

use crate::mark::line3d::Line3DInstance;

// ---------------------------------------------------------------------------
// Tick labels
// ---------------------------------------------------------------------------

/// A positioned text label for a 3D axis tick.
///
/// This struct provides the world-space position and text content for a
/// tick label. It can be fed to a billboard text renderer or projected to
/// screen-space for use with the existing `Text` mark.
#[derive(Debug, Clone)]
pub struct TickLabel3D {
    /// World-space position of the label.
    pub position: [f32; 3],
    /// Label text (e.g. `"0.5"`, `"-1.0"`).
    pub text: String,
    /// Which axis this label belongs to (0 = X, 1 = Y, 2 = Z).
    pub axis_index: u8,
}

// ---------------------------------------------------------------------------
// Axis3D
// ---------------------------------------------------------------------------

/// Colour for a single axis.
#[derive(Debug, Clone, Copy)]
pub struct AxisColor {
    /// RGBA colour for the axis line.
    pub color: [f32; 4],
}

impl Default for AxisColor {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Configuration for [`Axis3D`].
#[derive(Debug, Clone)]
pub struct Axis3DConfig {
    /// Origin point in world space (default: `[0, 0, 0]`).
    pub origin: [f32; 3],
    /// Half-length of each axis from the origin (default: `2.0`).
    pub length: f32,
    /// Line width in clip-space units (default: `0.005`).
    pub line_width: f32,
    /// Colour for the X axis (default: red).
    pub x_color: [f32; 4],
    /// Colour for the Y axis (default: green).
    pub y_color: [f32; 4],
    /// Colour for the Z axis (default: blue).
    pub z_color: [f32; 4],
    /// Number of tick marks per positive half-axis (default: `4`).
    /// Total ticks = 2 × `tick_count` per axis (positive + negative).
    pub tick_count: u32,
    /// Length of each tick mark perpendicular to the axis (default: `0.05`).
    pub tick_size: f32,
    /// Line width for tick marks (default: `0.003`).
    pub tick_width: f32,
    /// Colour for tick marks (default: `[0.6, 0.6, 0.6, 1.0]`).
    pub tick_color: [f32; 4],
}

impl Default for Axis3DConfig {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            length: 2.0,
            line_width: 0.005,
            x_color: [1.0, 0.3, 0.3, 1.0],
            y_color: [0.3, 1.0, 0.3, 1.0],
            z_color: [0.3, 0.3, 1.0, 1.0],
            tick_count: 4,
            tick_size: 0.05,
            tick_width: 0.003,
            tick_color: [0.6, 0.6, 0.6, 1.0],
        }
    }
}

/// 3D axis lines with tick marks.
///
/// Generates [`Line3DInstance`]s for three coloured axis lines (X, Y, Z)
/// plus small perpendicular tick marks at regular intervals.
#[derive(Debug, Clone)]
pub struct Axis3D {
    config: Axis3DConfig,
}

impl Axis3D {
    /// Create a new `Axis3D` with the given configuration.
    pub fn new(config: Axis3DConfig) -> Self {
        Self { config }
    }

    /// Generate [`Line3DInstance`]s for all three axis lines plus tick marks.
    pub fn generate_instances(&self) -> Vec<Line3DInstance> {
        let c = &self.config;
        let o = c.origin;

        // Pre-allocate: 3 axis lines + ticks (2 * tick_count per axis * 3 axes).
        let tick_total = 3 * 2 * c.tick_count as usize;
        let mut out = Vec::with_capacity(3 + tick_total);

        // Axis directions and colours.
        let axes: [([f32; 3], [f32; 4]); 3] = [
            ([1.0, 0.0, 0.0], c.x_color),
            ([0.0, 1.0, 0.0], c.y_color),
            ([0.0, 0.0, 1.0], c.z_color),
        ];

        for (dir, color) in &axes {
            // Main axis line from -length to +length.
            let start = [
                o[0] - dir[0] * c.length,
                o[1] - dir[1] * c.length,
                o[2] - dir[2] * c.length,
            ];
            let end = [
                o[0] + dir[0] * c.length,
                o[1] + dir[1] * c.length,
                o[2] + dir[2] * c.length,
            ];
            out.push(Line3DInstance {
                start,
                width: c.line_width,
                end,
                _pad: 0.0,
                color: *color,
            });

            // Tick marks along the axis.
            if c.tick_count > 0 {
                self.generate_ticks(dir, &mut out);
            }
        }

        out
    }

    /// Generate tick marks for a single axis direction.
    fn generate_ticks(&self, axis_dir: &[f32; 3], out: &mut Vec<Line3DInstance>) {
        let c = &self.config;
        let o = c.origin;

        // Choose two perpendicular directions for cross-shaped ticks.
        let perps = perpendicular_pair(axis_dir);

        let step = c.length / c.tick_count as f32;

        for i in 1..=c.tick_count {
            let t = step * i as f32;
            // Positive and negative sides of the axis.
            for sign in [-1.0_f32, 1.0] {
                let centre = [
                    o[0] + axis_dir[0] * t * sign,
                    o[1] + axis_dir[1] * t * sign,
                    o[2] + axis_dir[2] * t * sign,
                ];

                // One tick line using the first perpendicular.
                let p = &perps[0];
                let start = [
                    centre[0] - p[0] * c.tick_size,
                    centre[1] - p[1] * c.tick_size,
                    centre[2] - p[2] * c.tick_size,
                ];
                let end = [
                    centre[0] + p[0] * c.tick_size,
                    centre[1] + p[1] * c.tick_size,
                    centre[2] + p[2] * c.tick_size,
                ];
                out.push(Line3DInstance {
                    start,
                    width: c.tick_width,
                    end,
                    _pad: 0.0,
                    color: c.tick_color,
                });
            }
        }
    }

    /// Read-only access to the configuration.
    pub fn config(&self) -> &Axis3DConfig {
        &self.config
    }

    /// Generate [`TickLabel3D`]s for all tick positions.
    ///
    /// Each label contains the world-space position and a formatted numeric
    /// string. These can be projected to screen-space and rendered with the
    /// `Text` mark, or displayed as billboard text overlays.
    ///
    /// Returns an empty `Vec` when `tick_count` is zero.
    pub fn generate_tick_labels(&self) -> Vec<TickLabel3D> {
        let c = &self.config;
        let o = c.origin;

        if c.tick_count == 0 {
            return Vec::new();
        }

        let axis_dirs: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let step = c.length / c.tick_count as f32;

        let mut labels = Vec::with_capacity(3 * 2 * c.tick_count as usize);

        for (axis_index, dir) in axis_dirs.iter().enumerate() {
            // Choose a perpendicular offset direction so labels sit beside
            // the tick marks rather than on top of them.
            let perps = perpendicular_pair(dir);
            let offset_dir = perps[0];
            let offset_amount = c.tick_size * 1.5;

            for i in 1..=c.tick_count {
                let t = step * i as f32;
                for sign in [-1.0_f32, 1.0] {
                    let value = t * sign;
                    let position = [
                        o[0] + dir[0] * value + offset_dir[0] * offset_amount,
                        o[1] + dir[1] * value + offset_dir[1] * offset_amount,
                        o[2] + dir[2] * value + offset_dir[2] * offset_amount,
                    ];
                    labels.push(TickLabel3D {
                        position,
                        text: format!("{value:.1}"),
                        axis_index: axis_index as u8,
                    });
                }
            }
        }

        labels
    }
}

// ---------------------------------------------------------------------------
// Grid3D
// ---------------------------------------------------------------------------

/// Configuration for [`Grid3D`].
#[derive(Debug, Clone)]
pub struct Grid3DConfig {
    /// Centre of the grid in world space (default: `[0, 0, 0]`).
    pub origin: [f32; 3],
    /// Half-extent of the grid along X (default: `2.0`).
    pub extent_x: f32,
    /// Half-extent of the grid along Z (default: `2.0`).
    pub extent_z: f32,
    /// Spacing between grid lines (default: `0.5`).
    pub spacing: f32,
    /// Line width in clip-space units (default: `0.002`).
    pub line_width: f32,
    /// Colour for grid lines (default: `[0.35, 0.35, 0.35, 1.0]`).
    pub color: [f32; 4],
    /// Y offset of the ground plane (default: `0.0`).
    pub y_offset: f32,
}

impl Default for Grid3DConfig {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            extent_x: 2.0,
            extent_z: 2.0,
            spacing: 0.5,
            line_width: 0.002,
            color: [0.35, 0.35, 0.35, 1.0],
            y_offset: 0.0,
        }
    }
}

/// Ground-plane (XZ) grid made of [`Line3DInstance`]s.
///
/// The grid lies on the XZ plane at the configured Y offset.
#[derive(Debug, Clone)]
pub struct Grid3D {
    config: Grid3DConfig,
}

impl Grid3D {
    /// Create a new `Grid3D` with the given configuration.
    pub fn new(config: Grid3DConfig) -> Self {
        Self { config }
    }

    /// Generate [`Line3DInstance`]s for the grid lines.
    pub fn generate_instances(&self) -> Vec<Line3DInstance> {
        let c = &self.config;
        let y = c.origin[1] + c.y_offset;

        if c.spacing <= 0.0 {
            return Vec::new();
        }

        // Count lines: lines along X (constant-Z) + lines along Z (constant-X).
        let nx = ((2.0 * c.extent_x) / c.spacing).floor() as usize + 1;
        let nz = ((2.0 * c.extent_z) / c.spacing).floor() as usize + 1;
        let mut out = Vec::with_capacity(nx + nz);

        // Lines parallel to X axis (one per Z step).
        let z_min = c.origin[2] - c.extent_z;
        let z_max = c.origin[2] + c.extent_z;
        let x_min = c.origin[0] - c.extent_x;
        let x_max = c.origin[0] + c.extent_x;

        let mut z = z_min;
        while z <= z_max + c.spacing * 0.001 {
            out.push(Line3DInstance {
                start: [x_min, y, z],
                width: c.line_width,
                end: [x_max, y, z],
                _pad: 0.0,
                color: c.color,
            });
            z += c.spacing;
        }

        // Lines parallel to Z axis (one per X step).
        let mut x = x_min;
        while x <= x_max + c.spacing * 0.001 {
            out.push(Line3DInstance {
                start: [x, y, z_min],
                width: c.line_width,
                end: [x, y, z_max],
                _pad: 0.0,
                color: c.color,
            });
            x += c.spacing;
        }

        out
    }

    /// Read-only access to the configuration.
    pub fn config(&self) -> &Grid3DConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return one perpendicular direction to the given axis.
///
/// Given a unit axis direction, returns a pair `[p1, p2]` where `p1` and
/// `p2` are unit vectors perpendicular to `axis_dir`.
fn perpendicular_pair(axis_dir: &[f32; 3]) -> [[f32; 3]; 2] {
    // Choose a non-parallel reference vector.
    let reference = if axis_dir[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };

    // p1 = axis × reference
    let p1 = cross(axis_dir, &reference);
    let p1 = normalize(&p1);

    // p2 = axis × p1
    let p2 = cross(axis_dir, &p1);
    let p2 = normalize(&p2);

    [p1, p2]
}

fn cross(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: &[f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-10 {
        return [0.0; 3];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_axis_generates_three_lines_plus_ticks() {
        let axis = Axis3D::new(Axis3DConfig::default());
        let instances = axis.generate_instances();

        // 3 axis lines + 3 axes × 4 ticks × 2 sides = 3 + 24 = 27.
        assert_eq!(instances.len(), 27, "expected 3 axis lines + 24 ticks");
    }

    #[test]
    fn axis_lines_span_configured_length() {
        let config = Axis3DConfig {
            length: 5.0,
            tick_count: 0, // No ticks for simpler assertion.
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        let instances = axis.generate_instances();

        assert_eq!(instances.len(), 3, "3 axis lines, no ticks");

        // X axis should go from (-5, 0, 0) to (5, 0, 0).
        let x_line = &instances[0];
        assert_eq!(x_line.start, [-5.0, 0.0, 0.0]);
        assert_eq!(x_line.end, [5.0, 0.0, 0.0]);

        // Y axis: (0, -5, 0) to (0, 5, 0).
        let y_line = &instances[1];
        assert_eq!(y_line.start, [0.0, -5.0, 0.0]);
        assert_eq!(y_line.end, [0.0, 5.0, 0.0]);

        // Z axis: (0, 0, -5) to (0, 0, 5).
        let z_line = &instances[2];
        assert_eq!(z_line.start, [0.0, 0.0, -5.0]);
        assert_eq!(z_line.end, [0.0, 0.0, 5.0]);
    }

    #[test]
    fn axis_respects_origin() {
        let config = Axis3DConfig {
            origin: [1.0, 2.0, 3.0],
            length: 1.0,
            tick_count: 0,
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        let instances = axis.generate_instances();

        let x_line = &instances[0];
        assert_eq!(x_line.start, [0.0, 2.0, 3.0]);
        assert_eq!(x_line.end, [2.0, 2.0, 3.0]);
    }

    #[test]
    fn axis_uses_configured_colors() {
        let config = Axis3DConfig {
            x_color: [1.0, 0.0, 0.0, 1.0],
            y_color: [0.0, 1.0, 0.0, 1.0],
            z_color: [0.0, 0.0, 1.0, 1.0],
            tick_count: 0,
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        let instances = axis.generate_instances();

        assert_eq!(instances[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(instances[1].color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(instances[2].color, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn tick_marks_are_perpendicular_to_axis() {
        let config = Axis3DConfig {
            length: 2.0,
            tick_count: 1,
            tick_size: 0.1,
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        let instances = axis.generate_instances();

        // First tick is after the X axis line (index 1).
        let tick = &instances[1];

        // Tick endpoints should differ only in non-X components.
        let dx = tick.end[0] - tick.start[0];
        let dy = tick.end[1] - tick.start[1];
        let dz = tick.end[2] - tick.start[2];

        // The tick is perpendicular to X, so dx should be 0.
        assert!(dx.abs() < 1e-6, "tick on X axis should have dx≈0, got {dx}");
        // And the tick should have nonzero length in some other direction.
        let perp_len = (dy * dy + dz * dz).sqrt();
        assert!(
            (perp_len - 0.2).abs() < 1e-5,
            "tick length should be 2×tick_size=0.2, got {perp_len}"
        );
    }

    #[test]
    fn default_grid_generates_expected_line_count() {
        let grid = Grid3D::new(Grid3DConfig::default());
        let instances = grid.generate_instances();

        // extent=2, spacing=0.5 → lines from -2 to 2 in steps of 0.5 = 9 lines.
        // 9 along X + 9 along Z = 18.
        assert_eq!(instances.len(), 18, "expected 9+9=18 grid lines");
    }

    #[test]
    fn grid_lies_on_xz_plane() {
        let grid = Grid3D::new(Grid3DConfig::default());
        let instances = grid.generate_instances();

        for inst in &instances {
            assert!(
                (inst.start[1]).abs() < 1e-6,
                "grid line start Y should be 0"
            );
            assert!((inst.end[1]).abs() < 1e-6, "grid line end Y should be 0");
        }
    }

    #[test]
    fn grid_respects_y_offset() {
        let config = Grid3DConfig {
            y_offset: -1.5,
            ..Default::default()
        };
        let grid = Grid3D::new(config);
        let instances = grid.generate_instances();

        for inst in &instances {
            assert!(
                (inst.start[1] - (-1.5)).abs() < 1e-6,
                "grid Y should be -1.5"
            );
        }
    }

    #[test]
    fn grid_zero_spacing_returns_empty() {
        let config = Grid3DConfig {
            spacing: 0.0,
            ..Default::default()
        };
        let grid = Grid3D::new(config);
        assert!(grid.generate_instances().is_empty());
    }

    #[test]
    fn grid_asymmetric_extents() {
        let config = Grid3DConfig {
            extent_x: 1.0,
            extent_z: 2.0,
            spacing: 1.0,
            ..Default::default()
        };
        let grid = Grid3D::new(config);
        let instances = grid.generate_instances();

        // X extent 1, spacing 1 → Z lines at x=-1, 0, 1 → 3 lines along Z.
        // Z extent 2, spacing 1 → X lines at z=-2,-1,0,1,2 → 5 lines along X.
        // Total: 3 + 5 = 8.
        assert_eq!(instances.len(), 8);
    }

    #[test]
    fn perpendicular_pair_for_each_axis() {
        let dirs: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        for dir in &dirs {
            let [p1, p2] = perpendicular_pair(dir);

            // p1 · dir ≈ 0
            let dot1 = p1[0] * dir[0] + p1[1] * dir[1] + p1[2] * dir[2];
            assert!(dot1.abs() < 1e-6, "p1 not perpendicular for {dir:?}");

            // p2 · dir ≈ 0
            let dot2 = p2[0] * dir[0] + p2[1] * dir[1] + p2[2] * dir[2];
            assert!(dot2.abs() < 1e-6, "p2 not perpendicular for {dir:?}");

            // p1 · p2 ≈ 0
            let dot12 = p1[0] * p2[0] + p1[1] * p2[1] + p1[2] * p2[2];
            assert!(dot12.abs() < 1e-6, "p1,p2 not perpendicular for {dir:?}");
        }
    }

    // -- Tick label tests --

    #[test]
    fn tick_labels_count_matches_ticks() {
        let config = Axis3DConfig {
            tick_count: 3,
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        let labels = axis.generate_tick_labels();

        // 3 axes × 3 ticks × 2 sides = 18 labels.
        assert_eq!(labels.len(), 18);
    }

    #[test]
    fn tick_labels_zero_count_returns_empty() {
        let config = Axis3DConfig {
            tick_count: 0,
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        assert!(axis.generate_tick_labels().is_empty());
    }

    #[test]
    fn tick_labels_contain_formatted_values() {
        let config = Axis3DConfig {
            length: 2.0,
            tick_count: 2,
            ..Default::default()
        };
        let axis = Axis3D::new(config);
        let labels = axis.generate_tick_labels();

        // Collect texts for the X axis (axis_index == 0).
        let x_texts: Vec<&str> = labels
            .iter()
            .filter(|l| l.axis_index == 0)
            .map(|l| l.text.as_str())
            .collect();

        // tick_count=2, length=2 → step=1.0 → values -1.0, 1.0, -2.0, 2.0
        assert!(x_texts.contains(&"-1.0"));
        assert!(x_texts.contains(&"1.0"));
        assert!(x_texts.contains(&"-2.0"));
        assert!(x_texts.contains(&"2.0"));
    }

    #[test]
    fn tick_labels_have_correct_axis_index() {
        let axis = Axis3D::new(Axis3DConfig::default());
        let labels = axis.generate_tick_labels();

        let x_count = labels.iter().filter(|l| l.axis_index == 0).count();
        let y_count = labels.iter().filter(|l| l.axis_index == 1).count();
        let z_count = labels.iter().filter(|l| l.axis_index == 2).count();

        // Each axis: 4 ticks × 2 sides = 8 labels.
        assert_eq!(x_count, 8);
        assert_eq!(y_count, 8);
        assert_eq!(z_count, 8);
    }
}
