// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 10 of the tour: rows that arrive after the chart is built, and a
//! window that keeps only the newest of them.

use gup::prelude::*;

struct Sample {
    /// Seconds.
    t: f64,
    level: f64,
}

/// The sample taken `i` times in: two sine waves, summed.
fn sample(i: u32) -> Sample {
    let t = f64::from(i) * 0.05;
    Sample {
        t,
        level: (t * 3.0).sin() + 0.3 * (t * 11.0).sin(),
    }
}

/// The reading taken `i` times in: a slow wave and a little jitter.
fn reading(i: u32) -> Sample {
    let t = f64::from(i) * 0.05;
    let jitter = f64::from(i * 7919 % 100) / 100.0 - 0.5;
    Sample {
        t,
        level: (t * 0.1).sin() + 0.3 * jitter,
    }
}

pub fn append() -> gup::Result<impl Chart> {
    // tour:begin append
    let mut chart = gup::line(Vec::<Sample>::new())
        .x(|s| s.t)
        .y(|s| s.level)
        .stroke(Color::hex(0xd62728))
        .width(2.0)
        .title("Signal")
        .x_title("Time (s)")
        .y_title("Level");
    let feed = chart.appender();

    // Three batches of 40 rows. Another thread could send them.
    for batch in 0..3 {
        feed.extend((0..40).map(|i| sample(batch * 40 + i)))
            .expect("the chart is alive");
    }
    chart.resolve(&Context::shared()?, 640.0, 400.0)?;
    Ok(chart)
    // tour:end append
}

pub fn window() -> gup::Result<impl Chart> {
    // tour:begin window
    let cx = Context::shared()?;
    let mut chart = gup::scatter(Vec::<Sample>::new())
        .x(|s| s.t)
        .y(|s| s.level)
        .radius(2.0)
        .title("A window on the newest readings")
        .x_title("Time (s)")
        .y_title("Level")
        .window(Window::LastRows(2_000));
    chart.layer_mut().retain(Retain::GpuOnly);
    let feed = chart.appender();

    for batch in 0..25 {
        feed.extend((0..2_000).map(|i| reading(batch * 2_000 + i)))
            .expect("the chart is alive");
        chart.resolve(&cx, 640.0, 400.0)?;
    }
    // 50,000 rows came in. Rows leave a chunk (1,024 rows or more) at a
    // time, so the layer holds the 2,000 newest and up to two chunks more.
    let held = chart.layer_mut().len();
    assert!((2_000..=2_000 + 2 * 1_024).contains(&held), "{held}");
    Ok(chart)
    // tour:end window
}

fn main() -> gup::Result<()> {
    gup::show(window()?)
}
