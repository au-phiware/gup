// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-554: the box plot builder, headless. Mark positions against known
//! statistics, colours, tooltips, the value axis domain and the golden.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::prelude::*;
use gup_core::scene::ItemKind;
use gup_core::transform::box_stats;
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

fn render(cx: &Context, chart: &mut impl Chart) -> (image::RgbaImage, gup_core::Layout) {
    let resolved = chart.resolve(cx, W as f32, H as f32).unwrap();
    let layout = resolved.layout.clone();
    let image = gup_core::ImageTarget::new(cx, W, H)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, layout)
}

/// Pixel y of a value, interpolated between the first and last y ticks.
fn tick_py(layout: &gup_core::Layout) -> impl Fn(f64) -> f64 {
    let (a, b) = (layout.y_ticks[0], layout.y_ticks[layout.y_ticks.len() - 1]);
    let (a, b) = ((a.0, f64::from(a.1)), (b.0, f64::from(b.1)));
    move |v| a.1 + (v - a.0) / (b.0 - a.0) * (b.1 - a.1)
}

/// How dark pixel `(x, y)` is, 0..1 (the red channel of a black mark).
fn dark(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    (255.0 - f64::from(image.get_pixel(x, y).0[0])) / 255.0
}

/// One category, ten values 10..=19 and one at 40: Q1 12.5, median 15,
/// Q3 17.5 (NumPy's `percentile` of the eleven values), whiskers 10 and 19,
/// 40 an outlier.
fn one_category() -> Vec<(&'static str, f64)> {
    (10..=19)
        .map(|v| ("A", f64::from(v)))
        .chain([("A", 40.0)])
        .collect()
}

fn black_chart(rows: Vec<(&'static str, f64)>) -> impl Chart {
    gup_core::boxplot(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .fill(Color::hex(0x000000))
        .legends(false)
}

/// Box, median, whiskers, caps and the outlier sit where the statistics put
/// them, within a pixel.
#[test]
fn marks_are_where_the_statistics_put_them() {
    let cx = Context::new_blocking().unwrap();
    let s = box_stats(one_category().iter().map(|r| r.1)).unwrap();
    // Hand-computed type-7 quartiles.
    assert_eq!((s.q1, s.median, s.q3), (12.5, 15.0, 17.5));
    assert_eq!(
        (s.whisker_lo, s.whisker_hi, s.outliers.clone()),
        (10.0, 19.0, vec![40.0])
    );

    let mut chart = black_chart(one_category());
    let (image, layout) = render(&cx, &mut chart);
    let py = tick_py(&layout);
    let plot = layout.plot;
    let cx_px = f64::from(plot.x + plot.width / 2.0);
    let col = cx_px.round() as u32;

    // The box: across the row at the box's middle, from edge to edge.
    let mid = py((s.q1 + s.q3) / 2.0).round() as u32 + 2;
    let ink: Vec<u32> = (0..W).filter(|&x| dark(&image, x, mid) > 0.2).collect();
    let (left, right) = (f64::from(ink[0]), f64::from(ink[ink.len() - 1] + 1));
    // One band is 0.8 of the plot width (padding 0.2 between, 0.1 outside,
    // of one band); the box is 0.6 of it.
    let want = 0.6 * 0.8 * f64::from(plot.width);
    assert!(
        ((right - left) - want).abs() <= 1.5,
        "{} vs {want}",
        right - left
    );
    assert!((((left + right) / 2.0) - cx_px).abs() <= 1.0, "centred");

    // Down a column inside the box but off the whisker: the dark rows are
    // the top edge, the median and the bottom edge.
    let off = col + (want / 4.0) as u32;
    let mut rows: Vec<Vec<u32>> = Vec::new();
    for y in 0..H {
        if dark(&image, off, y) > 0.8 {
            match rows.last_mut() {
                Some(r) if *r.last().unwrap() + 1 == y => r.push(y),
                _ => rows.push(vec![y]),
            }
        }
    }
    let centres: Vec<f64> = rows
        .iter()
        .map(|r| f64::from(r[0] + r[r.len() - 1]) / 2.0 + 0.5)
        .collect();
    // The caps are 0.25 of a box either side of the centre: not at `off`.
    assert_eq!(centres.len(), 3, "{rows:?}");
    for (got, value) in centres.iter().zip([s.q3, s.median, s.q1]) {
        assert!((got - py(value)).abs() <= 1.0, "{got} vs {}", py(value));
    }

    // The upper whisker: dark all the way from the box to the cap, at the
    // centre column, and clear beside it.
    let (top, edge) = (py(s.whisker_hi).round() as u32, py(s.q3).round() as u32);
    for y in top + 1..edge - 1 {
        assert!(dark(&image, col, y) > 0.5, "whisker at row {y}");
        assert!(dark(&image, col + 6, y) < 0.1, "clear beside it at row {y}");
    }
    // The lower whisker.
    let (edge, bottom) = (py(s.q1).round() as u32, py(s.whisker_lo).round() as u32);
    for y in edge + 2..bottom - 1 {
        assert!(dark(&image, col, y) > 0.5, "lower whisker at row {y}");
    }
    // The caps: half a box wide, centred, on the whisker ends.
    for end in [s.whisker_hi, s.whisker_lo] {
        let row = py(end).round() as u32;
        let ink: Vec<u32> = (0..W)
            .filter(|&x| (0..2).any(|d| dark(&image, x, row - d) > 0.5))
            .collect();
        let width = f64::from(ink[ink.len() - 1] + 1 - ink[0]);
        assert!(
            (width - want / 2.0).abs() <= 2.0,
            "cap {width} vs {}",
            want / 2.0
        );
        let centre = f64::from(ink[0] + ink[ink.len() - 1] + 1) / 2.0;
        assert!((centre - cx_px).abs() <= 1.0);
    }

    // The outlier: a disc of radius 2.5 centred on 40.
    let (ox, oy) = (cx_px, py(40.0));
    let (mut sum, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for y in (oy as u32 - 5)..(oy as u32 + 6) {
        for x in (col - 5)..(col + 6) {
            let d = dark(&image, x, y);
            sum += d;
            sx += d * (f64::from(x) + 0.5);
            sy += d * (f64::from(y) + 0.5);
        }
    }
    assert!((sx / sum - ox).abs() <= 1.0 && (sy / sum - oy).abs() <= 1.0);
    let area = std::f64::consts::PI * 2.5 * 2.5;
    assert!((sum - area).abs() < 2.0, "ink {sum} vs disc area {area}");
}

/// The y axis spans every outlier and whisker.
#[test]
fn the_value_axis_includes_every_outlier_and_whisker() {
    let cx = Context::new_blocking().unwrap();
    let mut rows = one_category();
    rows.extend([("B", -60.0), ("B", 0.0), ("B", 1.0), ("B", 2.0), ("B", 3.0)]);
    let mut chart = black_chart(rows);
    let (_, layout) = render(&cx, &mut chart);
    let py = tick_py(&layout);
    let plot = layout.plot;
    for v in [-60.0, 40.0, 10.0, 19.0, 3.0] {
        let y = py(v) as f32;
        assert!(
            y >= plot.top() - 0.5 && y <= plot.bottom() + 0.5,
            "{v} at {y}"
        );
    }
    // And the ticks cover the extremes (the axis is nice, so it can go beyond).
    let (lo, hi) = (
        layout.y_ticks[0].0,
        layout.y_ticks[layout.y_ticks.len() - 1].0,
    );
    assert!(lo <= -60.0 + 20.0 && hi >= 40.0 - 20.0, "{lo}..{hi}");
}

fn two_groups() -> Vec<(&'static str, &'static str, f64)> {
    let mut rng = Lcg(554);
    let mut rows = Vec::new();
    for (cat, group, mean) in [
        ("auth", "fast", 30.0),
        ("search", "slow", 80.0),
        ("cart", "fast", 40.0),
    ] {
        for _ in 0..100 {
            rows.push((cat, group, mean + 8.0 * rng.normal()));
        }
    }
    rows
}

/// `color(key)` colours every mark of a category through one scale, with a
/// legend; `fill` sets one colour; the default is `#0072B2`.
#[test]
fn colours_come_from_the_key_the_fill_or_the_default() {
    let cx = Context::new_blocking().unwrap();
    let find = |image: &image::RgbaImage, layout: &gup_core::Layout, value: f64, col: u32| {
        let py = tick_py(layout)(value).round() as i32;
        (-2..=2)
            .map(|d| image.get_pixel(col, (py + d) as u32).0)
            .collect::<Vec<_>>()
    };
    let rgb = |c: Color| {
        let [r, g, b, _] = c.to_rgba8();
        [r, g, b, 255]
    };

    // By key: "fast" is the first key, "slow" the second.
    let mut chart = gup_core::boxplot(two_groups())
        .x(|r| r.0)
        .y(|r| r.2)
        .color(|r| r.1)
        .legend_title("Speed");
    let (image, layout) = render(&cx, &mut chart);
    for label in ["auth", "search", "cart", "fast", "slow", "Speed"] {
        assert!(
            layout.texts.iter().any(|t| &*t.run.text == label),
            "{label}"
        );
    }
    let medians: Vec<f64> = ["auth", "search", "cart"]
        .iter()
        .map(|c| {
            box_stats(two_groups().iter().filter(|r| r.0 == *c).map(|r| r.2))
                .unwrap()
                .median
        })
        .collect();
    let band = f64::from(layout.plot.width) / 3.0;
    let colours = Categorical::okabe_ito();
    for (i, (median, key)) in medians.iter().zip([0.0, 1.0, 0.0]).enumerate() {
        let col = (f64::from(layout.plot.x) + band * (i as f64 + 0.5) - 8.0) as u32;
        let want = rgb(colours.eval(key));
        assert!(
            find(&image, &layout, *median, col).contains(&want),
            "median of category {i}: {:?} vs {want:?}",
            find(&image, &layout, *median, col)
        );
    }

    // One fill.
    let orange = Color::hex(0xD5_5E_00);
    let mut chart = gup_core::boxplot(two_groups())
        .x(|r| r.0)
        .y(|r| r.2)
        .fill(orange);
    let (image, layout) = render(&cx, &mut chart);
    let col = (f64::from(layout.plot.x) + band * 0.5 - 8.0) as u32;
    assert!(find(&image, &layout, medians[0], col).contains(&rgb(orange)));

    // Default.
    let mut chart = gup_core::boxplot(two_groups()).x(|r| r.0).y(|r| r.2);
    let (image, layout) = render(&cx, &mut chart);
    let want = rgb(Color::hex(0x00_72_B2));
    assert!(find(&image, &layout, medians[0], col).contains(&want));
    // No colour key: no legend entries.
    assert!(!layout.texts.iter().any(|t| &*t.run.text == "fast"));
}

/// A tooltip describes a box, and a pick returns its summary.
#[test]
fn a_tooltip_describes_the_box_under_the_pointer() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::boxplot(one_category())
        .x(|r| r.0)
        .y(|r| r.1)
        .tooltip(|s| format!("{}: median {}, n={}", s.key, s.stats.median, s.stats.n));
    let (_, layout) = render(&cx, &mut chart);
    let py = tick_py(&layout);
    let plot = layout.plot;
    let at = gup_core::geom::Point::new(plot.x + plot.width / 2.0 + 20.0, py(16.0) as f32);
    let hit = chart.pick(at).expect("the box").clone();
    assert_eq!((&*hit.key, hit.stats.median, hit.stats.n), ("A", 15.0, 11));

    let anchor = chart
        .plot_mut()
        .pick_anchor(RowId { layer: 0, row: 0 })
        .expect("an anchor");
    assert!(chart.handle(Input::PointerMoved { at: anchor }).redraw);
    let resolved = chart.resolve(&cx, W as f32, H as f32).unwrap();
    let shown = resolved.scene.items.iter().any(|i| {
        matches!(&i.kind, ItemKind::Text(runs) if runs.iter().any(|r| &*r.text == "A: median 15, n=11"))
    });
    assert!(shown, "tooltip text");
}

/// Categories are in first-seen order, and a category with no finite value
/// has no box.
#[test]
fn categories_keep_their_first_seen_order() {
    let cx = Context::new_blocking().unwrap();
    let rows = vec![
        ("b", 1.0),
        ("a", 2.0),
        ("b", 3.0),
        ("c", f64::NAN),
        ("a", 4.0),
    ];
    let mut chart = gup_core::boxplot(rows).x(|r| r.0).y(|r| r.1);
    let (_, layout) = render(&cx, &mut chart);
    let mut cats: Vec<(&str, f32)> = layout
        .texts
        .iter()
        .filter(|t| matches!(&*t.run.text, "a" | "b" | "c"))
        .map(|t| (&*t.run.text, t.bounds.x))
        .collect();
    cats.sort_by(|p, q| p.1.total_cmp(&q.1));
    assert_eq!(cats.iter().map(|c| c.0).collect::<Vec<_>>(), ["b", "a"]);
}

/// Five services of 200 latencies each, two with injected outliers.
fn latencies() -> Vec<(&'static str, f64)> {
    let mut rng = Lcg(5540);
    let mut rows = Vec::new();
    for (i, (service, median, spread)) in [
        ("auth", 42.0, 6.0),
        ("search", 95.0, 18.0),
        ("cart", 60.0, 9.0),
        ("checkout", 130.0, 22.0),
        ("images", 70.0, 12.0),
    ]
    .into_iter()
    .enumerate()
    {
        for _ in 0..200 {
            rows.push((service, median + spread * rng.normal()));
        }
        if i == 1 {
            rows.extend([(service, 210.0), (service, 232.0)]);
        }
        if i == 3 {
            rows.extend([(service, 12.0), (service, 262.0)]);
        }
    }
    rows
}

/// AC3: five services, boxes with whiskers and outliers.
#[test]
fn boxplot_golden() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::boxplot(latencies())
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Request latency")
        .y_title("Milliseconds")
        .x_title("Service");
    let resolved = chart.resolve(&cx, W as f32, H as f32).unwrap().clone();
    let image = gup_core::ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    let meta = layout_metadata(&resolved.layout)
        .with_expected_color("box colour", rgba8(Color::hex(0x00_72_B2)));
    harness()
        .run("gup_core/boxplot", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    svg_marks::assert_matches("gup_core/boxplot", &resolved.scene, &vr, &meta);
}
