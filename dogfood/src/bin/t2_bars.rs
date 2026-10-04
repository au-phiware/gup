// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 2: grouped + stacked bar chart (region x quarter) with legend and
//! value labels, exported to PNG.
//!
//! Part A ("naive") uses the documented `group_by` / `stack_by` API.
//! Part B ("workaround") fakes both because `group_by`/`stack_by` do not
//! affect bar geometry, `.color(String)` renders grey, and there is no
//! legend / value-label API.

use gup::export::svg::SvgExportOptions;
use gup::export::svg::element::SvgElement;
use gup::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Sale {
    region: String,
    quarter: String,
    sales: f32,
}

/// Row fed to the workaround charts.
#[derive(Debug, Clone)]
struct Bar {
    key: String,
    value: f32,
    color: [f32; 4],
}

const PALETTE: [[f32; 4]; 4] = [
    [0.122, 0.467, 0.706, 1.0],
    [1.000, 0.498, 0.055, 1.0],
    [0.173, 0.627, 0.173, 1.0],
    [0.839, 0.153, 0.157, 1.0],
];
const REGIONS: [&str; 4] = ["North", "South", "East", "West"];
const QUARTERS: [&str; 4] = ["Q1", "Q2", "Q3", "Q4"];

fn load() -> Result<Vec<Sale>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path("/tmp/gup-dogfood/sales.csv")?;
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        out.push(Sale { region: rec[0].into(), quarter: rec[1].into(), sales: rec[2].parse()? });
    }
    Ok(out)
}

fn css([r, g, b, a]: [f32; 4]) -> String {
    format!("rgba({},{},{},{a})", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn text(x: f32, y: f32, s: String, anchor: &str) -> SvgElement {
    SvgElement::Text {
        x,
        y,
        content: s,
        font_family: "sans-serif".into(),
        font_size: 11.0,
        text_anchor: anchor.into(),
        fill: "black".into(),
        dominant_baseline: "auto".into(),
        font_weight: None,
    }
}

fn legend(px0: f32, py0: f32) -> Vec<SvgElement> {
    let mut v = Vec::new();
    for (i, r) in REGIONS.iter().enumerate() {
        let x = px0 + 10.0 + i as f32 * 80.0;
        v.push(SvgElement::Rect {
            x,
            y: py0 - 22.0,
            width: 10.0,
            height: 10.0,
            fill: css(PALETTE[i]),
            stroke: None,
            stroke_width: None,
            rx: None,
        });
        v.push(text(x + 14.0, py0 - 13.0, r.to_string(), "start"));
    }
    v
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let data = load()?;
    let ctx = Arc::new(RenderContext::new().await?);
    let (w, h) = (800u32, 500u32);

    // ---- Part A: documented API -------------------------------------------
    let quarter = || AccessorFunction::new(|d: &Sale| AccessorValue::String(d.quarter.clone()));
    let sales = || AccessorFunction::new(|d: &Sale| AccessorValue::Float(d.sales));
    let region = || AccessorFunction::new(|d: &Sale| AccessorValue::String(d.region.clone()));
    for (name, stacked) in [("grouped", false), ("stacked", true)] {
        let b = bar().x(quarter()).y(sales()).color(region()).gap(0.15);
        let b = if stacked { b.stack_by(region()) } else { b.group_by(region()) };
        let mut chart = b
            .title(format!("Sales by quarter and region ({name})"))
            .width(w as f32)
            .height(h as f32)
            .show_axes(true)
            .horizontal_grid()
            .build_with_data(data.clone(), ctx.clone())?;
        chart.export_png(format!("/tmp/gup-dogfood/t2_{name}.png"), w, h)?;
        chart.export_svg(format!("/tmp/gup-dogfood/t2_{name}.svg"), &SvgExportOptions::new(w, h))?;
    }

    // ---- Part B: workaround ------------------------------------------------
    let get = |q: &str, r: &str| {
        data.iter().find(|d| d.quarter == q && d.region == r).map(|d| d.sales).unwrap_or(0.0)
    };
    // Grouped: one band per (quarter, region); key encodes both.
    let grouped: Vec<Bar> = QUARTERS
        .iter()
        .flat_map(|q| {
            REGIONS.iter().enumerate().map(move |(i, r)| (q, i, r))
        })
        .map(|(q, i, r)| Bar { key: format!("{q} {}", &r[..1]), value: get(q, r), color: PALETTE[i] })
        .collect();
    // Stacked: overlapping cumulative bars, tallest first so smaller ones paint on top.
    let mut stacked = Vec::new();
    for q in QUARTERS {
        let mut cum = 0.0;
        let mut layers = Vec::new();
        for (i, r) in REGIONS.iter().enumerate() {
            cum += get(q, r);
            layers.push(Bar { key: q.into(), value: cum, color: PALETTE[i] });
        }
        layers.reverse();
        stacked.extend(layers);
    }

    for (name, rows, gap) in [("grouped_wa", grouped, 0.1), ("stacked_wa", stacked, 0.3)] {
        let mut chart = bar()
            .x(AccessorFunction::new(|b: &Bar| AccessorValue::String(b.key.clone())))
            .y(AccessorFunction::new(|b: &Bar| AccessorValue::Float(b.value)))
            .color(AccessorFunction::new(|b: &Bar| AccessorValue::Color(b.color)))
            .gap(gap)
            .title("Sales by quarter and region")
            .width(w as f32)
            .height(h as f32)
            .show_axes(true)
            .horizontal_grid()
            .build_with_data(rows.clone(), ctx.clone())?;
        chart.export_png(format!("/tmp/gup-dogfood/t2_{name}.png"), w, h)?;

        // Value labels + legend via SVG (PNG has no text). Bar geometry must be
        // re-derived: read the plot rect from gup's SVG axes, then re-implement
        // the band layout (padding semantics guessed from the image).
        let plain = chart.render_to_svg(&SvgExportOptions::new(w, h))?;
        let nums: Vec<f32> = plain
            .lines()
            .skip_while(|l| !l.contains("class=\"axes\""))
            .nth(1)
            .unwrap()
            .split('"')
            .filter_map(|s| s.parse().ok())
            .collect();
        let (px0, py_bottom, px1) = (nums[0], nums[1], nums[2]);
        let y_max_line = plain
            .lines()
            .skip_while(|l| !l.contains("class=\"axes\""))
            .nth(2)
            .unwrap()
            .split('"')
            .filter_map(|s| s.parse::<f32>().ok())
            .collect::<Vec<_>>();
        let py_top = y_max_line[3];
        let dmax = rows.iter().map(|r| r.value).fold(0.0, f32::max) * 1.1; // bar.rs headroom
        let mut keys: Vec<&str> = Vec::new();
        for r in &rows {
            if !keys.contains(&r.key.as_str()) {
                keys.push(&r.key);
            }
        }
        let step = (px1 - px0) / keys.len() as f32;
        let mut marks = legend(px0, py_top);
        let mut prev_by_key = std::collections::HashMap::new();
        for r in rows.iter().rev() {
            let k = keys.iter().position(|k| *k == r.key).unwrap();
            let cx = px0 + step * (k as f32 + 0.5);
            let y = py_bottom - r.value / dmax * (py_bottom - py_top);
            let prev: f32 = *prev_by_key.get(&r.key).unwrap_or(&0.0);
            let label = format!("{:.0}", r.value - prev);
            prev_by_key.insert(r.key.clone(), r.value);
            marks.push(text(cx, y + if name == "stacked_wa" { 14.0 } else { -3.0 }, label, "middle"));
        }
        let svg = chart.export_svg_with_marks(&SvgExportOptions::new(w, h), &marks)?;
        std::fs::write(format!("/tmp/gup-dogfood/t2_{name}.svg"), svg)?;
    }
    println!("done");
    Ok(())
}
