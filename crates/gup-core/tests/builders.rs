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
        .color(|r| r.segment.clone())
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
        .color(|r| r.segment.clone());
    let after = gup_core::scatter(rows())
        .color(|r| r.segment.clone())
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

// ---- GUP-463: the line builder -------------------------------------------

const START: f64 = 1_546_300_800.0; // 2019-01-01T00:00:00Z
const DAY: f64 = 86_400.0;
const DAYS: usize = 1826;
/// The 10 days of `None` in the middle series.
const GAP: std::ops::Range<usize> = 900..910;

#[derive(Clone, Debug)]
struct Quote {
    time: f64,
    ticker: String,
    price: Option<f64>,
}

/// Three tickers over `days` days, interleaved by date as a CSV would be.
/// `Bravo` has no price on the days in `GAP`.
fn quotes(days: usize) -> Vec<Quote> {
    let mut seed = 0x1234_5678_9abc_def1u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let mut level = [100.0, 140.0, 60.0];
    let mut rows = Vec::new();
    for d in 0..days {
        for (i, name) in ["Alpha", "Bravo", "Charlie"].iter().enumerate() {
            level[i] = (level[i] + next() * 3.0 + 0.02 * (i as f64 + 1.0)).max(5.0);
            rows.push(Quote {
                time: START + d as f64 * DAY,
                ticker: (*name).to_string(),
                price: (!(i == 1 && GAP.contains(&d))).then_some(level[i]),
            });
        }
    }
    rows
}

fn line_builder(rows: Vec<Quote>) -> gup_core::LineChart<Quote> {
    gup_core::line(rows)
        .x_scale(Time::new())
        .x(|q| q.time)
        .y(|q| q.price)
        .color(|q| q.ticker.clone())
        .legend_title("Ticker")
        .grid(Axes::Y)
        .title("Three tickers, five years")
}

/// Does any pixel of column `x` inside the plot rect look like `c`?
fn column_has(image: &image::RgbaImage, layout: &Layout, x: f64, c: Color) -> bool {
    let [r, g, b, _] = c.to_rgba8();
    let (top, bottom) = (layout.plot.top().ceil() as u32, layout.plot.bottom() as u32);
    (top..bottom).any(|y| {
        let p = image.get_pixel(x as u32, y).0;
        [(p[0], r), (p[1], g), (p[2], b)]
            .iter()
            .all(|&(a, b)| a.abs_diff(b) <= 40)
    })
}

#[test]
fn line_is_byte_identical_to_the_reference_plot() {
    let cx = Context::new_blocking().unwrap();
    let rows = quotes(200);
    let mut chart = gup_core::line(rows.clone())
        .x(|q| q.time)
        .y(|q| q.price)
        .color(|q| q.ticker.clone());
    let (built, built_layout) = chart.render_resolved(&cx, 640, 400).unwrap();

    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let cat = ScaleRef::new(Categorical::okabe_ito());
    for name in ["Alpha", "Bravo", "Charlie"] {
        let series: Vec<Quote> = rows.iter().filter(|q| q.ticker == name).cloned().collect();
        plot.line(series)
            .attr(Line::X, x.encode(|q: &Quote| q.time))
            .attr(Line::Y, y.encode(|q: &Quote| q.price))
            .color(cat.encode_key(|q: &Quote| q.ticker.as_str()));
    }
    let (reference, layout) = plot.render_resolved(&cx, 640, 400).unwrap();
    assert!(built.as_raw() == reference.as_raw(), "pixels differ");
    assert_eq!(format!("{built_layout:?}"), format!("{layout:?}"));
}

#[test]
fn series_follow_first_seen_order_and_keep_data_order() {
    // "Zulu" comes first in the data, so it is first: layer 0, palette 0.
    // Its rows are not sorted by x, and are drawn in data order.
    let rows: Vec<(f64, f64, &str)> = vec![
        (0.0, 1.0, "Zulu"),
        (0.0, 5.0, "Alpha"),
        (4.0, 9.0, "Zulu"),
        (4.0, 6.0, "Alpha"),
        (2.0, 2.0, "Zulu"),
    ];
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::line(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .color(|r| r.2)
        .width(4.0);
    assert_eq!(chart.layers_mut().len(), 2);
    let (image, layout) = chart.render_resolved(&cx, 400, 300).unwrap();
    let to = |x: f64, y: f64| (at(&layout.x_ticks, x), at(&layout.y_ticks, y));
    // Zulu: (0,1) -> (4,9) -> (2,2); the first segment passes (1, 3).
    let (x, y) = to(1.0, 3.0);
    assert_near(px(&image, x, y), over_white(OKABE_ITO[0], 1.0));
    // Alpha: (0,5) -> (4,6); the second layer takes the second colour.
    let (x, y) = to(2.0, 5.5);
    assert_near(px(&image, x, y), over_white(OKABE_ITO[1], 1.0));
    // The line goes back from (4,9) to (2,2): (3, 5.5) is on it.
    let (x, y) = to(3.0, 5.5);
    assert_near(px(&image, x, y), over_white(OKABE_ITO[0], 1.0));
    assert_eq!(
        chart
            .pick(Point::new(to(3.9, 8.7).0 as f32, to(3.9, 8.7).1 as f32))
            .map(|r| r.2),
        Some("Zulu")
    );
}

#[test]
fn line_golden_has_clean_breaks_and_a_legend() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = line_builder(quotes(DAYS));
    let (image, layout) = chart.render_resolved(&cx, 800, 450).unwrap();

    let gap_colour = OKABE_ITO[1];
    let x_of = |d: usize| at(&layout.x_ticks, START + d as f64 * DAY);
    let middle = x_of((GAP.start + GAP.end) / 2 - 1) + 0.5 * (x_of(1) - x_of(0));
    assert!(
        !column_has(&image, &layout, middle, gap_colour),
        "the gap column has the series colour"
    );
    for d in [GAP.start - 3, GAP.end + 2] {
        assert!(
            column_has(&image, &layout, x_of(d), gap_colour),
            "day {d} lacks the series colour"
        );
    }
    assert!(layout.texts.iter().any(|t| &*t.run.text == "Ticker"));

    let vr = RgbaImage::new(800, 450, image.into_raw()).unwrap();
    harness()
        .run("gup_core/builder_line", Ok((vr, layout_metadata(&layout))))
        .assert_ok();
}

#[test]
fn layers_mut_reaches_each_series() {
    let cx = Context::new_blocking().unwrap();
    let rows = quotes(200);
    let thickness = |chart: &mut gup_core::LineChart<Quote>, x: f64| -> u32 {
        let image = chart.render_resolved(&cx, 640, 400).unwrap().0;
        let col = x as u32;
        let ink = |c: Color| {
            (0..400u32)
                .filter(|&y| {
                    let p = image.get_pixel(col, y).0;
                    let [r, g, b, _] = c.to_rgba8();
                    p[0].abs_diff(r) < 60 && p[1].abs_diff(g) < 60 && p[2].abs_diff(b) < 60
                })
                .count() as u32
        };
        ink(OKABE_ITO[2]) * 1000 + ink(OKABE_ITO[0])
    };
    let mut thin = gup_core::line(rows.clone())
        .x(|q| q.time)
        .y(|q| q.price)
        .color(|q| q.ticker.clone());
    let mut wide = gup_core::line(rows)
        .x(|q| q.time)
        .y(|q| q.price)
        .color(|q| q.ticker.clone());
    let layers = wide.layers_mut();
    assert_eq!(layers.len(), 3);
    layers.into_iter().nth(2).unwrap().width(Px(8.0));
    let (a, b) = (thickness(&mut thin, 300.0), thickness(&mut wide, 300.0));
    // Only the third series (OKABE_ITO[2]) changed; the first did not.
    assert!(b / 1000 > a / 1000 + 3, "third series: {a} -> {b}");
    assert_eq!(a % 1000, b % 1000, "first series changed");
}

#[test]
fn pick_returns_the_nearer_endpoint_of_the_segment() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = vec![(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)];
    let mut chart = gup_core::line(rows).x(|r| r.0).y(|r| r.1).width(4.0);
    let layout = chart.render_resolved(&cx, 400, 300).unwrap().1;
    let p =
        |x: f64, y: f64| Point::new(at(&layout.x_ticks, x) as f32, at(&layout.y_ticks, y) as f32);
    // Just before the middle of the first segment: the start row.
    assert_eq!(chart.pick(p(2.0, 2.0)).copied(), Some((0.0, 0.0)));
    // Just past the middle of the first segment: the apex.
    assert_eq!(chart.pick(p(8.0, 8.0)).copied(), Some((10.0, 10.0)));
}

#[test]
#[should_panic(
    expected = "set layer encodings before the chart is first drawn; change a drawn chart through layer_mut()"
)]
fn line_setters_panic_after_the_first_draw() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::line(quotes(5)).x(|q| q.time).y(|q| q.price);
    chart.render(&cx, 200, 150).unwrap();
    let _ = chart.width(3.0);
}

#[test]
fn stroke_gives_one_layer_and_the_last_colour_call_wins() {
    let rows = quotes(5);
    let mut chart = gup_core::line(rows.clone())
        .color(|q| q.ticker.clone())
        .stroke(Color::BLACK);
    assert_eq!(chart.layers_mut().len(), 1);
    let mut chart = gup_core::line(rows)
        .stroke(Color::BLACK)
        .color(|q| q.ticker.clone());
    assert_eq!(chart.layers_mut().len(), 3);
}

/// Rows with an `Option` position draw only where there is a value.
#[test]
fn option_positions_skip_none_rows() {
    struct R {
        a: f64,
        maybe: Option<f64>,
        small: Option<f32>,
    }
    let cx = Context::new_blocking().unwrap();
    let rows = || {
        vec![
            R {
                a: 0.0,
                maybe: Some(0.0),
                small: Some(0.0),
            },
            R {
                a: 50.0,
                maybe: None,
                small: None,
            },
            R {
                a: 100.0,
                maybe: Some(100.0),
                small: Some(100.0),
            },
        ]
    };
    for f32_path in [false, true] {
        let mut plot = Plot::new();
        plot.legends(false);
        plot.x(Linear::new().domain(0.0, 100.0));
        plot.y(Linear::new().domain(0.0, 100.0));
        let layer = plot.circles(rows()).x(|r| r.a);
        if f32_path {
            layer.y(|r| r.small);
        } else {
            layer.y(|r| r.maybe);
        }
        layer.radius(Px(6.0));
        let (image, layout) = plot.render_resolved(&cx, 300, 300).unwrap();
        let (x, y) = (at(&layout.x_ticks, 50.0), at(&layout.y_ticks, 50.0));
        // The y position of a null would be NaN; nothing is drawn at the
        // middle of the diagonal, and the two real discs are.
        let mid = px(&image, x, y);
        assert!(mid.iter().all(|&v| v > 200), "f32={f32_path}: {mid:?}");
        for v in [0.0, 100.0] {
            let c = px(&image, at(&layout.x_ticks, v), at(&layout.y_ticks, v));
            assert!(c.iter().any(|&v| v < 100), "f32={f32_path}: {c:?}");
        }
    }
}

// ---- GUP-464: the bar builder --------------------------------------------

mod bars {
    use super::{at, harness};
    use gup_core::geom::Point;
    use gup_core::prelude::*;
    use gup_core::{ChartExt, Context, Layout, OKABE_ITO, ScaleRef};
    use gup_visual_regression::RgbaImage;

    #[derive(Clone, Debug)]
    pub struct Sale {
        pub quarter: &'static str,
        pub region: &'static str,
        pub total: f64,
    }

    pub const QUARTERS: [&str; 4] = ["Q1", "Q2", "Q3", "Q4"];
    pub const REGIONS: [&str; 4] = ["North", "South", "East", "West"];
    const TOTALS: [[f64; 4]; 4] = [
        [32.0, 24.0, 18.0, 40.0],
        [45.0, 51.0, 22.0, 35.0],
        [28.0, 39.0, 30.0, 26.0],
        [50.0, 41.0, 36.0, 44.0],
    ];

    /// Sales by quarter, then region.
    pub fn sales() -> Vec<Sale> {
        let mut rows = Vec::new();
        for (q, quarter) in QUARTERS.iter().enumerate() {
            for (r, region) in REGIONS.iter().enumerate() {
                rows.push(Sale {
                    quarter,
                    region,
                    total: TOTALS[q][r],
                });
            }
        }
        rows
    }

    fn sale(quarter: &'static str, region: &'static str, total: f64) -> Sale {
        Sale {
            quarter,
            region,
            total,
        }
    }

    pub fn grouped(rows: Vec<Sale>) -> gup_core::BarChart<Sale> {
        gup_core::bar(rows)
            .x(|r| r.quarter)
            .y(|r| r.total)
            .group(|r| r.region)
    }

    pub fn stacked(rows: Vec<Sale>) -> gup_core::BarChart<Sale> {
        gup_core::bar(rows)
            .x(|r| r.quarter)
            .y(|r| r.total)
            .stack(|r| r.region)
    }

    pub fn near(p: [u8; 4], c: Color, tolerance: u8) -> bool {
        let [r, g, b, _] = c.to_rgba8();
        p[0].abs_diff(r) <= tolerance
            && p[1].abs_diff(g) <= tolerance
            && p[2].abs_diff(b) <= tolerance
    }

    /// Non-background runs of at least 3 px along row `y` (the dogfood rule).
    fn count_runs(image: &image::RgbaImage, y: u32, x0: u32, x1: u32) -> usize {
        let bg = image.get_pixel(0, 0).0;
        let (mut runs, mut len) = (0, 0);
        for x in x0..x1 {
            let p = image.get_pixel(x, y).0;
            if (0..3).any(|c| p[c].abs_diff(bg[c]) > 8) {
                len += 1;
            } else {
                if len >= 3 {
                    runs += 1;
                }
                len = 0;
            }
        }
        if len >= 3 {
            runs += 1;
        }
        runs
    }

    fn runs_near_baseline(image: &image::RgbaImage, layout: &Layout) -> usize {
        let floor = at(&layout.y_ticks, 0.0) as u32;
        let plot = layout.plot;
        count_runs(
            image,
            floor - 3,
            plot.left() as u32 + 2,
            plot.right() as u32 - 2,
        )
    }

    fn pixel(image: &image::RgbaImage, x: f64, y: f64) -> [u8; 4] {
        image.get_pixel(x as u32, y as u32).0
    }

    /// The reference for simple bars: a `Plot` built by hand.
    fn reference_simple(rows: Vec<Sale>) -> Plot {
        let mut plot = Plot::new();
        let x = plot.x(Band::new());
        plot.y(Linear::new());
        plot.rects(rows)
            .x_with(x.encode_key(|r: &Sale| r.quarter).domain(QUARTERS))
            .width(x.band_width_px())
            .y(|r: &Sale| r.total)
            .y_end(|_: &Sale| 0.0f64);
        plot
    }

    #[test]
    fn simple_bars_are_byte_identical_to_the_hand_built_plot() {
        let cx = Context::new_blocking().unwrap();
        let rows: Vec<Sale> = sales()
            .into_iter()
            .filter(|r| r.region == "North")
            .collect();
        let mut chart = gup_core::bar(rows.clone()).x(|r| r.quarter).y(|r| r.total);
        let (built, layout) = chart.render_resolved(&cx, 400, 300).unwrap();
        let (reference, ref_layout) = reference_simple(rows)
            .render_resolved(&cx, 400, 300)
            .unwrap();
        assert!(built.as_raw() == reference.as_raw(), "pixels differ");
        assert_eq!(format!("{layout:?}"), format!("{ref_layout:?}"));
        assert_eq!(chart.layers_mut().len(), 1);
    }

    #[test]
    fn the_value_axis_includes_zero_for_all_positive_and_all_negative_bars() {
        let cx = Context::new_blocking().unwrap();
        for sign in [1.0, -1.0] {
            let rows: Vec<Sale> = sales()
                .into_iter()
                .filter(|r| r.region == "East")
                .map(|r| Sale {
                    total: sign * (r.total + 100.0),
                    ..r
                })
                .collect();
            let mut chart = gup_core::bar(rows).x(|r| r.quarter).y(|r| r.total);
            let (image, layout) = chart.render_resolved(&cx, 400, 300).unwrap();
            assert!(
                layout.y_ticks.iter().any(|t| t.0 == 0.0),
                "sign {sign}: no tick at 0 in {:?}",
                layout.y_ticks
            );
            // The bars sit on the 0 line: flush with it, one side only.
            let zero = at(&layout.y_ticks, 0.0);
            let x = f64::from(layout.x_ticks[1].1) + 10.0; // off the tick mark
            let (inside, outside) = if sign > 0.0 {
                (zero - 3.0, zero + 3.0)
            } else {
                (zero + 3.0, zero - 3.0)
            };
            assert!(
                near(pixel(&image, x, inside), Color::hex(0x0072b2), 3),
                "sign {sign}"
            );
            assert!(
                pixel(&image, x, outside)[0] > 240,
                "sign {sign}: bar crosses 0"
            );
        }
    }

    #[test]
    fn grouped_bars_are_sixteen_runs_in_okabe_ito_order_with_a_legend() {
        let cx = Context::new_blocking().unwrap();
        let mut chart = grouped(sales());
        let (image, layout) = chart.render_resolved(&cx, 800, 500).unwrap();
        assert_eq!(runs_near_baseline(&image, &layout), 16);
        for name in REGIONS {
            assert_eq!(
                layout.texts.iter().filter(|t| &*t.run.text == name).count(),
                1,
                "legend entry {name}"
            );
        }
        // Slot r of Q2 is region r's bar, in its own colour.
        let band = f64::from(layout.x_ticks[1].1 - layout.x_ticks[0].1);
        let centre = f64::from(layout.x_ticks[1].1);
        for (r, region_colour) in OKABE_ITO.iter().take(4).enumerate() {
            let x = centre + ((r as f64 + 0.5) / 4.0 - 0.5) * band * 0.8;
            let y = at(&layout.y_ticks, 0.0) - 6.0;
            // Allow for the padding in the band width: look for the colour
            // within the slot rather than at its exact centre.
            let found =
                (-6..=6).any(|d| near(pixel(&image, x + f64::from(d), y), *region_colour, 3));
            assert!(found, "region {r}: no {region_colour:?} near x = {x}");
        }
    }

    #[test]
    fn grouped_bars_are_byte_identical_to_four_hand_built_layers() {
        let cx = Context::new_blocking().unwrap();
        let mut chart = grouped(sales());
        let (built, layout) = chart.render_resolved(&cx, 800, 500).unwrap();

        let mut plot = Plot::new();
        let x = plot.x(Band::new());
        plot.y(Linear::new());
        let colours = ScaleRef::new(Categorical::okabe_ito());
        for (i, region) in REGIONS.iter().enumerate() {
            let part: Vec<Sale> = sales()
                .into_iter()
                .filter(|r| r.region == *region)
                .collect();
            plot.rects(part)
                .x_with(
                    x.sub_band(i as u32, 4)
                        .encode_key(|r: &Sale| r.quarter)
                        .domain(QUARTERS),
                )
                .width(x.sub_band_width_px(4, 0.1))
                .y(|r: &Sale| r.total)
                .y_end(|_: &Sale| 0.0f64)
                .fill(colours.encode_key(|r: &Sale| r.region));
        }
        let (reference, ref_layout) = plot.render_resolved(&cx, 800, 500).unwrap();
        assert!(built.as_raw() == reference.as_raw(), "pixels differ");
        assert_eq!(format!("{layout:?}"), format!("{ref_layout:?}"));
    }

    #[test]
    fn stacked_bars_start_each_segment_where_the_previous_ended() {
        let cx = Context::new_blocking().unwrap();
        let mut chart = stacked(sales());
        let (image, layout) = chart.render_resolved(&cx, 800, 500).unwrap();
        assert_eq!(runs_near_baseline(&image, &layout), 4);
        let y = |v: f64| at(&layout.y_ticks, v);
        for (q, totals) in TOTALS.iter().enumerate() {
            let x = f64::from(layout.x_ticks[q].1);
            let mut base = 0.0;
            for (r, total) in totals.iter().enumerate() {
                let top = base + total;
                let mid = y((base + top) / 2.0);
                assert!(
                    near(pixel(&image, x, mid), OKABE_ITO[r], 3),
                    "{} {}: middle is {:?}, wanted {:?}",
                    QUARTERS[q],
                    REGIONS[r],
                    pixel(&image, x, mid),
                    OKABE_ITO[r]
                );
                if r > 0 {
                    // The join: the segment below ends exactly where this
                    // one starts, with no gap showing the background.
                    let join = y(base);
                    for dy in [-0.5, 0.0, 0.5] {
                        let p = pixel(&image, x, join + dy);
                        assert!(
                            p[..3].iter().any(|&c| c < 200),
                            "{} {}: a light gap at the join: {p:?}",
                            QUARTERS[q],
                            REGIONS[r]
                        );
                    }
                    assert!(near(pixel(&image, x, join + 2.0), OKABE_ITO[r - 1], 3));
                    assert!(near(pixel(&image, x, join - 2.0), OKABE_ITO[r], 3));
                }
                base = top;
            }
            // The stack's top edge is at the sum, within a pixel.
            let ink_top = (0..image.height())
                .find(|&py| {
                    let p = image.get_pixel(x as u32, py).0;
                    py as f32 > layout.plot.top() && near(p, OKABE_ITO[3], 3)
                })
                .unwrap();
            assert!(
                (f64::from(ink_top) - y(base)).abs() <= 1.0,
                "{} top edge at {ink_top}, wanted {}",
                QUARTERS[q],
                y(base)
            );
        }
    }

    #[test]
    fn negative_values_stack_downward_from_zero_separately() {
        let cx = Context::new_blocking().unwrap();
        let rows = vec![
            sale("Q1", "North", 30.0),
            sale("Q1", "South", -20.0),
            sale("Q1", "East", 20.0),
            sale("Q1", "West", -10.0),
        ];
        let mut chart = stacked(rows);
        let (image, layout) = chart.render_resolved(&cx, 400, 300).unwrap();
        let x = f64::from(layout.x_ticks[0].1);
        let y = |v: f64| at(&layout.y_ticks, v);
        for (v, colour, what) in [
            (15.0, OKABE_ITO[0], "North above"),
            (40.0, OKABE_ITO[2], "East above North"),
            (-10.0, OKABE_ITO[1], "South below"),
            (-25.0, OKABE_ITO[3], "West below South"),
        ] {
            assert!(
                near(pixel(&image, x, y(v)), colour, 3),
                "{what}: {:?}",
                pixel(&image, x, y(v))
            );
        }
    }

    #[test]
    fn rows_sharing_a_category_and_key_draw_as_one_summed_segment() {
        let cx = Context::new_blocking().unwrap();
        let rows = vec![
            sale("Q1", "North", 20.0),
            sale("Q1", "North", 30.0),
            sale("Q1", "South", 10.0),
        ];
        let mut chart = stacked(rows);
        let (image, layout) = chart.render_resolved(&cx, 400, 300).unwrap();
        let x = f64::from(layout.x_ticks[0].1);
        let y = |v: f64| at(&layout.y_ticks, v);
        assert!(near(pixel(&image, x, y(25.0)), OKABE_ITO[0], 3));
        assert!(near(pixel(&image, x, y(55.0)), OKABE_ITO[1], 3));
        assert!(pixel(&image, x, y(65.0))[0] > 240, "taller than the sum");
    }

    #[test]
    fn layers_follow_first_seen_key_order() {
        let rows = vec![
            sale("Q1", "B", 1.0),
            sale("Q1", "A", 1.0),
            sale("Q2", "A", 2.0),
        ];
        let lens = |mut chart: gup_core::BarChart<Sale>| {
            chart
                .layers_mut()
                .iter()
                .map(|l| l.len())
                .collect::<Vec<_>>()
        };
        assert_eq!(lens(grouped(rows.clone())), [1, 2]);
        assert_eq!(lens(stacked(rows.clone())), [1, 2]);
        assert_eq!(
            lens(gup_core::bar(rows).x(|r| r.quarter).y(|r| r.total)),
            [3]
        );
    }

    #[test]
    fn pick_returns_the_row_under_a_bar() {
        let cx = Context::new_blocking().unwrap();
        let mut chart = grouped(sales());
        let (_, layout) = chart.render_resolved(&cx, 800, 500).unwrap();
        let band = f64::from(layout.x_ticks[1].1 - layout.x_ticks[0].1);
        for (q, r) in [(0usize, 0usize), (2, 3), (3, 1)] {
            let x = f64::from(layout.x_ticks[q].1) + ((r as f64 + 0.5) / 4.0 - 0.5) * band * 0.8;
            let y = at(&layout.y_ticks, TOTALS[q][r] / 2.0);
            let row = chart.pick(Point::new(x as f32, y as f32)).expect("a bar");
            assert_eq!((row.quarter, row.region), (QUARTERS[q], REGIONS[r]));
        }
    }

    #[test]
    #[should_panic(
        expected = "set layer encodings before the chart is first drawn; change a drawn chart through layer_mut()"
    )]
    fn layer_setters_panic_after_the_first_draw() {
        let cx = Context::new_blocking().unwrap();
        let mut chart = grouped(sales());
        chart.render(&cx, 200, 150).unwrap();
        let _ = chart.padding(0.3);
    }

    /// Two renders side by side: grouped on the left, stacked on the right.
    #[test]
    fn bar_golden_has_gaps_a_legend_and_bars_on_the_axis() {
        let cx = Context::new_blocking().unwrap();
        let build = |chart: gup_core::BarChart<Sale>, title: &str| {
            chart
                .title(title)
                .y_title("Sales")
                .legend_title("Region")
                .grid(Axes::Y)
        };
        let (left, ll) = build(grouped(sales()), "Grouped")
            .render_resolved(&cx, 400, 500)
            .unwrap();
        let (right, rl) = build(stacked(sales()), "Stacked")
            .render_resolved(&cx, 400, 500)
            .unwrap();
        let mut image = image::RgbaImage::new(800, 500);
        image::imageops::replace(&mut image, &left, 0, 0);
        image::imageops::replace(&mut image, &right, 400, 0);

        // One metadata for the composite: the plot rect spans both plots
        // (so the confinement check is weaker than for one chart), and the
        // right chart's texts and guides are shifted into place.
        use crate::common::vr::{rect, rgba8, role};
        use gup_core::geom::Rect as GRect;
        let plot = GRect::new(
            ll.plot.x,
            ll.plot.y.min(rl.plot.y),
            rl.plot.right() + 400.0 - ll.plot.x,
            ll.plot.bottom().max(rl.plot.bottom()) - ll.plot.y.min(rl.plot.y),
        );
        let mut meta = gup_visual_regression::LayoutMetadata::new(rect(plot))
            .with_background(gup_visual_regression::Rgba8::WHITE);
        for (layout, dx) in [(&ll, 0.0), (&rl, 400.0)] {
            let shift = |r: GRect| GRect { x: r.x + dx, ..r };
            for t in &layout.texts {
                meta = meta.with_text(
                    role(t.run.role),
                    t.run.text.to_string(),
                    rect(shift(t.bounds)),
                    rgba8(t.run.style.color),
                );
            }
            for g in &layout.guides {
                meta = meta.with_guide(rect(shift(*g)).inflate(0.5));
            }
        }
        for (i, region) in REGIONS.iter().enumerate() {
            meta = meta.with_expected_color(format!("{region} fill"), rgba8(OKABE_ITO[i]));
        }
        let vr = RgbaImage::new(800, 500, image.into_raw()).unwrap();
        harness()
            .run("gup_core/builder_bars", Ok((vr, meta)))
            .assert_ok();
    }
}
