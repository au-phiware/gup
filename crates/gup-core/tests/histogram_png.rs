// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-548: the histogram builder, headless. Axis edges and ticks, the
//! 1 px gap, stacking by key, the axis title, tooltips and picking, and
//! the golden.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::scene::ItemKind;
use gup_core::transform::bin;
use gup_core::{Input, RowId};
use gup_visual_regression::RgbaImage;

const W: u32 = 640;
const H: u32 = 400;

/// A deterministic stream of uniform numbers in (0, 1): a 64-bit LCG.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// A standard normal number (Box–Muller).
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.next(), self.next());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// `(hump, value)`: two overlapping normal-like humps, 1,200 values near
/// 38 and 800 near 62.
fn humps() -> Vec<(&'static str, f64)> {
    let mut rng = Lcg(548);
    let mut rows = Vec::new();
    for _ in 0..1200 {
        rows.push(("Morning", 38.0 + 9.0 * rng.normal()));
    }
    for _ in 0..800 {
        rows.push(("Evening", 62.0 + 7.0 * rng.normal()));
    }
    rows
}

fn render(cx: &Context, chart: &mut impl Chart) -> (image::RgbaImage, gup_core::Layout) {
    let resolved = chart.resolve(cx, W as f32, H as f32).unwrap();
    let layout = resolved.layout.clone();
    let image = gup_core::ImageTarget::new(cx, W, H)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, layout)
}

/// Pixels of an x value, interpolated between the first and last ticks.
fn tick_px(layout: &gup_core::Layout) -> impl Fn(f64) -> f64 {
    let (a, b) = (layout.x_ticks[0], layout.x_ticks[layout.x_ticks.len() - 1]);
    let (a, b) = ((a.0, f64::from(a.1)), (b.0, f64::from(b.1)));
    move |v| a.1 + (v - a.0) / (b.0 - a.0) * (b.1 - a.1)
}

fn spread(n: usize, lo: f64, hi: f64) -> Vec<f64> {
    (0..n)
        .map(|i| lo + (hi - lo) * i as f64 / (n - 1) as f64)
        .collect()
}

/// The x axis runs exactly from the first edge to the last, and every x
/// tick falls on an edge.
#[test]
fn the_axis_spans_the_edges_and_its_ticks_fall_on_them() {
    let cx = Context::new_blocking().unwrap();
    for (lo, hi) in [(0.3, 9.7), (0.0, 100.0), (12.0, 37.0), (-4.2, 3.3)] {
        let mut chart = gup_core::histogram(spread(1000, lo, hi)).x(|v| *v);
        let (_, layout) = render(&cx, &mut chart);
        let bins = bin(spread(1000, lo, hi), &Bins::Auto);
        let mut edges: Vec<f64> = bins.iter().map(|b| b.lo).collect();
        edges.push(bins[bins.len() - 1].hi);
        let ticks: Vec<f64> = layout.x_ticks.iter().map(|t| t.0).collect();
        assert!(ticks.len() >= 3, "{ticks:?}");
        for t in &ticks {
            assert!(
                edges.iter().any(|e| (e - t).abs() < 1e-9),
                "tick {t} is not an edge of {edges:?} ({lo}..{hi})"
            );
        }
        // The axis runs from the first edge to the last.
        let px = tick_px(&layout);
        let plot = layout.plot;
        assert!((px(edges[0]) - f64::from(plot.left())).abs() < 1.0);
        let end = px(edges[edges.len() - 1]);
        assert!(end <= f64::from(plot.right()) + 0.5, "{end}");
    }
}

/// Ink (darkness, 0..1) summed over `x0..x1` on `row`.
fn ink(image: &image::RgbaImage, row: u32, x0: u32, x1: u32) -> f64 {
    (x0..x1)
        .map(|x| (255.0 - f64::from(image.get_pixel(x, row).0[0])) / 255.0)
        .sum()
}

/// Bars are 1 px apart when a bin is 4 px or wider, and touch when narrower.
#[test]
fn bars_are_separated_by_a_pixel_when_wide_enough_and_touch_otherwise() {
    let cx = Context::new_blocking().unwrap();
    for (bins, wide) in [(10usize, true), (250, false)] {
        let step = 10.0 / bins as f64;
        let edges: Vec<f64> = (0..=bins).map(|i| i as f64 * step).collect();
        let values: Vec<f64> = (0..bins)
            .flat_map(|i| [(i as f64 + 0.5) * step; 3])
            .collect();
        let mut chart = gup_core::histogram(values)
            .x(|v| *v)
            .bins(Bins::Edges(edges.clone()))
            .fill(Color::hex(0x000000))
            .legends(false);
        let (image, layout) = render(&cx, &mut chart);
        let plot = layout.plot;
        let row = plot.bottom() as u32 - 3;
        let px = tick_px(&layout);
        let bin_px = px(step) - px(0.0);
        assert_eq!(bin_px >= 4.0, wide, "{bin_px}");
        // Around an edge near the middle: 3 px of window, 2 px either side
        // of the bar ink when a pixel is missing.
        let mid = px(edges[bins / 2]);
        let (x0, x1) = (mid as u32 - 1, mid as u32 + 2);
        let got = ink(&image, row, x0, x1);
        if wide {
            assert!((1.8..=2.2).contains(&got), "gap: ink {got} of 3");
        } else {
            assert!(got >= 2.4, "no gap wanted: ink {got} of 3");
        }
    }
}

/// One layer per colour key, in first-seen order.
#[test]
fn a_colour_key_makes_a_layer_per_key() {
    let mut stacked = gup_core::histogram(humps()).x(|r| r.1).color(|r| r.0);
    assert_eq!(stacked.layers_mut().len(), 2);
    let mut plain = gup_core::histogram(humps()).x(|r| r.1);
    assert_eq!(plain.layers_mut().len(), 1);
}

#[test]
fn the_y_title_is_count_unless_replaced() {
    let cx = Context::new_blocking().unwrap();
    let has =
        |layout: &gup_core::Layout, text: &str| layout.texts.iter().any(|t| &*t.run.text == text);
    let mut chart = gup_core::histogram([1.0, 2.0, 2.5, 4.0]).x(|v| *v);
    let (_, layout) = render(&cx, &mut chart);
    assert!(has(&layout, "Count"));
    let mut chart = gup_core::histogram([1.0, 2.0, 2.5, 4.0])
        .x(|v| *v)
        .y_title("Trips");
    let (_, layout) = render(&cx, &mut chart);
    assert!(has(&layout, "Trips") && !has(&layout, "Count"));
}

/// A tooltip speaks in bins, and a pick returns the bin under the pointer.
#[test]
fn tooltips_and_picks_speak_in_bins() {
    let cx = Context::new_blocking().unwrap();
    let values = [0.5, 0.6, 0.7, 1.5, 2.5, 2.6];
    let mut chart = gup_core::histogram(values)
        .x(|v| *v)
        .bins(Bins::Width(1.0))
        .tooltip(|b| format!("{}..{}: {}", b.lo, b.hi, b.count));
    let (_, layout) = render(&cx, &mut chart);
    let plot = layout.plot;
    // The first bin is 0..1 of an axis 0..3, 3 values tall.
    let at = Point::new(plot.left() + plot.width / 6.0, plot.bottom() - 4.0);
    let bar = chart.pick(at).expect("the first bin").clone();
    assert_eq!((bar.lo, bar.hi, bar.count), (0.0, 1.0, 3));
    assert!(
        chart
            .pick(Point::new(tick_px(&layout)(2.5) as f32, plot.y + 1.0))
            .is_none()
    );

    let anchor = chart
        .plot_mut()
        .pick_anchor(RowId { layer: 0, row: 2 })
        .expect("an anchor");
    assert!(chart.handle(Input::PointerMoved { at: anchor }).redraw);
    let resolved = chart.resolve(&cx, W as f32, H as f32).unwrap();
    let shown = resolved.scene.items.iter().any(
        |i| matches!(&i.kind, ItemKind::Text(runs) if runs.iter().any(|r| &*r.text == "2..3: 2")),
    );
    assert!(shown, "tooltip of the third bin");
}

/// AC3: two overlapping humps, stacked by hump.
#[test]
fn histogram_golden() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::histogram(humps())
        .x(|r| r.1)
        .color(|r| r.0)
        .title("Time to finish")
        .x_title("Minutes")
        .legend_title("Session");
    let resolved = chart.resolve(&cx, W as f32, H as f32).unwrap().clone();
    let image = gup_core::ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    let meta = layout_metadata(&resolved.layout)
        .with_expected_color("Morning fill", rgba8(Categorical::okabe_ito().eval(0.0)))
        .with_expected_color("Evening fill", rgba8(Categorical::okabe_ito().eval(1.0)));
    harness()
        .run("gup_core/histogram", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    svg_marks::assert_matches("gup_core/histogram", &resolved.scene, &vr, &meta);
}
