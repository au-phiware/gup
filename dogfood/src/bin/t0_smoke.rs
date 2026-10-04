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
        .build_with_data(data.clone(), ctx.clone())?;
    chart.export_png("/tmp/gup-dogfood/t0_smoke_builder.png", 800, 500)?;

    // Same chart via plot() + field names (README style).
    let r = plot()
        .with_context(ctx.clone())
        .data(data)
        .scatter(x("x"), y("y"))
        .title("plot() api")
        .build_async()
        .await;
    match r {
        Ok(mut c) => {
            c.export_png("/tmp/gup-dogfood/t0_smoke_plot.png", 800, 500)?;
            println!("plot() ok");
        }
        Err(e) => println!("plot() error: {e}"),
    }
    Ok(())
}
