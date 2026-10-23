// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Colour parity of the example's Bevy screenshot with the chart's own
//! render: Bevy's sprite shader must not re-encode what Gup wrote.
//!
//! Run the example with `GUP_SCREENSHOT_PATH=/tmp/x.png` (under `xvfb-run`),
//! then `GUP_BEVY_SCREENSHOT=/tmp/x.png cargo test -p gup-bevy --test
//! screenshot_parity`. Without the variable the test has nothing to compare
//! and passes trivially.

use gup_core::prelude::*;
use gup_core::{ChartExt, Context};
use gup_visual_regression::Rgba8;

/// The example's chart before rows are appended (`examples/bevy_scatter.rs`).
fn chart() -> gup_core::ScatterChart<(f64, f64)> {
    let pts: Vec<(f64, f64)> = (0..20)
        .map(|i| (i as f64 * 0.5, (i * 7 % 10) as f64 / 2.0))
        .collect();
    gup_core::scatter(pts)
        .x(|p| p.0)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y(|p| p.1)
        .y_scale(Linear::new().domain(0.0, 10.0))
        .fill(Color::hex(0x1f77b4))
        .radius(5.0)
        .title("Bevy scatter")
}

fn px(image: &image::RgbaImage, x: u32, y: u32) -> Rgba8 {
    let p = image.get_pixel(x, y).0;
    Rgba8 {
        r: p[0],
        g: p[1],
        b: p[2],
        a: p[3],
    }
}

/// Pixel position of `v` on an axis, from its first and last tick.
fn at(ticks: &[(f64, f32)], v: f64) -> f32 {
    let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
    a.1 + (b.1 - a.1) * ((v - a.0) / (b.0 - a.0)) as f32
}

#[test]
fn screenshot_marks_match_the_charts_own_render() {
    let Ok(path) = std::env::var("GUP_BEVY_SCREENSHOT") else {
        eprintln!("GUP_BEVY_SCREENSHOT not set: nothing to compare");
        return;
    };
    let shot = image::open(path).unwrap().to_rgba8();
    let cx = Context::new_blocking().unwrap();
    let (reference, layout) = chart().render_resolved(&cx, 480, 320).unwrap();

    // The 480 x 320 sprite is centred in the example's 640 x 480 window.
    let (ox, oy) = ((shot.width() - 480) / 2, (shot.height() - 320) / 2);

    // Three mark centres (initial points 3, 11 and 17).
    for i in [3usize, 11, 17] {
        let (dx, dy) = (i as f64 * 0.5, (i * 7 % 10) as f64 / 2.0);
        let (x, y) = (
            at(&layout.x_ticks, dx) as u32,
            at(&layout.y_ticks, dy) as u32,
        );
        let (want, got) = (px(&reference, x, y), px(&shot, x + ox, y + oy));
        assert_eq!(want, Rgba8::from_hex(0x1f77b4), "reference mark {i}");
        let de = want.delta_e(got);
        eprintln!("mark {i}: {want:?} vs {got:?}, ΔE {de}");
        assert!(
            de <= 2.0,
            "mark {i} at ({x},{y}): {want:?} vs {got:?}, ΔE {de}"
        );
    }
}
