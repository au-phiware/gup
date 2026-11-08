// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 11 of the tour: a chart of columns, with no row type and no
//! closure per row.

use gup::prelude::*;

/// `n` points on a spiral arm with a little scatter, made without a random
/// number generator: a linear congruential sequence, so every run is the
/// same.
fn spiral(n: u32) -> (Vec<f64>, Vec<f64>) {
    let mut state = 12345_u64;
    let mut noise = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as f64 / f64::from(1_u32 << 31) - 0.5
    };
    (0..n)
        .map(|i| {
            let t = f64::from(i) / f64::from(n);
            let angle = 12.0 * t;
            let radius = 1.0 + 9.0 * t;
            (
                radius * angle.cos() + 0.8 * noise(),
                radius * angle.sin() + 0.8 * noise(),
            )
        })
        .unzip()
}

pub fn columns() -> gup::Result<impl Chart> {
    // tour:begin columns
    let (x, y) = spiral(200_000);
    let cols = Columns::new().with("x", x)?.with("y", y)?;

    let mut plot = Plot::new();
    plot.title("200,000 points from two columns")
        .x_title("x")
        .y_title("y");
    plot.add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .attr(
            Circle::FILL,
            Sequential::viridis().encode_column(&cols["y"]),
        )
        .attr(Circle::RADIUS, Px(1.5))
        .legend_title(Circle::FILL, "y");
    Ok(plot)
    // tour:end columns
}

fn main() -> gup::Result<()> {
    gup::show(columns()?)
}
