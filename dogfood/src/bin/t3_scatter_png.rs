// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 3a: 200k-point scatter, colour = categorical segment, size = numeric
//! weight, log-scale x axis; PNG export.

use gup::label::formatter::CustomFormatter;
use gup::prelude::*;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Pt {
    pub id: u32,
    pub income: f32,
    pub spend: f32,
    pub segment: String,
    pub weight: f32,
}

pub fn load() -> Result<Vec<Pt>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path("/tmp/gup-dogfood/points.csv")?;
    let mut out = Vec::with_capacity(200_000);
    for rec in rdr.records() {
        let r = rec?;
        out.push(Pt {
            id: r[0].parse()?,
            income: r[1].parse()?,
            spend: r[2].parse()?,
            segment: r[3].into(),
            weight: r[4].parse()?,
        });
    }
    Ok(out)
}

pub const SEGMENTS: [&str; 5] = ["retail", "wholesale", "online", "partner", "other"];
pub const PALETTE: [[f32; 3]; 5] = [
    [0.122, 0.467, 0.706],
    [1.000, 0.498, 0.055],
    [0.173, 0.627, 0.173],
    [0.839, 0.153, 0.157],
    [0.580, 0.404, 0.741],
];
pub fn seg_color(s: &str) -> [f32; 4] {
    let i = SEGMENTS.iter().position(|x| *x == s).unwrap_or(4);
    let [r, g, b] = PALETTE[i];
    [r, g, b, 0.35]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let t0 = Instant::now();
    let data = load()?;
    println!("loaded {} rows in {:?}", data.len(), t0.elapsed());
    let ctx = Arc::new(RenderContext::new().await?);

    let (xmin, xmax) = data.iter().fold((f32::MAX, f32::MIN), |(a, b), p| {
        (a.min(p.income), b.max(p.income))
    });
    let (ymin, ymax) = data.iter().fold((f32::MAX, f32::MIN), |(a, b), p| {
        (a.min(p.spend), b.max(p.spend))
    });

    let t1 = Instant::now();
    let mut chart = scatter()
        .x(AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.income)))
        .y(AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.spend)))
        // Categorical -> black on scatter; fill_opacity is a no-op; size is
        // "percent of plot width" (undocumented). Map everything by hand.
        .color(AccessorFunction::new(|p: &Pt| AccessorValue::Color(seg_color(&p.segment))))
        .size(AccessorFunction::new(|p: &Pt| AccessorValue::Float(0.04 * p.weight)))
        .x_scale(LogScale::base10(xmin, xmax, -1.0, 1.0))
        .y_scale(LinearScale::new(ymin, ymax, -1.0, 1.0))
        .x_tick_format(CustomFormatter::new(|v| format!("${:.0}k", v / 1000.0)))
        .title("Spend vs income (200k customers)")
        .width(1000.0)
        .height(700.0)
        .show_axes(true)
        .grid()
        .build_with_data(data, ctx)?;
    println!("built in {:?}", t1.elapsed());
    // The Circle mark's default black stroke is wider than any small NDC
    // radius, so every point renders solid black. The builder has no stroke
    // API; reach into the public `visualization` field, zero it, re-prepare.
    chart.visualization.attr("stroke_width", |_: &Pt| 0.0f32);
    {
        let ctx = chart.visualization.context().cloned().unwrap();
        chart
            .visualization
            .prepare_render_bound(ctx.device(), ctx.queue(), None, None)?;
    }

    let t2 = Instant::now();
    chart.export_png("/tmp/gup-dogfood/t3_scatter.png", 1000, 700)?;
    println!("exported in {:?}", t2.elapsed());
    chart.export_svg(
        "/tmp/gup-dogfood/t3_scatter.svg",
        &gup::export::svg::SvgExportOptions::new(1000, 700),
    )?;
    Ok(())
}
