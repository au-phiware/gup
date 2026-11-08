// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 5 of the tour: the default palette and the dark and
//! high-contrast themes.

use gup::prelude::*;

/// Monthly sales of five products: (month, product, units).
fn sales() -> Vec<(f64, &'static str, f64)> {
    let products = ["Tea", "Coffee", "Cocoa", "Juice", "Water"];
    (1..=12)
        .flat_map(|month| {
            let m = f64::from(month);
            products.iter().enumerate().map(move |(i, product)| {
                let i = i as f64;
                let units = 20.0 + 14.0 * i + 5.0 * (m / 2.0 + i).sin() + 0.25 * m * (i - 2.0);
                (m, *product, units)
            })
        })
        .collect()
}

pub fn palette() -> impl Chart {
    // tour:begin palette
    gup::line(sales())
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .legend_title("Product")
        .title("Units sold by product")
        .x_title("Month")
    // tour:end palette
}

pub fn dark() -> impl Chart {
    // tour:begin dark
    gup::line(sales())
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .legend_title("Product")
        .title("Units sold by product")
        .x_title("Month")
        .theme(Theme::dark())
    // tour:end dark
}

pub fn high_contrast() -> impl Chart {
    // tour:begin high_contrast
    gup::line(sales())
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .legend_title("Product")
        .title("Units sold by product")
        .x_title("Month")
        .theme(Theme::high_contrast())
    // tour:end high_contrast
}

fn main() -> gup::Result<()> {
    gup::show(palette())
}
