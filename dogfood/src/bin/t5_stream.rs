// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 5 (alt): push points in batches through a scatter chart's
//! `appender()` and export a PNG after each batch (GUP-480). The chart is
//! built once, empty; each batch is evaluated and uploaded when the next
//! render resolves.

use gup::ChartExt;
use gup::prelude::*;

/// A sample: its index and a sine of it.
struct S {
    i: f64,
    v: f64,
}

fn sample(i: u32) -> S {
    let i = f64::from(i);
    S {
        i,
        v: (12.0 * i / 200.0).sin() * 0.7,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cx = Context::new_blocking()?;
    let mut chart = gup::scatter(Vec::<S>::new())
        .x(|s| s.i)
        .y(|s| s.v)
        .fill(Color::hex(0xd62728))
        .radius(3.0);
    let feed = chart.appender();

    for batch in 0..3 {
        feed.extend((batch * 50..(batch + 1) * 50).map(sample))?;
        chart.save_png(
            &cx,
            format!("/tmp/gup-dogfood/t5_stream_{batch}.png"),
            400,
            300,
        )?;
        println!(
            "batch {batch}: rendered, needs_frame {}",
            chart.needs_frame()
        );
    }
    Ok(())
}
