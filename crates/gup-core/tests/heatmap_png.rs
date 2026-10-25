// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-549: the heatmap builder, headless. Category order, the 1 px gap,
//! missing and null cells, the ramp legend, the diverging midpoint and the
//! goldens.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::{Layout, NULL_COLOR};
use gup_visual_regression::RgbaImage;

const W: u32 = 640;
const H: u32 = 400;

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[derive(Clone)]
struct Cell {
    col: String,
    row: &'static str,
    value: Option<f64>,
}

fn cell(col: impl ToString, row: &'static str, value: Option<f64>) -> Cell {
    Cell {
        col: col.to_string(),
        row,
        value,
    }
}

/// A `cols` by `rows` grid, the value rising with the column.
fn grid(cols: usize, rows: &[&'static str]) -> Vec<Cell> {
    rows.iter()
        .flat_map(|&row| (0..cols).map(move |c| cell(format!("{c:02}"), row, Some(c as f64))))
        .collect()
}

fn chart(rows: Vec<Cell>) -> gup_core::HeatmapChart<Cell> {
    gup_core::heatmap(rows)
        .x(|c| c.col.clone())
        .y(|c| c.row)
        .color_value(|c| c.value)
}

fn render(
    cx: &Context,
    chart: &mut impl Chart,
    w: u32,
    h: u32,
) -> (image::RgbaImage, gup_core::Layout) {
    let resolved = chart.resolve(cx, w as f32, h as f32).unwrap();
    let layout = resolved.layout.clone();
    let image = gup_core::ImageTarget::new(cx, w, h)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, layout)
}

/// The pixel of a tick along an axis from its code, extrapolated from the
/// first two ticks (a thinned axis still has two).
fn along(ticks: &[(f64, f32)], code: f64) -> f64 {
    if ticks.len() < 2 {
        return f64::from(ticks[0].1);
    }
    let (a, b) = (ticks[0], ticks[1]);
    f64::from(a.1) + (code - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
}

/// The centre of cell (`col`, `row`) of a grid with `rows` rows, in pixels.
fn centre(layout: &Layout, rows: usize, col: usize, row: usize) -> (u32, u32) {
    let x = along(&layout.x_ticks, col as f64);
    let y = along(&layout.y_ticks, (rows - 1 - row) as f64);
    (x as u32, y as u32)
}

/// The width of a column in pixels.
fn step(layout: &Layout) -> f64 {
    along(&layout.x_ticks, 1.0) - along(&layout.x_ticks, 0.0)
}

fn near(p: [u8; 4], c: Color, tolerance: u8) -> bool {
    let want = c.to_rgba8();
    (0..3).all(|i| p[i].abs_diff(want[i]) <= tolerance)
}

fn text_centre(layout: &Layout, text: &str) -> (f32, f32) {
    let t = layout
        .texts
        .iter()
        .find(|t| &*t.run.text == text)
        .unwrap_or_else(|| panic!("no text {text:?}"));
    (
        t.bounds.x + t.bounds.width / 2.0,
        t.bounds.y + t.bounds.height / 2.0,
    )
}

/// AC1: x runs left to right and y top to bottom, in first-seen order, and
/// the labels sit on their cells.
#[test]
fn categories_run_left_to_right_and_top_to_bottom_in_first_seen_order() {
    let cx = Context::new_blocking().unwrap();
    let rows = ["Wed", "Mon", "Fri"];
    let mut cells = Vec::new();
    for &row in &rows {
        for col in ["b", "a", "c"] {
            cells.push(cell(col, row, Some(1.0)));
        }
    }
    let mut c = chart(cells);
    let (_, layout) = render(&cx, &mut c, 400, 300);

    let xs: Vec<f32> = ["b", "a", "c"]
        .iter()
        .map(|t| text_centre(&layout, t).0)
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    let ys: Vec<f32> = rows.iter().map(|t| text_centre(&layout, t).1).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");

    // The tick pixels agree: three of each, evenly spaced across the plot,
    // and the first-seen y category (the last position of the reversed
    // domain) is the topmost tick.
    assert_eq!(layout.x_ticks.len(), 3);
    assert_eq!(layout.y_ticks.len(), 3);
    let mut by_px = layout.y_ticks.clone();
    by_px.sort_by(|a, b| a.1.total_cmp(&b.1));
    let codes: Vec<f64> = by_px.iter().map(|t| t.0).collect();
    assert!(codes.windows(2).all(|w| w[0] > w[1]), "{by_px:?}");
    let mut xs = layout.x_ticks.clone();
    xs.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert!(xs.windows(2).all(|w| w[0].0 < w[1].0), "{xs:?}");
}

/// Runs of non-`cell` pixels across the plot at row `y`.
fn seams(image: &image::RgbaImage, layout: &Layout, cols: usize, y: u32, cell: Color) -> usize {
    let half = step(layout) / 2.0;
    let first = along(&layout.x_ticks, 0.0) - half;
    let last = along(&layout.x_ticks, (cols - 1) as f64) + half;
    let (x0, x1) = (first.ceil() as u32 + 1, last.floor() as u32 - 1);
    let mut runs = 0;
    let mut inside = false;
    for x in x0..x1 {
        let off = !near(image.get_pixel(x, y).0, cell, 12);
        if off && !inside {
            runs += 1;
        }
        inside = off;
    }
    runs
}

/// AC1: a 1 px gap between cells when the band is at least 8 px, and none
/// below.
#[test]
fn cells_leave_a_pixel_gap_when_wide_enough_and_abut_otherwise() {
    let cx = Context::new_blocking().unwrap();
    let flat = |cols: usize, nrows: usize| {
        // Every cell has the same value, so every cell is one colour.
        let cells: Vec<Cell> = (0..nrows)
            .flat_map(|r| {
                (0..cols).map(move |c| {
                    cell(
                        format!("{c:03}"),
                        Box::leak(format!("r{r:02}").into_boxed_str()),
                        Some(0.5),
                    )
                })
            })
            .collect();
        gup_core::heatmap(cells)
            .x(|c| c.col.clone())
            .y(|c| c.row)
            .color_value(|c| c.value)
            .color_value_scale(Sequential::viridis().domain(0.0, 1.0))
            .legends(false)
    };
    let teal = Sequential::viridis().domain(0.0, 1.0).eval(0.5);

    // 10 columns across ~500 px: 50 px bands, nine gaps.
    let mut wide = flat(10, 2);
    let (image, layout) = render(&cx, &mut wide, 600, 300);
    let y = centre(&layout, 2, 0, 0).1;
    assert_eq!(seams(&image, &layout, 10, y, teal), 9);
    // The gap between the rows too: a column through the cell centres.
    let x = centre(&layout, 2, 3, 0).0;
    let mid = (layout.plot.y + layout.plot.height / 2.0) as u32;
    let col: Vec<bool> = (mid - 3..=mid + 3)
        .map(|y| !near(image.get_pixel(x, y).0, teal, 12))
        .collect();
    assert!(col.iter().any(|&b| b), "no vertical gap {col:?}");
    assert!(col.iter().filter(|&&b| b).count() <= 2, "{col:?}");

    // 100 columns by 40 rows across ~500 by ~250 px: bands under 8 px, so
    // no gap. The antialiased shared edges leave at most a faint seam, much
    // fainter than the gap of a wide band.
    let mut narrow = flat(100, 40);
    let (image, layout) = render(&cx, &mut narrow, 600, 300);
    let (x, y) = centre(&layout, 40, 20, 12);
    let worst = |points: Vec<(u32, u32)>| {
        points
            .into_iter()
            .map(|(x, y)| {
                let p = image.get_pixel(x, y).0;
                (0..3)
                    .map(|i| p[i].abs_diff(teal.to_rgba8()[i]))
                    .max()
                    .unwrap()
            })
            .max()
            .unwrap()
    };
    let half = step(&layout) as u32;
    let row = (x - 4 * half..x + 4 * half).map(|x| (x, y)).collect();
    let col = (y - 4 * half..y + 4 * half).map(|y| (x, y)).collect();
    let (across, down) = (worst(row), worst(col));
    assert!(across < 70 && down < 70, "seams {across} {down}");
}

/// AC1: a missing (x, y) pair draws no cell; a null value draws the null
/// colour.
#[test]
fn a_missing_pair_shows_the_background_and_a_null_the_null_colour() {
    let cx = Context::new_blocking().unwrap();
    let mut cells = grid(3, &["a", "b", "c"]);
    // Drop (col 01, row b), null (col 02, row c).
    cells.retain(|c| !(c.col == "01" && c.row == "b"));
    for c in &mut cells {
        if c.col == "02" && c.row == "c" {
            c.value = None;
        }
    }
    let mut c = chart(cells).legends(false);
    let (image, layout) = render(&cx, &mut c, 400, 300);

    let (x, y) = centre(&layout, 3, 1, 1);
    assert_eq!(image.get_pixel(x, y).0, [255, 255, 255, 255], "missing");
    let (x, y) = centre(&layout, 3, 2, 2);
    assert!(
        near(image.get_pixel(x, y).0, NULL_COLOR, 2),
        "null {:?}",
        image.get_pixel(x, y).0
    );
    let (x, y) = centre(&layout, 3, 0, 0);
    assert!(!near(image.get_pixel(x, y).0, NULL_COLOR, 20));
    assert_ne!(image.get_pixel(x, y).0, [255, 255, 255, 255]);
}

/// AC1: the ramp legend is at the right, titled.
#[test]
fn the_ramp_legend_is_at_the_right_with_its_title() {
    let cx = Context::new_blocking().unwrap();
    let mut c = chart(grid(5, &["a", "b"])).legend_title("Visits");
    let (_, layout) = render(&cx, &mut c, 480, 300);
    let legend = layout.legend.expect("a legend");
    assert!(
        legend.x >= layout.plot.right(),
        "{legend:?} {:?}",
        layout.plot
    );
    let title = text_centre(&layout, "Visits");
    assert!(
        title.0 >= legend.x && title.0 <= legend.right(),
        "{title:?}"
    );
    // Ticks at the data's round values.
    for t in ["0", "1", "2", "3", "4"] {
        assert!(
            layout
                .texts
                .iter()
                .any(|r| &*r.run.text == t && r.bounds.x >= layout.plot.right()),
            "legend tick {t}"
        );
    }
}

/// AC2: a diverging scale colours 0 with its midpoint colour.
#[test]
fn a_diverging_scale_paints_zero_with_the_midpoint_colour() {
    let cx = Context::new_blocking().unwrap();
    let scale = Diverging::blue_red().domain(-1.0, 0.0, 1.0);
    let cells = vec![
        cell("a", "r", Some(-1.0)),
        cell("b", "r", Some(0.0)),
        cell("c", "r", Some(1.0)),
    ];
    let mut c = chart(cells)
        .color_value_scale(scale.clone())
        .legend_title("r");
    let (image, layout) = render(&cx, &mut c, 400, 240);
    for (i, v) in [-1.0, 0.0, 1.0].into_iter().enumerate() {
        let (x, y) = centre(&layout, 1, i, 0);
        let got = image.get_pixel(x, y).0;
        assert!(near(got, scale.eval(v), 2), "{v}: {got:?}");
    }
    assert!(
        layout.legend.is_some(),
        "a ramp legend for a Diverging fill"
    );
}

fn visits() -> Vec<Cell> {
    DAYS.iter()
        .enumerate()
        .flat_map(|(d, &day)| {
            (0..24).map(move |h| {
                let hf = h as f64;
                let work = (-((hf - 13.0) / 3.5).powi(2)).exp();
                let evening = (-((hf - 20.0) / 2.5).powi(2)).exp();
                let v = if d < 5 {
                    120.0 * work + 25.0 * evening
                } else {
                    35.0 * work + 90.0 * evening
                } + 4.0 * (d as f64 * 1.7 + hf * 0.9).sin().abs();
                cell(format!("{h:02}"), day, Some(v.round()))
            })
        })
        .collect()
}

/// Check the render against its golden and the visual checks; `svg` also
/// compares the scene's SVG with it.
fn golden(name: &str, resolved: &gup_core::Resolved, image: image::RgbaImage, svg: bool) {
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    let meta = layout_metadata(&resolved.layout);
    harness()
        .run(name, Ok((vr.clone(), meta.clone())))
        .assert_ok();
    if svg {
        svg_marks::assert_matches(name, &resolved.scene, &vr, &meta);
    }
}

/// AC3: activity by weekday and hour.
#[test]
fn heatmap_golden() {
    let cx = Context::new_blocking().unwrap();
    let mut c = chart(visits())
        .title("Visits by weekday and hour")
        .x_title("Hour")
        .legend_title("Visits");
    let resolved = c.resolve(&cx, W as f32, H as f32).unwrap().clone();
    let image = gup_core::ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    // The SVG comparison is skipped: the 168 hairline gaps between cells
    // are drawn with slightly different antialiasing by resvg, which moves
    // a few 4x4 blocks by up to dE 5.4, past the 5.0 limit. The diverging
    // golden has the same marks and is compared.
    golden("gup_core/heatmap", &resolved, image, false);
}

/// A symmetric 6 by 6 correlation matrix.
const CORRELATION: [[f64; 6]; 6] = [
    [1.00, 0.62, -0.12, -0.58, -0.86, 0.31],
    [0.62, 1.00, 0.28, 0.05, -0.41, -0.67],
    [-0.12, 0.28, 1.00, 0.47, 0.19, -0.33],
    [-0.58, 0.05, 0.47, 1.00, 0.71, -0.09],
    [-0.86, -0.41, 0.19, 0.71, 1.00, -0.52],
    [0.31, -0.67, -0.33, -0.09, -0.52, 1.00],
];

/// AC3: a correlation matrix on a diverging scale.
#[test]
fn heatmap_diverging_golden() {
    let cx = Context::new_blocking().unwrap();
    let names = ["Price", "Volume", "Age", "Rating", "Stock", "Returns"];
    let mut cells = Vec::new();
    for (i, &row) in names.iter().enumerate() {
        for (j, &col) in names.iter().enumerate() {
            cells.push(cell(col, row, Some(CORRELATION[i][j])));
        }
    }
    let scale = Diverging::blue_red().domain(-1.0, 0.0, 1.0);
    let mut c = chart(cells)
        .color_value_scale(scale)
        .title("Correlation")
        .legend_title("r");
    let resolved = c.resolve(&cx, W as f32, H as f32).unwrap().clone();
    let image = gup_core::ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    golden("gup_core/heatmap_diverging", &resolved, image, true);
}
