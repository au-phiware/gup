// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 5 (alt): Tutorial 5's DataStream -> Selection::stream route. Push
//! points in batches, flush, and export a PNG after each batch.

use gup::mark::circle::CircleInstance;
use gup::prelude::*;
use gup::streaming::{BackpressureStrategy, DataStream, StreamMode};
use std::sync::Arc;

fn inst(i: usize) -> CircleInstance {
    let t = i as f32 / 200.0;
    CircleInstance {
        center: [t * 1.8 - 0.9, (t * 12.0).sin() * 0.7],
        radius: 0.01,
        _pad0: 0.0,
        fill_color: [0.84, 0.15, 0.16, 1.0],
        stroke_width: 0.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0; 4],
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let ctx = Arc::new(RenderContext::new().await?);
    let stream = DataStream::<CircleInstance>::builder()
        .capacity(200)
        .mode(StreamMode::SlidingWindow)
        .backpressure(BackpressureStrategy::EvictOldest)
        .build(ctx.device())?;
    let mut sel = Selection::<CircleInstance, Circle>::new(vec![], ctx.clone())?;
    sel.stream(stream);
    let config = gup::chart_builder::ChartConfig {
        show_axes: false,
        ..Default::default()
    };
    let mut chart = gup::chart_builder::ComposedChart::new(sel, config);

    for batch in 0..3 {
        {
            let s = chart
                .visualization
                .stream_mut::<CircleInstance>()
                .ok_or("no stream")?;
            s.push_batch((batch * 50..(batch + 1) * 50).map(inst).collect());
            let n = s.flush(ctx.device(), ctx.queue());
            println!("batch {batch}: flushed {n} bytes, stream len {}", s.len());
        }
        let r = chart
            .visualization
            .prepare_render_bound(ctx.device(), ctx.queue(), None, None);
        println!(
            "  prepare_render_bound: {:?}; render ready: {}",
            r.map(|_| ()),
            chart.visualization.is_render_ready()
        );
        chart.export_png(format!("/tmp/gup-dogfood/t5_stream_{batch}.png"), 400, 300)?;
    }
    Ok(())
}
