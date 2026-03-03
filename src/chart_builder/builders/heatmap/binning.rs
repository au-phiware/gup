// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CPU-side 2D binning for heatmap data.
//!
//! Given a flat dataset and X/Y accessors, [`BinGrid::from_data`] partitions
//! the records into an `x_bins × y_bins` regular grid and applies an
//! [`AggregateFunc`] per cell.

use super::HeatmapCell;

// ── AggregateFunc ────────────────────────────────────────────────────────

/// Per-cell aggregation function applied during 2D binning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AggregateFunc {
    /// Count the number of records that fall into each cell.
    #[default]
    Count,
    /// Sum the fill values of all records in each cell.
    Sum,
    /// Arithmetic mean of the fill values in each cell.
    Mean,
    /// Minimum fill value in each cell.
    Min,
    /// Maximum fill value in each cell.
    Max,
}

// ── BinSpec ──────────────────────────────────────────────────────────────

/// Specification for one axis of the 2D binning grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinSpec {
    /// Number of bins along this axis.
    pub bins: usize,
    /// Domain minimum.
    pub min: f32,
    /// Domain maximum.
    pub max: f32,
}

impl BinSpec {
    /// Create a new bin specification.
    pub fn new(bins: usize, min: f32, max: f32) -> Self {
        Self {
            bins: bins.max(1),
            min,
            max,
        }
    }

    /// Width of each bin.
    pub fn bin_width(&self) -> f32 {
        if self.bins == 0 {
            return 0.0;
        }
        (self.max - self.min) / self.bins as f32
    }

    /// Map a data value to a bin index (clamped to `[0, bins - 1]`).
    pub fn bin_index(&self, value: f32) -> usize {
        if self.bins == 0 {
            return 0;
        }
        let width = self.bin_width();
        if width == 0.0 {
            return 0;
        }
        let idx = ((value - self.min) / width).floor() as isize;
        idx.clamp(0, self.bins as isize - 1) as usize
    }
}

// ── Accumulator ──────────────────────────────────────────────────────────

/// Per-cell accumulator used during the binning pass.
#[derive(Debug, Clone, Copy)]
struct CellAccum {
    count: u32,
    sum: f32,
    min: f32,
    max: f32,
}

impl CellAccum {
    fn new() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
        }
    }

    fn push(&mut self, value: f32) {
        self.count += 1;
        self.sum += value;
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
    }

    fn finalize(self, func: AggregateFunc, no_data: f32) -> f32 {
        if self.count == 0 {
            return no_data;
        }
        match func {
            AggregateFunc::Count => self.count as f32,
            AggregateFunc::Sum => self.sum,
            AggregateFunc::Mean => self.sum / self.count as f32,
            AggregateFunc::Min => self.min,
            AggregateFunc::Max => self.max,
        }
    }
}

// ── BinGrid ──────────────────────────────────────────────────────────────

/// Result of 2D binning: a grid of [`HeatmapCell`] values.
#[derive(Debug, Clone)]
pub struct BinGrid {
    /// Flat vector of cells in row-major order.
    pub cells: Vec<HeatmapCell>,
    /// X-axis bin specification.
    pub x_spec: BinSpec,
    /// Y-axis bin specification.
    pub y_spec: BinSpec,
}

impl BinGrid {
    /// Partition `data` into an `x_spec.bins × y_spec.bins` grid.
    ///
    /// - `x_values` and `y_values` are the per-record coordinates.
    /// - `fill_values` are the per-record values fed to the aggregate.
    /// - `func` selects the per-cell reduction.
    /// - `no_data` is the fill value for empty cells.
    ///
    /// All three slices must have the same length.
    pub fn from_data(
        x_values: &[f32],
        y_values: &[f32],
        fill_values: &[f32],
        x_spec: BinSpec,
        y_spec: BinSpec,
        func: AggregateFunc,
        no_data: f32,
    ) -> Self {
        let n_cells = x_spec.bins * y_spec.bins;
        let mut accums = vec![CellAccum::new(); n_cells];

        for i in 0..x_values.len() {
            let xi = x_spec.bin_index(x_values[i]);
            let yi = y_spec.bin_index(y_values[i]);
            let idx = yi * x_spec.bins + xi;
            accums[idx].push(fill_values[i]);
        }

        let cells = accums
            .into_iter()
            .enumerate()
            .map(|(idx, acc)| {
                let xi = (idx % x_spec.bins) as u32;
                let yi = (idx / x_spec.bins) as u32;
                HeatmapCell {
                    x_index: xi,
                    y_index: yi,
                    value: acc.finalize(func, no_data),
                }
            })
            .collect();

        Self {
            cells,
            x_spec,
            y_spec,
        }
    }

    /// Compute the min and max cell values, ignoring NaN.
    pub fn value_range(&self) -> (f32, f32) {
        let mut vmin = f32::INFINITY;
        let mut vmax = f32::NEG_INFINITY;
        for c in &self.cells {
            if c.value.is_finite() {
                vmin = vmin.min(c.value);
                vmax = vmax.max(c.value);
            }
        }
        if vmin > vmax {
            // All NaN or empty
            (0.0, 1.0)
        } else {
            (vmin, vmax)
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec(bins: usize, min: f32, max: f32) -> BinSpec {
        BinSpec::new(bins, min, max)
    }

    // ── BinSpec tests ─────────────────────────────────────────────────

    #[test]
    fn test_bin_spec_width() {
        let spec = make_spec(10, 0.0, 100.0);
        assert!((spec.bin_width() - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bin_spec_index_basic() {
        let spec = make_spec(10, 0.0, 10.0);
        assert_eq!(spec.bin_index(0.0), 0);
        assert_eq!(spec.bin_index(0.5), 0);
        assert_eq!(spec.bin_index(1.0), 1);
        assert_eq!(spec.bin_index(9.99), 9);
    }

    #[test]
    fn test_bin_spec_index_clamped() {
        let spec = make_spec(5, 0.0, 10.0);
        // Below domain → clamped to 0
        assert_eq!(spec.bin_index(-5.0), 0);
        // Above domain → clamped to last bin
        assert_eq!(spec.bin_index(100.0), 4);
    }

    #[test]
    fn test_bin_spec_single_bin() {
        let spec = make_spec(1, 0.0, 10.0);
        assert_eq!(spec.bin_index(5.0), 0);
        assert_eq!(spec.bin_index(0.0), 0);
        assert_eq!(spec.bin_index(10.0), 0);
    }

    #[test]
    fn test_bin_spec_zero_width_domain() {
        let spec = make_spec(5, 3.0, 3.0);
        assert_eq!(spec.bin_index(3.0), 0);
    }

    // ── BinGrid tests ─────────────────────────────────────────────────

    #[test]
    fn test_empty_input() {
        let grid = BinGrid::from_data(
            &[],
            &[],
            &[],
            make_spec(3, 0.0, 3.0),
            make_spec(3, 0.0, 3.0),
            AggregateFunc::Count,
            f32::NAN,
        );
        assert_eq!(grid.cells.len(), 9);
        // All cells should be NaN (no data)
        for c in &grid.cells {
            assert!(c.value.is_nan(), "Expected NaN for empty cell");
        }
    }

    #[test]
    fn test_single_cell_grid() {
        let grid = BinGrid::from_data(
            &[0.5],
            &[0.5],
            &[42.0],
            make_spec(1, 0.0, 1.0),
            make_spec(1, 0.0, 1.0),
            AggregateFunc::Sum,
            0.0,
        );
        assert_eq!(grid.cells.len(), 1);
        assert!((grid.cells[0].value - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_count_aggregation() {
        // Place 5 points into different bins of a 2×2 grid:
        //   (0.25, 0.25) → (0, 0)
        //   (0.75, 0.25) → (1, 0)
        //   (0.25, 0.75) → (0, 1)
        //   (0.75, 0.75) → (1, 1)
        //   (0.75, 0.75) → (1, 1)  duplicate
        let xs = [0.25, 0.75, 0.25, 0.75, 0.75];
        let ys = [0.25, 0.25, 0.75, 0.75, 0.75];
        let fs = [1.0; 5];

        let grid = BinGrid::from_data(
            &xs,
            &ys,
            &fs,
            make_spec(2, 0.0, 1.0),
            make_spec(2, 0.0, 1.0),
            AggregateFunc::Count,
            0.0,
        );

        // row 0: (0,0)=1, (1,0)=1
        // row 1: (0,1)=1, (1,1)=2
        assert_eq!(grid.cells.len(), 4);
        let val = |xi: u32, yi: u32| -> f32 {
            grid.cells
                .iter()
                .find(|c| c.x_index == xi && c.y_index == yi)
                .unwrap()
                .value
        };
        assert!((val(0, 0) - 1.0).abs() < f32::EPSILON);
        assert!((val(1, 0) - 1.0).abs() < f32::EPSILON);
        assert!((val(0, 1) - 1.0).abs() < f32::EPSILON);
        assert!((val(1, 1) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sum_aggregation_roundtrip() {
        // AC2 round-trip test: aggregate Sum over 10 000 uniformly
        // distributed points into a 10×10 grid and verify total.
        let n = 10_000;
        let mut xs = Vec::with_capacity(n);
        let mut ys = Vec::with_capacity(n);
        let mut fs = Vec::with_capacity(n);

        for i in 0..n {
            // Deterministic pseudo-uniform distribution
            let t = i as f32 / n as f32;
            xs.push((t * 997.0) % 1.0 * 10.0); // in [0, 10)
            ys.push((t * 991.0) % 1.0 * 10.0);
            fs.push(1.0);
        }

        let input_sum: f32 = fs.iter().sum();

        let grid = BinGrid::from_data(
            &xs,
            &ys,
            &fs,
            make_spec(10, 0.0, 10.0),
            make_spec(10, 0.0, 10.0),
            AggregateFunc::Sum,
            0.0,
        );

        let grid_sum: f32 = grid.cells.iter().map(|c| c.value).sum();
        assert!(
            (grid_sum - input_sum).abs() < 1.0,
            "Grid sum {grid_sum} should equal input sum {input_sum}"
        );
    }

    #[test]
    fn test_mean_aggregation() {
        let xs = [0.5, 0.5, 0.5];
        let ys = [0.5, 0.5, 0.5];
        let fs = [10.0, 20.0, 30.0];

        let grid = BinGrid::from_data(
            &xs,
            &ys,
            &fs,
            make_spec(1, 0.0, 1.0),
            make_spec(1, 0.0, 1.0),
            AggregateFunc::Mean,
            0.0,
        );

        assert!((grid.cells[0].value - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_min_max_aggregation() {
        let xs = [0.5, 0.5, 0.5];
        let ys = [0.5, 0.5, 0.5];
        let fs = [10.0, 5.0, 30.0];

        let min_grid = BinGrid::from_data(
            &xs,
            &ys,
            &fs,
            make_spec(1, 0.0, 1.0),
            make_spec(1, 0.0, 1.0),
            AggregateFunc::Min,
            0.0,
        );
        assert!((min_grid.cells[0].value - 5.0).abs() < f32::EPSILON);

        let max_grid = BinGrid::from_data(
            &xs,
            &ys,
            &fs,
            make_spec(1, 0.0, 1.0),
            make_spec(1, 0.0, 1.0),
            AggregateFunc::Max,
            0.0,
        );
        assert!((max_grid.cells[0].value - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_nan_cell_handling() {
        // Empty cells should be NaN when no_data is NaN.
        let grid = BinGrid::from_data(
            &[0.25],
            &[0.25],
            &[5.0],
            make_spec(2, 0.0, 1.0),
            make_spec(2, 0.0, 1.0),
            AggregateFunc::Sum,
            f32::NAN,
        );

        let filled: Vec<_> = grid.cells.iter().filter(|c| c.value.is_finite()).collect();
        let empty: Vec<_> = grid.cells.iter().filter(|c| c.value.is_nan()).collect();
        assert_eq!(filled.len(), 1);
        assert_eq!(empty.len(), 3);
    }

    #[test]
    fn test_value_range() {
        let grid = BinGrid::from_data(
            &[0.1, 0.9],
            &[0.1, 0.9],
            &[3.0, 7.0],
            make_spec(2, 0.0, 1.0),
            make_spec(2, 0.0, 1.0),
            AggregateFunc::Sum,
            f32::NAN,
        );

        let (vmin, vmax) = grid.value_range();
        assert!((vmin - 3.0).abs() < f32::EPSILON);
        assert!((vmax - 7.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_value_range_all_nan() {
        let grid = BinGrid::from_data(
            &[],
            &[],
            &[],
            make_spec(2, 0.0, 1.0),
            make_spec(2, 0.0, 1.0),
            AggregateFunc::Sum,
            f32::NAN,
        );

        let (vmin, vmax) = grid.value_range();
        assert!((vmin - 0.0).abs() < f32::EPSILON);
        assert!((vmax - 1.0).abs() < f32::EPSILON);
    }
}
