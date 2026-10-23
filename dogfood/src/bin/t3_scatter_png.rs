// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 3a: 200k-point scatter, colour = categorical segment, size = numeric
//! weight, log-scale x axis; PNG and SVG (guides only) export.

use gup::prelude::*;
use gup::{SvgTarget, VectorTarget};
use gup_dogfood::{PALETTE_HEX, load_points};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let t0 = Instant::now();
    let data = load_points()?;
    println!("loaded {} rows in {:?}", data.len(), t0.elapsed());
    let cx = Context::new_blocking()?;

    let mut chart = gup::scatter(data)
        .x(|p| p.income)
        .x_scale(Log::new())
        .y(|p| p.spend)
        .color(|p| p.segment.as_str())
        .color_scale(Categorical::from_palette(PALETTE_HEX.map(Color::hex)))
        .size(|p| p.weight)
        .opacity(0.35)
        .grid(Axes::Both)
        .title("Spend vs income (200k customers)");

    let t1 = Instant::now();
    chart.save_png(&cx, "/tmp/gup-dogfood/t3_scatter.png", 1000, 700)?;
    println!("exported in {:?}", t1.elapsed());

    let resolved = chart.resolve(&cx, 1000.0, 700.0)?;
    let mut svg = SvgTarget::new();
    svg.render(&resolved.scene)?;
    std::fs::write("/tmp/gup-dogfood/t3_scatter.svg", svg.svg())?;
    Ok(())
}
