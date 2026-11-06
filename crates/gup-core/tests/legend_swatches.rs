// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-616: a legend's swatch is the colour its marks are drawn in, so a
//! translucent mark's swatch is its colour composited over the
//! background, and one legend serves an area's fill and its outline.

use gup_core::Layout;
use gup_core::prelude::*;

/// The pixel at (`x`, `y`) in logical pixels, as RGB.
fn px(image: &image::RgbaImage, x: f64, y: f64) -> [u8; 3] {
    let p = image.get_pixel(x.floor() as u32, y.floor() as u32).0;
    [p[0], p[1], p[2]]
}

/// Where `value` falls along `ticks` (value, pixel), by interpolation.
fn at(ticks: &[(f64, f32)], value: f64) -> f64 {
    let (a, b) = (ticks[0], *ticks.last().unwrap());
    f64::from(a.1) + (value - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
}

/// The centre of `label`'s swatch: 12 px wide, then a 6 px gap, then the
/// label, whose anchor is level with the swatch's centre.
fn swatch_centre(layout: &Layout, label: &str) -> (f64, f64) {
    let t = layout
        .texts
        .iter()
        .find(|t| &*t.run.text == label)
        .unwrap_or_else(|| panic!("no legend entry {label}"));
    (f64::from(t.run.at.x) - 12.0, f64::from(t.run.at.y))
}

fn assert_near(what: &str, got: [u8; 3], want: [u8; 3]) {
    for c in 0..3 {
        assert!(
            got[c].abs_diff(want[c]) <= 2,
            "{what}: got {got:?}, wanted {want:?}"
        );
    }
}

#[test]
fn area_swatches_match_the_unoverlapped_fill() {
    let cx = Context::new_blocking().unwrap();
    // "web" is 8 high on the left and 2 on the right; "app" the reverse
    // (4 on the left), so each has a stretch the other does not cover.
    let rows: Vec<(f64, &str, f64)> = (0..=10)
        .flat_map(|i| {
            let x = f64::from(i);
            let left = x < 5.0;
            [
                (x, "web", if left { 8.0 } else { 2.0 }),
                (x, "app", if left { 4.0 } else { 8.0 }),
            ]
        })
        .collect();
    let mut chart = gup_core::area(rows)
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .legend_title("Source")
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y_scale(Linear::new().domain(0.0, 10.0));
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let (x, y) = (&layout.x_ticks, &layout.y_ticks);
    // Above "app" on the left, only "web" is drawn; above "web" on the
    // right, only "app".
    let web = px(&image, at(x, 2.0), at(y, 6.0));
    let app = px(&image, at(x, 8.0), at(y, 6.0));
    let (wx, wy) = swatch_centre(&layout, "web");
    let (ax, ay) = swatch_centre(&layout, "app");
    assert_near("web", px(&image, wx, wy), web);
    assert_near("app", px(&image, ax, ay), app);
}

#[test]
fn an_outlined_area_has_one_legend_matching_its_fill() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, &str, f64)> = (0..=10)
        .flat_map(|i| {
            let x = f64::from(i);
            [(x, "web", 8.0), (x, "app", 4.0)]
        })
        .collect();
    let mut chart = gup_core::area(rows)
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .outline(1.0)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y_scale(Linear::new().domain(0.0, 10.0));
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let (x, y) = (&layout.x_ticks, &layout.y_ticks);
    let count = |s: &str| layout.texts.iter().filter(|t| &*t.run.text == s).count();
    assert_eq!((count("web"), count("app")), (1, 1));
    let web = px(&image, at(x, 5.0), at(y, 6.0));
    let (wx, wy) = swatch_centre(&layout, "web");
    assert_near("web", px(&image, wx, wy), web);
}

#[test]
fn scatter_swatches_match_translucent_discs() {
    let cx = Context::new_blocking().unwrap();
    let rows = vec![(2.0, 5.0, "a"), (8.0, 5.0, "b")];
    let mut chart = gup_core::scatter(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .color(|r| r.2)
        .radius(10.0)
        .opacity(0.6)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y_scale(Linear::new().domain(0.0, 10.0));
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let (x, y) = (&layout.x_ticks, &layout.y_ticks);
    for (label, vx) in [("a", 2.0), ("b", 8.0)] {
        let disc = px(&image, at(x, vx), at(y, 5.0));
        let (sx, sy) = swatch_centre(&layout, label);
        assert_near(label, px(&image, sx, sy), disc);
    }
}
