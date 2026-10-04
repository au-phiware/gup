// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Control: the simplest possible chart -> PNG, to check text/axes render.

use gup::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct P {
    x: f32,
    y: f32,
}

#[tokio::main]
async fn main() -> GupResult<()> {
    env_logger::init();
    let ctx = Arc::new(RenderContext::new().await?);
    let data: Vec<P> = (0..20).map(|i| P { x: i as f32, y: (i * i) as f32 }).collect();

    let mut chart = scatter()
        .x(AccessorFunction::new(|p: &P| AccessorValue::Float(p.x)))
        .y(AccessorFunction::new(|p: &P| AccessorValue::Float(p.y)))
        .title("Smoke test")
        .show_axes(true)
        .build_with_data(data, ctx)?;
    chart.export_png("/tmp/gup-dogfood/t0_smoke_builder.png", 800, 500)?;
    // The audit also tried the README-style `plot().scatter(x("x"), y("y"))`
    // route: it "succeeded" but drew all 20 points on one spot (field
    // accessors evaluated to 0.0). That API is being deleted (GUP-389), so
    // the suite no longer exercises it.
    Ok(())
}
