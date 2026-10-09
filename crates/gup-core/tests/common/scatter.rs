// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The S0 reference scatter: GDP per capita on a linear x axis,
//! population on a log y axis, fill by life expectancy (viridis), constant
//! radius. One definition shared by the PNG golden test
//! (`tests/scatter_png.rs`), the window parity test
//! (`tests/window_parity.rs`) and the `scatter_window` example, so every
//! target draws the same `Plot`.

#![allow(dead_code)] // each includer uses a different subset

use gup_core::ScaleRef;
use gup_core::prelude::*;

/// Golden image width.
pub const WIDTH: u32 = 720;
/// Golden image height.
pub const HEIGHT: u32 = 450;
/// The chart title.
pub const TITLE: &str = "Wealth, population and life expectancy";

/// One row.
#[derive(Clone, Debug)]
pub struct Country {
    pub gdp_per_capita: f64,
    pub population: f64,
    pub life_expectancy: f64,
}

/// `n` deterministic, plausible-looking rows (no RNG crate needed).
pub fn countries_n(n: usize) -> Vec<Country> {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..n)
        .map(|_| {
            let wealth = next();
            Country {
                gdp_per_capita: 500.0 + wealth * wealth * 58_000.0,
                population: 10f64.powf(5.2 + next() * 3.8),
                life_expectancy: 52.0 + wealth * 26.0 + next() * 6.0,
            }
        })
        .collect()
}

/// The 120 rows of the golden image.
pub fn countries() -> Vec<Country> {
    countries_n(120)
}

/// The reference plot over `rows`, with its x and y scale handles.
pub fn plot_rows(rows: Vec<Country>, radius: Px) -> (Plot, ScaleRef<Linear>, ScaleRef<Log>) {
    plot_rows_chunked(rows, radius, None)
}

/// [`plot_rows`] with at most `max_chunk_rows` rows per column chunk, so
/// the layer is drawn by several instanced draws (RFC-001 S4a).
pub fn plot_rows_chunked(
    rows: Vec<Country>,
    radius: Px,
    max_chunk_rows: Option<u32>,
) -> (Plot, ScaleRef<Linear>, ScaleRef<Log>) {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
    let heat = Sequential::viridis();
    let layer = plot
        .title(TITLE)
        .add(Selection::<Country, Circle>::new(rows));
    layer
        .attr(Circle::X, x.encode(|c: &Country| c.gdp_per_capita))
        .attr(Circle::Y, y.encode(|c: &Country| c.population))
        .attr(Circle::FILL, heat.encode(|c: &Country| c.life_expectancy))
        .attr(Circle::RADIUS, radius);
    if let Some(rows) = max_chunk_rows {
        layer.max_chunk_rows(rows);
    }
    (plot, x, y)
}

/// The golden scatter.
pub fn plot() -> Plot {
    plot_rows(countries(), Px(4.5)).0
}

/// The golden scatter's rows in column chunks of `max_chunk_rows` rows.
pub fn plot_chunked(max_chunk_rows: u32) -> Plot {
    plot_rows_chunked(countries(), Px(4.5), Some(max_chunk_rows)).0
}

/// The fill scale with the domain the plot fits, for colour checks.
pub fn fill() -> Sequential {
    let ext = countries()
        .iter()
        .map(|c| c.life_expectancy)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    Sequential::viridis().domain(ext.0, ext.1)
}
