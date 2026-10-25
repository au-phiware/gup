// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-545: the `Area` mark rendered headlessly, and written as SVG.
//! Fills are measured on the pixels: a red fill on white has a coverage
//! of `(255 - green) / 255`.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::{ImageTarget, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;

const RED: Color = Color::hex(0xff0000);
const W: u32 = 500;
const H: u32 = 400;

/// A row: x, top, baseline.
type Row = (f64, f64, f64);

/// A rendered area, and each row's top and baseline in pixels.
struct Drawn {
    image: image::RgbaImage,
    /// `(x, top, base)` in pixels; `None` for a row with a null.
    points: Vec<Option<(f64, f64, f64)>>,
}

/// Draw `rows` as a red area of `opacity` over x domain 0..500 and y
/// domain 0..400, in chunks of `chunk` rows if given.
fn draw(rows: &[Row], opacity: f32, chunk: Option<u32>) -> Drawn {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    p.legends(false);
    let x = p.x(Linear::new().domain(0.0, 500.0));
    let y = p.y(Linear::new().domain(0.0, 400.0));
    let area = p
        .area(rows.to_vec())
        .x(|r: &Row| r.0)
        .y(|r: &Row| r.1)
        .y_end(|r: &Row| r.2)
        .fill(RED)
        .fill_opacity(Unit(opacity));
    if let Some(n) = chunk {
        area.max_chunk_rows(n);
    }
    let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(resolved.scene.min_samples(), 4);
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let points = rows
        .iter()
        .map(|&(a, b, c)| {
            [a, b, c]
                .iter()
                .all(|v| v.is_finite())
                .then(|| (x.read().eval(a), y.read().eval(b), y.read().eval(c)))
        })
        .collect();
    Drawn { image, points }
}

/// The fill over white at `opacity`, as the green channel of red on white.
fn green_over_white(opacity: f32) -> f64 {
    255.0 * (1.0 - f64::from(opacity))
}

fn green(d: &Drawn, x: f64, y: f64) -> f64 {
    f64::from(d.image.get_pixel(x as u32, y as u32).0[1])
}

const ROWS: [Row; 5] = [
    (50.0, 100.0, 0.0),
    (150.0, 300.0, 0.0),
    (250.0, 200.0, 0.0),
    (350.0, 350.0, 0.0),
    (450.0, 150.0, 0.0),
];

/// AC2: the centre of each trapezoid is the fill over white.
#[test]
fn trapezoid_centres_are_the_fill() {
    for opacity in [1.0, 0.5] {
        let d = draw(&ROWS, opacity, None);
        let want = green_over_white(opacity);
        for w in d.points.windows(2) {
            let (a, b) = (w[0].unwrap(), w[1].unwrap());
            let centre = ((a.0 + b.0) / 2.0, (a.1 + b.1 + a.2 + b.2) / 4.0);
            let got = green(&d, centre.0, centre.1);
            assert!(
                (got - want).abs() <= 2.0,
                "opacity {opacity}: green {got}, want {want} at {centre:?}"
            );
            // The red channel stays full: the fill is red.
            assert_eq!(
                d.image.get_pixel(centre.0 as u32, centre.1 as u32).0[0],
                255
            );
        }
    }
}

/// AC2: no seams. A translucent area, scanned just above its baseline across
/// five trapezoids, varies by at most 1/255.
#[test]
fn a_translucent_area_has_no_seams() {
    let rows: Vec<Row> = (0..6)
        .map(|i| (40.0 + 80.0 * i as f64, 150.0 + 40.0 * (i % 3) as f64, 0.0))
        .collect();
    let d = draw(&rows, 0.5, None);
    let (first, last) = (d.points[0].unwrap(), d.points[5].unwrap());
    let y = first.2 - 10.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut x = first.0 + 2.0;
    while x < last.0 - 2.0 {
        let g = green(&d, x, y);
        lo = lo.min(g);
        hi = hi.max(g);
        x += 1.0;
    }
    assert!(hi - lo <= 1.0, "the scan varies from {lo} to {hi}");
    assert!((lo - green_over_white(0.5)).abs() <= 2.0, "scan {lo}");
}

/// AC2: a segment whose top crosses its baseline draws the two triangles
/// either side of the crossing, and nothing else.
#[test]
fn a_crossing_draws_two_triangles() {
    // Top 300 -> 100 over a baseline of 200: the crossing is at x = 200.
    let rows = [(100.0, 300.0, 200.0), (300.0, 100.0, 200.0)];
    let d = draw(&rows, 1.0, None);
    let (a, b) = (d.points[0].unwrap(), d.points[1].unwrap());
    let (cx, base) = ((a.0 + b.0) / 2.0, a.2);
    let background = 255.0;
    // Just above the baseline, left of the crossing: filled.
    let left = green(&d, cx - 20.0, base - 3.0);
    assert!(left < 5.0, "left lobe is {left}");
    // Just below it, right of the crossing: filled.
    let right = green(&d, cx + 20.0, base + 3.0);
    assert!(right < 5.0, "right lobe is {right}");
    // The wrong sides: background.
    for (x, y) in [(cx + 20.0, base - 3.0), (cx - 20.0, base + 3.0)] {
        let got = green(&d, x, y);
        assert_eq!(got, background, "({x}, {y}) is {got}");
    }
    // At the crossing, 3 px off the baseline either way.
    for y in [base - 3.0, base + 3.0] {
        let got = green(&d, cx, y);
        assert_eq!(got, background, "the crossing column at {y} is {got}");
    }
}

/// AC2: a null `y` breaks the area: the two trapezoids touching it are not
/// drawn, and the others are.
#[test]
fn a_null_row_breaks_the_area() {
    let mut rows = ROWS;
    rows[2].1 = f64::NAN;
    let d = draw(&rows, 1.0, None);
    // The row's pixels: the null's position is unknown, take the others.
    let p = |i: usize| {
        let (x, _, base) = d.points[i].unwrap_or_else(|| {
            let (x, _, base) = d_points_fallback(&rows, &d, i);
            (x, 0.0, base)
        });
        (x, base)
    };
    let (x1, base) = p(1);
    let (x2, _) = p(2);
    let (x3, _) = p(3);
    // Just above the baseline, mid-way along each of the four segments.
    let at = |x: f64| green(&d, x, base - 4.0);
    let (x0, _) = p(0);
    let (x4, _) = p(4);
    assert!(at((x0 + x1) / 2.0) < 5.0, "segment 0-1 is drawn");
    assert_eq!(at((x1 + x2) / 2.0), 255.0, "segment 1-2 is hidden");
    assert_eq!(at((x2 + x3) / 2.0), 255.0, "segment 2-3 is hidden");
    assert!(at((x3 + x4) / 2.0) < 5.0, "segment 3-4 is drawn");
}

/// The pixel `(x, top, base)` of a row with a null top, from its neighbours'
/// spacing (x and baseline are not null).
fn d_points_fallback(rows: &[Row], d: &Drawn, i: usize) -> (f64, f64, f64) {
    let (a, b) = (d.points[i - 1].unwrap(), d.points[i + 1].unwrap());
    let t = (rows[i].0 - rows[i - 1].0) / (rows[i + 1].0 - rows[i - 1].0);
    (a.0 + t * (b.0 - a.0), 0.0, a.2)
}

/// AC2: ten rows in chunks of four match the one-chunk render.
#[test]
fn a_chunked_area_matches_the_whole() {
    let rows: Vec<Row> = (0..10)
        .map(|i| {
            (
                30.0 + 45.0 * i as f64,
                if i % 2 == 0 { 120.0 } else { 300.0 },
                60.0,
            )
        })
        .collect();
    let chunked = draw(&rows, 1.0, Some(4));
    let whole = draw(&rows, 1.0, None);
    let differ = chunked
        .image
        .pixels()
        .zip(whole.image.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(x, y)| x.abs_diff(y) > 2))
        .count();
    assert!(
        differ <= 8,
        "{differ} pixels differ from the one-chunk area"
    );
}

/// AC3: an area of five rows with a null in the middle writes two paths of
/// four corners each.
#[test]
fn a_null_splits_the_svg_path() {
    let mut rows = ROWS;
    rows[2].1 = f64::NAN;
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    p.area(rows.to_vec())
        .x(|r: &Row| r.0)
        .y(|r: &Row| r.1)
        .y_end(|r: &Row| r.2)
        .fill(RED)
        .fill_opacity(Unit(0.5));
    let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
    let mut target = SvgTarget::new();
    target.render(&resolved.scene).unwrap();
    let svg = target.svg();
    let paths: Vec<_> = svg
        .lines()
        .filter(|l| l.contains(r##"fill="#ff0000""##) && l.starts_with("<path "))
        .collect();
    assert_eq!(paths.len(), 2, "{svg}");
    for p in &paths {
        assert_eq!(p.matches('M').count(), 1, "{p}");
        assert_eq!(p.matches('L').count(), 3, "{p}");
        assert!(
            p.contains(r#"fill-opacity="0.5""#) && p.contains(" Z\""),
            "{p}"
        );
    }
}

/// AC4: a sine wave filled to a zero baseline, crossing it four times, with
/// one null gap; and its SVG matches.
#[test]
fn area_mark_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (480u32, 320u32);
    let rows: Vec<(f64, f64)> = (0..64)
        .map(|i| {
            let t = 0.5 + 0.2 * f64::from(i);
            (
                t,
                if (30..33).contains(&i) {
                    f64::NAN
                } else {
                    t.sin()
                },
            )
        })
        .collect();
    let mut p = Plot::new();
    p.legends(false);
    p.grid(Axes::Y);
    p.x(Linear::new());
    p.y(Linear::new().domain(-1.2, 1.2));
    p.area(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .y_end(|_| 0.0)
        .fill_opacity(Unit(0.6));
    let resolved = p.resolve(&cx, w as f32, h as f32).unwrap();
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    // The fill is 60% opaque, so its colour over white is not the configured
    // one: no expected colour (the pixel tests above check the blend).
    let meta = layout_metadata(&resolved.layout);
    harness()
        .run("gup_core/area_mark", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    let svg = svg_marks::assert_matches("gup_core/area_mark", &resolved.scene, &vr, &meta);
    assert_eq!(
        svg.matches("<path d=\"M").count(),
        2,
        "one run each side of the gap"
    );
}
