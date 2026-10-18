// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 2: grouped + stacked bar chart (region x quarter) with legend,
//! exported to PNG. Value labels are not available and are not checked.

use gup_core::prelude::*;

struct Sale {
    region: String,
    quarter: String,
    sales: f64,
}

const PALETTE: [u32; 4] = [0x1f77b4, 0xff7f0e, 0x2ca02c, 0xd62728];

fn load() -> Result<Vec<Sale>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path("/tmp/gup-dogfood/sales.csv")?;
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec?;
        out.push(Sale {
            region: rec[0].into(),
            quarter: rec[1].into(),
            sales: rec[2].parse()?,
        });
    }
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let data = load()?;
    let cx = Context::new_blocking()?;
    for stacked in [false, true] {
        let name = if stacked { "stacked" } else { "grouped" };
        let rows = data.iter().map(|s| Sale {
            region: s.region.clone(),
            quarter: s.quarter.clone(),
            sales: s.sales,
        });
        let chart = gup_core::bar(rows)
            .x(|d| d.quarter.as_str())
            .y(|d| d.sales)
            .color(|d| d.region.as_str())
            .color_scale(Categorical::from_palette(PALETTE.map(Color::hex)))
            .padding(0.15)
            .title(format!("Sales by quarter and region ({name})"));
        let mut chart = if stacked {
            chart.stack(|d| d.region.as_str())
        } else {
            chart.group(|d| d.region.as_str())
        };
        chart.save_png(&cx, format!("/tmp/gup-dogfood/t2_{name}.png"), 800, 500)?;
    }
    println!("wrote t2_grouped.png, t2_stacked.png");
    Ok(())
}
