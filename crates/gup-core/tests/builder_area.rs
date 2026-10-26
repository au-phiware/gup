// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-547: the area builder is sugar over `Plot` with `Area` layers: a
//! single area is byte-identical to the hand-built plot, `color` overlaps
//! translucent areas, `stack` aligns series so layers meet exactly, and
//! picking and tooltips speak in the user's rows.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, Z_TOOLTIP};
use gup_core::{Input, Layout, OKABE_ITO};
use gup_visual_regression::RgbaImage;

/// x, series, value.
type Row = (f64, &'static str, Option<f64>);

fn at(ticks: &[(f64, f32)], value: f64) -> f64 {
    let (a, b) = (ticks[0], *ticks.last().unwrap());
    f64::from(a.1) + (value - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
}

fn px(image: &image::RgbaImage, x: f64, y: f64) -> [u8; 3] {
    let p = image.get_pixel(x.floor() as u32, y.floor() as u32).0;
    [p[0], p[1], p[2]]
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

/// Six x values 0, 2, .., 10. A zig-zags between 4 and 6; B is 3 and C is 2.
/// B has no row at x = 4 and C none at x = 8.
fn stacked_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for i in 0..6 {
        let x = f64::from(i) * 2.0;
        rows.push((x, "A", Some(if i % 2 == 0 { 4.0 } else { 6.0 })));
        if i != 2 {
            rows.push((x, "B", Some(3.0)));
        }
        if i != 4 {
            rows.push((x, "C", Some(2.0)));
        }
    }
    rows
}

fn stack_chart(rows: Vec<Row>) -> gup_core::AreaChart<Row> {
    gup_core::area(rows)
        .x(|r| r.0)
        .y(|r| r.2)
        .stack(|r| r.1)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y_scale(Linear::new().domain(0.0, 14.0))
}

#[test]
fn single_area_is_byte_identical_to_the_reference_plot() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (0..12)
        .map(|i| (f64::from(i), 20.0 + f64::from(i * 7 % 11)))
        .collect();
    let mut chart = gup_core::area(rows.clone())
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Rain");
    let (built, built_layout) = chart.render_resolved(&cx, 640, 400).unwrap();

    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.title("Rain");
    plot.area(rows)
        .attr(Area::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Area::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Area::Y_END, y.encode(|_: &(f64, f64)| 0.0));
    let (reference, layout) = plot.render_resolved(&cx, 640, 400).unwrap();
    assert!(built.as_raw() == reference.as_raw(), "pixels differ");
    assert_eq!(format!("{built_layout:?}"), format!("{layout:?}"));
}

#[test]
fn color_gives_translucent_overlapping_areas_and_a_legend() {
    let cx = Context::new_blocking().unwrap();
    // "web" is 8 high everywhere and "app" is 4 high, over x in 0..10.
    let rows: Vec<Row> = (0..=10)
        .flat_map(|i| {
            let x = f64::from(i);
            [(x, "web", Some(8.0)), (x, "app", Some(4.0))]
        })
        .collect();
    let mut chart = gup_core::area(rows)
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .legend_title("Source")
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y_scale(Linear::new().domain(0.0, 10.0));
    assert_eq!(chart.layers_mut().len(), 2);
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let (x, y) = (&layout.x_ticks, &layout.y_ticks);
    // Only "web" above 4; both below.
    let only_web = px(&image, at(x, 5.0), at(y, 6.0));
    assert_near(only_web, over_white(OKABE_ITO[0], 0.5));
    let both = px(&image, at(x, 5.0), at(y, 2.0));
    let over_web = over_white(OKABE_ITO[0], 0.5).map(f32::from);
    let want = [0, 1, 2].map(|c| {
        let [r, g, b, _] = OKABE_ITO[1].to_rgba8();
        let src = [r, g, b][c] as f32;
        (src * 0.5 + over_web[c] * 0.5).round() as u8
    });
    assert_near(both, want);
    let text = |s: &str| layout.texts.iter().any(|t| &*t.run.text == s);
    assert!(text("Source") && text("web") && text("app"));
}

/// The tops of A, B and C at an odd x, mid-way between two even x values,
/// where a missing row (B at 4, C at 8) counts as 0.
fn tops(x: f64) -> [f64; 3] {
    let value = |series: &str, x: f64| -> f64 {
        stacked_rows()
            .iter()
            .find(|r| r.1 == series && r.0 == x)
            .and_then(|r| r.2)
            .unwrap_or(0.0)
    };
    let (lo, hi) = (x - 1.0, x + 1.0);
    let mid = |series: &str| (value(series, lo) + value(series, hi)) / 2.0;
    let a = mid("A");
    [a, a + mid("B"), a + mid("B") + mid("C")]
}

#[test]
fn stacked_layers_meet_with_no_gap_or_overlap() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = stack_chart(stacked_rows());
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let (xt, yt) = (&layout.x_ticks, &layout.y_ticks);
    let colour = |i: usize| over_white(OKABE_ITO[i], 1.0);
    // Mid-way along segments, where the aligned edges are slanted.
    for x in [1.0, 3.0, 5.0, 7.0, 9.0] {
        let [a_top, b_top, c_top] = tops(x);
        let col = at(xt, x);
        // The pixel just above layer 1's top is layer 2's, and just below
        // layer 2's base is layer 1's (within a pixel).
        let edge = at(yt, a_top);
        assert_near(px(&image, col, edge - 2.0), colour(1));
        assert_near(px(&image, col, edge + 2.0), colour(0));
        let edge = at(yt, b_top);
        assert_near(px(&image, col, edge - 2.0), colour(2));
        assert_near(px(&image, col, edge + 2.0), colour(1));
        // Above C is white; between the baseline and C there is nothing but
        // the three colours (no white seam, no mixed pixel).
        assert_near(px(&image, col, at(yt, c_top) - 2.0), [255, 255, 255]);
        let (lo, hi) = (at(yt, c_top) + 2.0, at(yt, 0.0) - 2.0);
        let mut y = lo;
        while y < hi {
            let p = px(&image, col, y);
            let near = |c: [u8; 3]| (0..3).all(|k| p[k].abs_diff(c[k]) <= 3);
            let at_edge = [at(yt, a_top), at(yt, b_top)]
                .iter()
                .any(|e| (y - e).abs() < 1.5);
            assert!(
                at_edge || near(colour(0)) || near(colour(1)) || near(colour(2)),
                "x = {x}, y = {y}: {p:?}"
            );
            y += 1.0;
        }
    }
    // The baseline is the bottom of the plot.
    assert_near(px(&image, at(xt, 3.0), at(yt, 0.0) - 2.0), colour(0));
}

#[test]
fn unsorted_stacked_rows_draw_as_sorted_ones() {
    let cx = Context::new_blocking().unwrap();
    let sorted = stacked_rows();
    // Each series reversed, the series in the same first-seen order.
    let shuffled: Vec<Row> = ["A", "B", "C"]
        .into_iter()
        .flat_map(|name| sorted.iter().rev().filter(move |r| r.1 == name).copied())
        .collect();
    let a = stack_chart(sorted).render(&cx, 400, 300).unwrap();
    let b = stack_chart(shuffled).render(&cx, 400, 300).unwrap();
    assert!(a.as_raw() == b.as_raw(), "pixels differ");
}

#[test]
fn unstacked_series_keep_data_order() {
    // The same points in another order make another outline.
    let cx = Context::new_blocking().unwrap();
    let draw = |rows: Vec<(f64, f64)>| {
        gup_core::area(rows)
            .x(|r| r.0)
            .y(|r| r.1)
            .x_scale(Linear::new().domain(0.0, 4.0))
            .y_scale(Linear::new().domain(0.0, 4.0))
            .render(&cx, 200, 150)
            .unwrap()
    };
    let ordered = draw(vec![(0.0, 1.0), (2.0, 3.0), (4.0, 1.0)]);
    let swapped = draw(vec![(0.0, 1.0), (4.0, 1.0), (2.0, 3.0)]);
    assert!(ordered.as_raw() != swapped.as_raw());
    let mut chart = gup_core::area(vec![(2.0, 1.0), (0.0, 1.0)])
        .x(|r| r.0)
        .y(|r| r.1);
    assert_eq!(chart.layers_mut()[0].len(), 2);
}

#[test]
fn outline_is_the_opaque_series_colour_along_the_top() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = stack_chart(stacked_rows()).outline(3.0).opacity(0.3);
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let (xt, yt) = (&layout.x_ticks, &layout.y_ticks);
    // On each layer's top edge the pixel is the opaque colour, though the
    // fills are translucent.
    for (i, top) in tops(3.0).into_iter().enumerate() {
        assert_near(
            px(&image, at(xt, 3.0), at(yt, top) + 0.5),
            over_white(OKABE_ITO[i], 1.0),
        );
    }
    // Inside a layer, away from edges, the fill is translucent.
    assert_near(
        px(&image, at(xt, 3.0), at(yt, 2.5)),
        over_white(OKABE_ITO[0], 0.3),
    );
    // Only the areas are layers of the area type.
    assert_eq!(chart.layers_mut().len(), 3);
}

#[test]
fn pick_and_tooltips_speak_in_user_rows() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = stack_chart(stacked_rows()).tooltip(|r| format!("{} at {}", r.1, r.0));
    let (_, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    let pt =
        |x: f64, y: f64| Point::new(at(&layout.x_ticks, x) as f32, at(&layout.y_ticks, y) as f32);
    // Layer B spans 5..8 around x = 2.5; the nearer point is B's own row at 2.
    let row = chart.pick(pt(1.6, 6.5)).copied();
    assert_eq!(row.map(|r| (r.0, r.1)), Some((2.0, "B")));
    // Layer B has no row at x = 4: a pointer nearer 4 than 2 finds an
    // aligned point, which is no user row.
    assert!(chart.pick(pt(3.6, 4.7)).is_none());
    // Layer A at x = 3.6 is nearer its row at 4.
    assert_eq!(
        chart.pick(pt(3.6, 2.0)).map(|r| (r.0, r.1)),
        Some((4.0, "A"))
    );
    // Off the areas.
    assert!(chart.pick(pt(5.0, 13.0)).is_none());

    let tooltip = |chart: &mut gup_core::AreaChart<Row>, at: Point| -> Vec<String> {
        let _ = chart.handle(Input::PointerMoved { at });
        let resolved = chart.resolve(&cx, 640.0, 400.0).unwrap();
        resolved
            .scene
            .items
            .iter()
            .filter(|i| i.z == Z_TOOLTIP)
            .filter_map(|i| match &i.kind {
                ItemKind::Text(runs) => Some(runs),
                _ => None,
            })
            .flatten()
            .map(|r| r.text.to_string())
            .collect()
    };
    assert_eq!(tooltip(&mut chart, pt(1.6, 6.5)), ["B at 2"]);
    assert!(tooltip(&mut chart, pt(3.6, 4.7)).is_empty());
}

#[test]
fn last_colour_call_wins_and_layer_setters_panic_once_drawn() {
    let cx = Context::new_blocking().unwrap();
    let rows = stacked_rows();
    let mut one = gup_core::area(rows.clone())
        .x(|r| r.0)
        .y(|r| r.2)
        .stack(|r| r.1)
        .fill(Color::hex(0xcc0000));
    assert_eq!(one.layers_mut().len(), 1);
    let image = one.render(&cx, 200, 150).unwrap();
    assert!(image.pixels().any(|p| p.0 == [0xcc, 0, 0, 0xff]));

    let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = one.outline(1.0);
    }));
    assert!(drawn.is_err());
}

fn months() -> Vec<(f64, &'static str, f64)> {
    let mut rows = Vec::new();
    for m in 0..24 {
        let t = f64::from(m);
        let season = (t * std::f64::consts::TAU / 12.0).sin();
        rows.push((t, "Web", 40.0 + 12.0 * season + t));
        rows.push((t, "Mobile", 25.0 + 8.0 * (season * 0.5 + 0.5) + t * 1.4));
        rows.push((t, "Partner", 10.0 + 4.0 * (t * 0.5).cos().abs() + t * 0.3));
    }
    rows
}

#[test]
fn stacked_golden_lists_the_bottom_layer_first() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::area(months())
        .x(|r| r.0)
        .y(|r| r.2)
        .stack(|r| r.1)
        .legend_title("Channel")
        .title("Visits by channel")
        .x_title("Month")
        .y_title("Visits (thousands)")
        .grid(Axes::Y);
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    assert_legend_order(&layout, &["Web", "Mobile", "Partner"]);
    let vr = RgbaImage::new(640, 400, image.into_raw()).unwrap();
    harness()
        .run(
            "gup_core/builder_area_stacked",
            Ok((vr, layout_metadata(&layout))),
        )
        .assert_ok();
}

#[test]
fn overlapping_golden_shows_both_series() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, &str, f64)> = months().into_iter().filter(|r| r.1 != "Partner").collect();
    let mut chart = gup_core::area(rows)
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .outline(1.5)
        .legend_title("Channel")
        .title("Visits by channel, overlapping")
        .x_title("Month")
        .y_title("Visits (thousands)")
        .grid(Axes::Y);
    let (image, layout) = chart.render_resolved(&cx, 640, 400).unwrap();
    assert_legend_order(&layout, &["Web", "Mobile"]);
    let vr = RgbaImage::new(640, 400, image.into_raw()).unwrap();
    harness()
        .run("gup_core/builder_area", Ok((vr, layout_metadata(&layout))))
        .assert_ok();
}

/// The legend entries run top to bottom in `order`.
fn assert_legend_order(layout: &Layout, order: &[&str]) {
    let ys: Vec<f32> = order
        .iter()
        .map(|name| {
            layout
                .texts
                .iter()
                .find(|t| &*t.run.text == *name)
                .unwrap_or_else(|| panic!("no legend entry {name}"))
                .bounds
                .y
        })
        .collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
}
