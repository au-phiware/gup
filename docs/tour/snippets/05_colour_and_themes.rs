// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 5 of the tour: the default palette and the dark and
//! high-contrast themes.

use gup::prelude::*;

struct Sale {
    month: f64,
    product: &'static str,
    units: f64,
}

/// Monthly sales of five products.
fn sales() -> Vec<Sale> {
    let products = ["Tea", "Coffee", "Cocoa", "Juice", "Water"];
    (1..=12)
        .flat_map(|month| {
            let m = f64::from(month);
            products.iter().enumerate().map(move |(i, product)| {
                let i = i as f64;
                Sale {
                    month: m,
                    product,
                    units: 20.0 + 14.0 * i + 5.0 * (m / 2.0 + i).sin() + 0.25 * m * (i - 2.0),
                }
            })
        })
        .collect()
}

pub fn palette() -> impl Chart {
    // tour:begin palette
    gup::line(sales())
        .x(|s| s.month)
        .y(|s| s.units)
        .color(|s| s.product)
        .legend_title("Product")
        .title("Units sold by product")
        .x_title("Month")
    // tour:end palette
}

pub fn dark() -> impl Chart {
    // tour:begin dark
    gup::line(sales())
        .x(|s| s.month)
        .y(|s| s.units)
        .color(|s| s.product)
        .legend_title("Product")
        .title("Units sold by product")
        .x_title("Month")
        .theme(Theme::dark())
    // tour:end dark
}

pub fn high_contrast() -> impl Chart {
    // tour:begin high_contrast
    gup::line(sales())
        .x(|s| s.month)
        .y(|s| s.units)
        .color(|s| s.product)
        .legend_title("Product")
        .title("Units sold by product")
        .x_title("Month")
        .theme(Theme::high_contrast())
    // tour:end high_contrast
}

fn main() -> gup::Result<()> {
    gup::show(palette())
}
