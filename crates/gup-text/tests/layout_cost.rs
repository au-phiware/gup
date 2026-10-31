// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-frame cost of measuring the zoom benchmark's text (GUP-405): 26 tick
//! labels and the title, each measured four times a frame.
//!
//! ```text
//! cargo test -p gup-text --release --test layout_cost -- --ignored --nocapture
//! ```

use gup_text::Font;
use std::time::Instant;

#[test]
#[ignore = "a timing measurement; run with --release --ignored --nocapture"]
fn measuring_the_zoom_benchmark_text_per_frame() {
    let font = Font::inter();
    let mut labels: Vec<String> = (0..26).map(|i| (i * 100).to_string()).collect();
    labels.push("Zoom benchmark (100,000 points)".to_owned());
    let frame = |font: &Font| {
        let mut total = 0.0;
        for text in &labels {
            for _ in 0..4 {
                total += font.measure(text, 11.0).width;
            }
        }
        std::hint::black_box(total);
    };
    for _ in 0..10 {
        frame(&font);
    }
    let start = Instant::now();
    for _ in 0..1000 {
        frame(&font);
    }
    let per_frame = start.elapsed().as_secs_f64() * 1e6 / 1000.0;
    println!("layout_cost: {per_frame:.2} us per frame");
}
