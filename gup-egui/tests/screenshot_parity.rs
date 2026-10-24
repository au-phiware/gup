// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Colour parity of the example's egui screenshot with the chart's own
//! render: egui's shader must not re-encode what Gup wrote.
//!
//! The test is ignored: it needs the example's screenshot. Run
//! `mask egui-screenshot`, which runs the example headless with
//! `GUP_SCREENSHOT_PATH` and then this test with `GUP_EGUI_SCREENSHOT` set
//! (`cargo test -p gup-egui --test screenshot_parity -- --ignored`). Run
//! without the variable, the test panics rather than pass without comparing.

use gup_core::prelude::*;
use gup_core::{ChartExt, Context};
use gup_visual_regression::Rgba8;

/// The example's scatter (`examples/egui_chart.rs`), 420 × 320 points.
fn chart() -> gup_core::ScatterChart<(f64, f64)> {
    let pts = (0..80).map(|i| (i as f64 * 0.5 % 10.0, ((i * 7) % 20) as f64 / 2.0));
    gup_core::scatter(pts.collect::<Vec<_>>())
        .x(|p| p.0)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y(|p| p.1)
        .y_scale(Linear::new().domain(0.0, 10.0))
        .fill(Color::hex(0x1f77b4))
        .radius(5.0)
        .tooltip(|p| format!("x {:.1}, y {:.1}", p.0, p.1))
        .title("Scatter (hover for tooltips)")
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
#[ignore = "needs the example's screenshot: run mask egui-screenshot"]
fn screenshot_marks_match_the_charts_own_render() {
    let path = std::env::var("GUP_EGUI_SCREENSHOT")
        .expect("GUP_EGUI_SCREENSHOT is not set: run mask egui-screenshot");
    let shot = image::open(path).unwrap().to_rgba8();
    let cx = Context::new_blocking().unwrap();
    let (reference, layout) = chart().render_resolved(&cx, 420, 320).unwrap();

    // Where egui put the chart: the offset with the smallest difference.
    let diff = |ox: u32, oy: u32| -> u64 {
        let mut sum = 0u64;
        for y in (0..320).step_by(3) {
            for x in (0..420).step_by(3) {
                let (a, b) = (px(&reference, x, y), px(&shot, x + ox, y + oy));
                sum +=
                    a.r.abs_diff(b.r) as u64 + a.g.abs_diff(b.g) as u64 + a.b.abs_diff(b.b) as u64;
            }
        }
        sum
    };
    let (ox, oy) = (0..=60u32)
        .flat_map(|y| (0..=60u32).map(move |x| (x, y)))
        .filter(|&(x, y)| x + 420 <= shot.width() && y + 320 <= shot.height())
        .min_by_key(|&(x, y)| diff(x, y))
        .unwrap();
    let samples = (105 * 107) as u64;
    assert!(
        diff(ox, oy) / samples <= 1,
        "no offset of the screenshot matches the chart (best {ox},{oy})"
    );

    // Three mark centres (data points 3, 21 and 44).
    for i in [3usize, 21, 44] {
        let (dx, dy) = (i as f64 * 0.5 % 10.0, ((i * 7) % 20) as f64 / 2.0);
        let (x, y) = (
            at(&layout.x_ticks, dx) as u32,
            at(&layout.y_ticks, dy) as u32,
        );
        let (want, got) = (px(&reference, x, y), px(&shot, x + ox, y + oy));
        assert_eq!(want, Rgba8::from_hex(0x1f77b4), "reference mark {i}");
        let de = want.delta_e(got);
        assert!(
            de <= 2.0,
            "mark {i} at ({x},{y}): {want:?} vs {got:?}, ΔE {de}"
        );
    }
}
