// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 1: multi-line time series from CSV with date axis, legend, title, PNG.

use chrono::{DateTime, NaiveDate};
use gup::prelude::*;
use gup::{SvgTarget, VectorTarget};

struct Row {
    /// Unix seconds.
    t: f64,
    ticker: String,
    price: Option<f64>,
}

const PALETTE: [u32; 3] = [0x1f77b4, 0xff7f0e, 0x2ca02c];

fn load(path: &str) -> Result<Vec<Row>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let tickers: Vec<String> = rdr.headers()?.iter().skip(1).map(String::from).collect();
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        let date = NaiveDate::parse_from_str(&rec[0], "%Y-%m-%d")?;
        let t = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64;
        for (i, tk) in tickers.iter().enumerate() {
            out.push(Row {
                t,
                ticker: tk.clone(),
                price: rec.get(i + 1).and_then(|s| s.parse().ok()),
            });
        }
    }
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let rows = load("/tmp/gup-dogfood/prices.csv")?;
    let cx = Context::new_blocking()?;
    let mut chart = gup::line(rows)
        .x(|r| r.t)
        .x_scale(Time::new())
        .x_tick_format(|v| {
            DateTime::from_timestamp(v as i64, 0)
                .map(|d| d.format("%Y-%m").to_string())
                .unwrap_or_default()
        })
        .y(|r| r.price)
        .color(|r| r.ticker.clone())
        .color_scale(Categorical::from_palette(PALETTE.map(Color::hex)))
        .width(1.5)
        .grid(Axes::Y)
        .title("Daily close, 2020-2024");
    chart.save_png(&cx, "/tmp/gup-dogfood/t1.png", 1000, 500)?;
    println!("wrote t1.png");
    let resolved = chart.resolve(&cx, 1000.0, 500.0)?;
    let mut svg = SvgTarget::new();
    svg.render(&resolved.scene)?;
    std::fs::write("/tmp/gup-dogfood/t1.svg", svg.svg())?;
    println!("wrote t1.svg");
    Ok(())
}
