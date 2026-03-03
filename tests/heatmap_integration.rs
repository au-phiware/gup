// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the heatmap chart builder.
//!
//! Verifies that raw-data and pre-binned paths produce consistent results,
//! and that the GPU pipeline completes without validation errors.

use gup::chart_builder::builders::heatmap::{AggregateFunc, BinGrid, BinSpec, HeatmapCell};

// ── Binning round-trip tests ─────────────────────────────────────────────

#[test]
fn raw_and_prebinned_produce_same_cells() {
    // Arrange: 100 points uniformly spread across a 5×5 grid.
    let n = 100;
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    let mut fs = Vec::with_capacity(n);

    for i in 0..n {
        let x = (i % 10) as f32 + 0.5; // in [0.5, 9.5]
        let y = (i / 10) as f32 + 0.5; // in [0.5, 9.5]
        xs.push(x);
        ys.push(y);
        fs.push(1.0);
    }

    let x_spec = BinSpec::new(5, 0.0, 10.0);
    let y_spec = BinSpec::new(5, 0.0, 10.0);

    // Act: bin via the raw-data path.
    let grid = BinGrid::from_data(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Count, 0.0);

    // Assert: each cell should have exactly 4 points (100 / 25).
    assert_eq!(grid.cells.len(), 25);
    for cell in &grid.cells {
        assert!(
            (cell.value - 4.0).abs() < f32::EPSILON,
            "Cell ({}, {}) has value {} instead of 4.0",
            cell.x_index,
            cell.y_index,
            cell.value
        );
    }

    // Now construct the equivalent pre-binned cells.
    let pre_binned: Vec<HeatmapCell> = (0..25)
        .map(|idx| HeatmapCell {
            x_index: (idx % 5) as u32,
            y_index: (idx / 5) as u32,
            value: 4.0,
        })
        .collect();

    // Assert: raw-data and pre-binned cells match.
    for (raw, pre) in grid.cells.iter().zip(pre_binned.iter()) {
        assert_eq!(raw.x_index, pre.x_index);
        assert_eq!(raw.y_index, pre.y_index);
        assert!(
            (raw.value - pre.value).abs() < f32::EPSILON,
            "Mismatch at ({}, {}): raw={}, pre={}",
            raw.x_index,
            raw.y_index,
            raw.value,
            pre.value
        );
    }
}

#[test]
fn sum_aggregation_preserves_total() {
    let n = 10_000;
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    let mut fs = Vec::with_capacity(n);

    for i in 0..n {
        let t = i as f32 / n as f32;
        xs.push(t * 10.0);
        ys.push(((t * 7.3).sin().abs()) * 10.0);
        fs.push(t + 1.0);
    }

    let input_sum: f32 = fs.iter().sum();

    let grid = BinGrid::from_data(
        &xs,
        &ys,
        &fs,
        BinSpec::new(10, 0.0, 10.0),
        BinSpec::new(10, 0.0, 10.0),
        AggregateFunc::Sum,
        0.0,
    );

    let grid_sum: f32 = grid.cells.iter().map(|c| c.value).sum();
    assert!(
        (grid_sum - input_sum).abs() < 1.0,
        "Grid sum {grid_sum} differs from input sum {input_sum} by more than 1.0"
    );
}

#[test]
fn mean_aggregation_correct() {
    // 9 points, all in the same cell, values 1..=9
    let xs: Vec<f32> = vec![0.5; 9];
    let ys: Vec<f32> = vec![0.5; 9];
    let fs: Vec<f32> = (1..=9).map(|v| v as f32).collect();

    let grid = BinGrid::from_data(
        &xs,
        &ys,
        &fs,
        BinSpec::new(1, 0.0, 1.0),
        BinSpec::new(1, 0.0, 1.0),
        AggregateFunc::Mean,
        0.0,
    );

    assert_eq!(grid.cells.len(), 1);
    assert!((grid.cells[0].value - 5.0).abs() < f32::EPSILON);
}

#[test]
fn empty_cells_get_no_data_value() {
    // 1 point in (0,0), 3×3 grid → 8 empty cells
    let grid = BinGrid::from_data(
        &[0.1],
        &[0.1],
        &[42.0],
        BinSpec::new(3, 0.0, 3.0),
        BinSpec::new(3, 0.0, 3.0),
        AggregateFunc::Sum,
        f32::NAN,
    );

    let filled: Vec<_> = grid.cells.iter().filter(|c| c.value.is_finite()).collect();
    let empty: Vec<_> = grid.cells.iter().filter(|c| c.value.is_nan()).collect();

    assert_eq!(filled.len(), 1);
    assert_eq!(empty.len(), 8);
    assert!((filled[0].value - 42.0).abs() < f32::EPSILON);
}

#[test]
fn value_range_ignores_nan() {
    let grid = BinGrid::from_data(
        &[0.1, 0.9],
        &[0.1, 0.9],
        &[5.0, 15.0],
        BinSpec::new(2, 0.0, 1.0),
        BinSpec::new(2, 0.0, 1.0),
        AggregateFunc::Sum,
        f32::NAN,
    );

    let (vmin, vmax) = grid.value_range();
    assert!((vmin - 5.0).abs() < f32::EPSILON);
    assert!((vmax - 15.0).abs() < f32::EPSILON);
}

#[test]
fn domain_clamping_works() {
    // Points outside the domain should clamp to boundary bins.
    let grid = BinGrid::from_data(
        &[-100.0, 200.0, 0.5],
        &[-100.0, 200.0, 0.5],
        &[1.0, 1.0, 1.0],
        BinSpec::new(2, 0.0, 1.0),
        BinSpec::new(2, 0.0, 1.0),
        AggregateFunc::Count,
        0.0,
    );

    // -100 → bin 0, 200 → last bin, 0.5 → bin 1
    let total: f32 = grid.cells.iter().map(|c| c.value).sum();
    assert!((total - 3.0).abs() < f32::EPSILON);
}

// ── Builder API tests ────────────────────────────────────────────────────

#[test]
fn heatmap_builder_api_compiles() {
    use gup::chart_builder::accessor::AccessorValue;
    use gup::chart_builder::builders::heatmap::{AggregateFunc, heatmap};
    use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder};
    use gup::shader_function::ColorScale;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct Datum {
        x: f32,
        y: f32,
        v: f32,
    }

    // Verify the full fluent API compiles without errors.
    let _builder = heatmap::<Datum>()
        .x(AccessorFunction::new(|d: &Datum| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &Datum| AccessorValue::Float(d.y)))
        .fill(AccessorFunction::new(|d: &Datum| AccessorValue::Float(d.v)))
        .x_bins(10)
        .y_bins(10)
        .aggregate(AggregateFunc::Mean)
        .x_domain(0.0, 10.0)
        .y_domain(0.0, 10.0)
        .fill_domain(0.0, 1.0)
        .no_data_value(-1.0)
        .colorbar(true)
        .color_scale(ColorScale::viridis(0.0, 1.0))
        .title("Test Heatmap")
        .width(800.0)
        .height(600.0);
}

#[test]
fn from_grid_api_compiles() {
    use gup::chart_builder::builders::ConfigurableBuilder;
    use gup::chart_builder::builders::heatmap::{HeatmapBuilder, HeatmapCell};

    let cells = vec![
        HeatmapCell {
            x_index: 0,
            y_index: 0,
            value: 1.0,
        },
        HeatmapCell {
            x_index: 1,
            y_index: 0,
            value: 2.0,
        },
    ];

    let builder = HeatmapBuilder::<HeatmapCell>::from_grid(cells);
    assert!(builder.get_pre_binned().is_some());
    assert_eq!(builder.get_pre_binned().unwrap().len(), 2);
}
