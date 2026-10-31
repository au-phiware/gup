// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless timing of `Plot::resolve` under the `zoom_bench` zoom (GUP-567):
//! 4,194,304 x-sorted rows in chunks of 65,536 (the rows of `zoom_bench
//! --points 4194304 --chunk-rows 65536 --sorted`), both domains set every
//! frame, resolved at 1920x1080 without a window. Prints
//! `metric edge.resolve.median_ms` and `metric edge.resolve.p95_ms`.
//!
//! ```text
//! cargo run -p gup-core --release --example edge_inset_bench
//! ```

#[path = "../tests/common/scatter.rs"]
mod scatter;

use gup_core::prelude::*;
use std::time::Instant;

const POINTS: usize = 4_194_304;
const CHUNK_ROWS: u32 = 65_536;
const WARMUP: usize = 60;
const FRAMES: usize = 600;
const SIZE: (f32, f32) = (1920.0, 1080.0);

fn main() {
    let mut rows = scatter::countries_n(POINTS);
    rows.sort_by(|a, b| a.gdp_per_capita.total_cmp(&b.gdp_per_capita));
    let (mut plot, x, y) = scatter::plot_rows_chunked(rows, Px(3.0), Some(CHUNK_ROWS));
    plot.chunk_culling(true);
    let cx = Context::new_blocking().expect("a GPU context");

    let mut full = None;
    let mut times = Vec::with_capacity(FRAMES);
    for frame in 0..WARMUP + FRAMES {
        // Frame 0 fits the data domains; every later frame zooms.
        if let Some(((x0, x1), (y0, y1))) = full {
            let t = frame as f64 / (WARMUP + FRAMES) as f64;
            let level = 0.5 * (1.0 - (std::f64::consts::TAU * 2.0 * t).cos());
            let k = 0.05f64.powf(level);
            let centre_x = x0 + 0.35 * (x1 - x0);
            let (ly0, ly1) = (f64::log10(y0), f64::log10(y1));
            let centre_y = ly0 + 0.6 * (ly1 - ly0);
            x.write()
                .set_domain(
                    centre_x - (centre_x - x0) * k,
                    centre_x + (x1 - centre_x) * k,
                )
                .expect("x domain");
            y.write()
                .set_domain(
                    10f64.powf(centre_y - (centre_y - ly0) * k),
                    10f64.powf(centre_y + (ly1 - centre_y) * k),
                )
                .expect("y domain");
        }
        let start = Instant::now();
        plot.resolve(&cx, SIZE.0, SIZE.1).expect("resolve");
        let elapsed = start.elapsed();
        if full.is_none() {
            full = Some((
                x.read().current_domain().unwrap(),
                y.read().current_domain().unwrap(),
            ));
        }
        if frame >= WARMUP {
            times.push(elapsed.as_secs_f64() * 1e3);
        }
    }
    times.sort_by(f64::total_cmp);
    let q = |p: f64| times[((times.len() - 1) as f64 * p).round() as usize];
    println!("metric edge.resolve.median_ms {:.3}", q(0.5));
    println!("metric edge.resolve.p95_ms {:.3}", q(0.95));
}
