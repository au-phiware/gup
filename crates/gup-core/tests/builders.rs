// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-462: the scatter builder is thin sugar over `Plot`: byte-identical
//! to the hand-built reference, sensible defaults for colour and size, and
//! escape hatches that materialise the layer.

mod common;

use common::scatter::{HEIGHT, TITLE, WIDTH, countries, plot};
use common::vr::{harness, layout_metadata};
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{Layout, OKABE_ITO};
use gup_visual_regression::RgbaImage;

#[derive(Clone, Debug)]
struct Row {
    x: f64,
    y: f64,
    segment: String,
    weight: f64,
}

/// 300 deterministic rows. The first 298 sit at x in 10..70 so that the
/// smallest disc (weight 0, at x = 85, y = 25) and the largest (weight 100,
/// at x = 85, y = 65) are isolated and can be measured.
fn rows() -> Vec<Row> {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let name = |i: usize| ["Alpha", "Bravo", "Charlie", "Delta"][i % 4].to_string();
    let mut rows: Vec<Row> = (0..298)
        .map(|i| Row {
            x: 10.0 + next() * 60.0,
            y: 10.0 + next() * 80.0,
            segment: name(i),
            weight: 5.0 + next() * 40.0,
        })
        .collect();
    rows.push(Row {
        x: 85.0,
        y: 25.0,
        segment: name(1),
        weight: 0.0,
    });
    rows.push(Row {
        x: 85.0,
        y: 65.0,
        segment: name(2),
        weight: 100.0,
    });
    rows
}

fn builder() -> gup_core::ScatterChart<Row> {
    gup_core::scatter(rows())
        .x(|r| r.x)
        .y(|r| r.y)
        .color(|r| r.segment.as_str())
        .size(|r| r.weight)
        .opacity(0.7)
        .legend_title("Segment")
        .grid(Axes::Both)
        .title("Four segments, sized by weight")
}

fn at(ticks: &[(f64, f32)], value: f64) -> f64 {
    let (a, b) = (ticks[0], *ticks.last().unwrap());
    f64::from(a.1) + (value - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
}

fn centre(layout: &Layout, r: &Row) -> (f64, f64) {
    (at(&layout.x_ticks, r.x), at(&layout.y_ticks, r.y))
}

fn px(image: &image::RgbaImage, x: f64, y: f64) -> [u8; 3] {
    let p = image.get_pixel(x.floor() as u32, y.floor() as u32).0;
    [p[0], p[1], p[2]]
}

/// A disc's radius, from the coverage along the row through its centre.
fn radius(image: &image::RgbaImage, (cx, cy): (f64, f64)) -> f64 {
    let mid = px(image, cx, cy);
    let ch = (0..3).min_by_key(|&c| mid[c]).unwrap();
    let ink = f64::from(255 - mid[ch]);
    let total: f64 = (-20..=20)
        .map(|d| {
            let v = px(image, (cx + f64::from(d)).max(0.0), cy)[ch];
            (f64::from(255 - v) / ink).clamp(0.0, 1.0)
        })
        .sum();
    total / 2.0
}

fn over_white(c: Color, opacity: f32) -> [u8; 3] {
    let [r, g, b, _] = c.to_rgba8();
    [r, g, b].map(|v| (f32::from(v) * opacity + 255.0 * (1.0 - opacity)).round() as u8)
}

fn assert_near(got: [u8; 3], want: [u8; 3]) {
    for c in 0..3 {
        assert!(
            got[c].abs_diff(want[c]) <= 3,
            "got {got:?}, wanted {want:?}"
        );
    }
}

#[test]
fn byte_identical_to_the_reference_scatter() {
    let cx = Context::new_blocking().unwrap();
    let rows = countries();
    let mut chart = gup_core::scatter(rows)
        .x(|c| c.gdp_per_capita)
        .y(|c| c.population)
        .y_scale(Log::new())
        .color_value(|c| c.life_expectancy)
        .radius(4.5)
        .title(TITLE)
        .legends(false);
    let (built, built_layout) = chart.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let (reference, layout) = plot().render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    assert!(built.as_raw() == reference.as_raw(), "pixels differ");
    assert_eq!(format!("{built_layout:?}"), format!("{layout:?}"));

    let mut again = chart.into_plot();
    let resolved = again.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    assert_eq!(format!("{:?}", resolved.layout), format!("{layout:?}"));
}

#[test]
fn defaults_render_a_titled_legend_and_scaled_discs() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = builder();
    let (image, layout) = chart.render_resolved(&cx, WIDTH, HEIGHT).unwrap();

    let all = rows();
    let (small, large) = (&all[298], &all[299]);
    let (rs, rl) = (
        radius(&image, centre(&layout, small)),
        radius(&image, centre(&layout, large)),
    );
    assert!((rs - 2.0).abs() <= 0.5, "smallest radius {rs}");
    assert!((rl - 12.0).abs() <= 0.5, "largest radius {rl}");

    // Segment colours: first-seen order takes the palette in order.
    for (r, seg) in [(small, 1), (large, 2)] {
        let (cx_, cy) = centre(&layout, r);
        assert_near(px(&image, cx_, cy), over_white(OKABE_ITO[seg], 0.7));
    }
    assert!(layout.texts.iter().any(|t| &*t.run.text == "Segment"));

    let vr = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    harness()
        .run(
            "gup_core/builder_scatter",
            Ok((vr, layout_metadata(&layout))),
        )
        .assert_ok();
}

#[test]
fn color_scale_applies_in_either_order() {
    let cx = Context::new_blocking().unwrap();
    let palette = [0xaa0000, 0x00aa00, 0x0000aa, 0xaaaa00].map(Color::hex);
    let key = |r: &Row| -> String { r.segment.clone() };
    let _ = key;
    let before = gup_core::scatter(rows())
        .color_scale(Categorical::from_palette(palette))
        .color(|r| r.segment.as_str());
    let after = gup_core::scatter(rows())
        .color(|r| r.segment.as_str())
        .color_scale(Categorical::from_palette(palette));
    let all = rows();
    for chart in [before, after] {
        let mut chart = chart.x(|r| r.x).y(|r| r.y).radius(6.0);
        let (image, layout) = chart.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
        let (x, y) = centre(&layout, &all[299]);
        assert_near(px(&image, x, y), over_white(palette[2], 1.0));
    }
}

#[test]
fn layer_mut_strokes_the_discs() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::scatter(rows()).x(|r| r.x).y(|r| r.y).radius(10.0);
    chart.layer_mut().stroke(Color::BLACK).stroke_width(Px(1.0));
    let (image, layout) = chart.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let (x, y) = centre(&layout, &rows()[299]);
    let edge = px(&image, x - 9.5, y);
    assert!(edge.iter().all(|&v| v < 90), "edge pixel {edge:?}");
    assert!(px(&image, x, y).iter().any(|&v| v > 90), "centre is filled");
}

#[test]
fn pick_returns_the_row_under_a_disc() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = builder();
    let layout = chart.render_resolved(&cx, WIDTH, HEIGHT).unwrap().1;
    let all = rows();
    let (x, y) = centre(&layout, &all[299]);
    let hit = chart.pick(Point::new(x as f32, y as f32)).expect("a row");
    assert_eq!(hit.weight, 100.0);
    let empty = Point::new(layout.plot.left() + 3.0, layout.plot.top() + 3.0);
    assert!(chart.pick(empty).is_none());
}

#[test]
#[should_panic(
    expected = "set layer encodings before the chart is first drawn; change a drawn chart through layer_mut()"
)]
fn layer_setters_panic_after_the_first_draw() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::scatter(rows()).x(|r| r.x).y(|r| r.y);
    chart.render(&cx, 200, 150).unwrap();
    let _ = chart.x(|r| r.y);
}

#[test]
fn plot_setters_work_after_the_first_draw() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::scatter(rows()).x(|r| r.x).y(|r| r.y);
    let first = chart.render(&cx, 300, 200).unwrap();
    let mut chart = chart.title("Now with a title");
    let second = chart.render(&cx, 300, 200).unwrap();
    assert!(first.as_raw() != second.as_raw());
}

#[test]
#[should_panic(expected = "opacity must be within 0..=1")]
fn opacity_outside_the_unit_range_panics() {
    let _ = gup_core::scatter(rows()).opacity(1.5);
}
