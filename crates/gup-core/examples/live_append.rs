// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A line that a thread feeds: one row every 50 ms for 3 s, through the
//! chart's `appender()`. The window wakes as rows arrive.
//!
//! ```text
//! cargo run -p gup-core --example live_append
//! ```

use std::time::Duration;

fn main() -> gup_core::Result<()> {
    let mut chart = gup_core::line(vec![(0.0_f64, 0.0_f64)])
        .x(|p| p.0)
        .y(|p| p.1)
        .x_scale(gup_core::Linear::new().domain(0.0, 3.0))
        .y_scale(gup_core::Linear::new().domain(-1.5, 1.5))
        .stroke(gup_core::Color::hex(0xd62728))
        .width(2.0);
    let feed = chart.appender();
    std::thread::spawn(move || {
        for i in 1..=60 {
            std::thread::sleep(Duration::from_millis(50));
            let t = f64::from(i) * 0.05;
            if feed.push((t, (t * 4.0).sin())).is_err() {
                break;
            }
        }
    });
    gup_core::show(chart)
}
