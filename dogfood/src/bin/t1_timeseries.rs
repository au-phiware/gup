// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 1: multi-line time series from CSV with date axis, legend, title, PNG.
//!
//! What a user has to do today (see report):
//! * compute x/y domains by hand (axes default to 0..1 otherwise),
//! * pick a time unit for f32 x values that DateTimeFormatter understands,
//! * PNG export has no text (no title / ticks / legend), so we also export
//!   SVG; SVG export has text but no data marks, so we hand-build the line
//!   paths and the legend as SvgElements in pixel space and rasterise with
//!   ImageMagick.

use chrono::NaiveDate;
use gup::export::svg::SvgExportOptions;
use gup::export::svg::element::SvgElement;
use gup::label::formatter::DateTimeFormatter;
use gup::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Row {
    t: f32, // MILLIseconds since epoch (DateTimeFormatter unit, undocumented)
    ticker: String,
    price: Option<f32>,
}

const PALETTE: [[f32; 4]; 3] = [
    [0.12, 0.47, 0.71, 1.0],
    [1.00, 0.50, 0.05, 1.0],
    [0.17, 0.63, 0.17, 1.0],
];

fn load(path: &str) -> Result<(Vec<String>, Vec<Row>), Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let tickers: Vec<String> = rdr.headers()?.iter().skip(1).map(String::from).collect();
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        let date = NaiveDate::parse_from_str(&rec[0], "%Y-%m-%d")?;
        let t = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis() as f32;
        for (i, tk) in tickers.iter().enumerate() {
            let price = rec.get(i + 1).and_then(|s| s.parse().ok());
            out.push(Row { t, ticker: tk.clone(), price });
        }
    }
    Ok((tickers, out))
}

fn css([r, g, b, a]: [f32; 4]) -> String {
    format!("rgba({},{},{},{a})", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let (tickers, rows) = load("/tmp/gup-dogfood/prices.csv")?;
    let context = Arc::new(RenderContext::new().await?);

    // Domains by hand.
    let (x0, x1) = rows.iter().fold((f32::MAX, f32::MIN), |(a, b), r| (a.min(r.t), b.max(r.t)));
    let y1 = rows.iter().filter_map(|r| r.price).fold(0.0f32, f32::max) * 1.05;
    let (w, h) = (1000u32, 500u32);

    let mut chart = line()
        .x(AccessorFunction::new(|r: &Row| AccessorValue::Float(r.t)))
        .y(AccessorFunction::new(|r: &Row| AccessorValue::Float(r.price.unwrap_or(f32::NAN))))
        // Only String/Categorical splits series; Color draws ONE zigzag line.
        .color(AccessorFunction::new(|r: &Row| AccessorValue::Categorical(r.ticker.clone())))
        .connect_nulls(false)
        .stroke_width_px(1.5)
        .x_scale(LinearScale::new(x0, x1, -1.0, 1.0))
        .y_scale(LinearScale::new(0.0, y1, -1.0, 1.0))
        .title("Daily close, 2020-2024")
        .width(w as f32)
        .height(h as f32)
        .show_axes(true)
        .horizontal_grid()
        .x_tick_format(DateTimeFormatter::new("%Y-%m"))
        .build_with_data(rows.clone(), context)?;

    chart.export_png("/tmp/gup-dogfood/t1.png", w, h)?;

    // ---- Workaround: SVG with hand-built marks + legend -------------------
    // Plot rect discovered by reading gup's own SVG output (not documented).
    let plain = chart.render_to_svg(&SvgExportOptions::new(w, h))?;
    let axis_line = plain
        .lines()
        .skip_while(|l| !l.contains("class=\"axes\""))
        .nth(1)
        .ok_or("no axes in svg")?;
    let num = |key: &str| -> f32 {
        let s = &axis_line[axis_line.find(key).unwrap() + key.len() + 2..];
        s[..s.find('"').unwrap()].parse().unwrap()
    };
    let (px0, py_bottom, px1) = (num("x1"), num("y1"), num("x2"));
    let py_top = 40.0; // ditto; read from y-axis line in the real thing
    let sx = |t: f32| px0 + (t - x0) / (x1 - x0) * (px1 - px0);
    let sy = |v: f32| py_bottom - v / y1 * (py_bottom - py_top);

    let mut marks = Vec::new();
    for (i, tk) in tickers.iter().enumerate() {
        let mut d = String::new();
        let mut pen_up = true;
        for r in rows.iter().filter(|r| &r.ticker == tk) {
            match r.price {
                Some(p) => {
                    d.push_str(&format!("{}{:.1} {:.1} ", if pen_up { "M" } else { "L" }, sx(r.t), sy(p)));
                    pen_up = false;
                }
                None => pen_up = true,
            }
        }
        marks.push(SvgElement::Path {
            d,
            fill: "none".into(),
            stroke: Some(css(PALETTE[i])),
            stroke_width: Some(1.2),
        });
        // Legend swatch + label (top-left of plot area).
        let ly = py_top + 12.0 + i as f32 * 18.0;
        marks.push(SvgElement::Line {
            x1: px0 + 12.0,
            y1: ly,
            x2: px0 + 32.0,
            y2: ly,
            stroke: css(PALETTE[i]),
            stroke_width: 3.0,
            stroke_dasharray: None,
        });
        marks.push(SvgElement::Text {
            x: px0 + 38.0,
            y: ly + 4.0,
            content: tk.clone(),
            font_family: "sans-serif".into(),
            font_size: 12.0,
            text_anchor: "start".into(),
            fill: "black".into(),
            dominant_baseline: "auto".into(),
            font_weight: None,
        });
    }
    let svg = chart.export_svg_with_marks(&SvgExportOptions::new(w, h), &marks)?;
    std::fs::write("/tmp/gup-dogfood/t1.svg", svg)?;
    let ok = std::process::Command::new("magick")
        .args(["/tmp/gup-dogfood/t1.svg", "/tmp/gup-dogfood/t1_svg.png"])
        .status()?;
    println!("wrote t1.png, t1.svg, t1_svg.png (magick ok={})", ok.success());
    Ok(())
}
