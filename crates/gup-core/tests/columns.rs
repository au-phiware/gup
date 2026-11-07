// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-628: a columnar layer (`Plot::add_columns`) draws, fits, picks and
//! announces exactly what the same rows through closures do.

use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{Column, Retain, RowId};

const W: u32 = 480;
const H: u32 = 320;
const N: usize = 10_000;

fn cx() -> Context {
    Context::new_blocking().expect("headless GPU context")
}

/// `N` rows of (x, y, v): x ascending, y a random walk, v another.
fn rows() -> Vec<(f64, f64, f64)> {
    let mut s = 1u64;
    let mut unit = move || {
        s = s
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut y, mut v) = (0.0, 0.0);
    (0..N)
        .map(|i| {
            y += unit() - 0.5;
            v += unit() - 0.5;
            (i as f64 * 0.37 + 1.0e4, y, v)
        })
        .collect()
}

/// The (x, y) of every 1000th row, from row 500.
fn rows_at() -> impl Iterator<Item = (f64, f64)> {
    rows()
        .into_iter()
        .skip(500)
        .step_by(1000)
        .map(|r| (r.0, r.1))
}

fn columns(rows: &[(f64, f64, f64)]) -> Columns {
    Columns::new()
        .with("x", rows.iter().map(|r| r.0).collect::<Vec<_>>())
        .unwrap()
        .with("y", rows.iter().map(|r| r.1).collect::<Vec<_>>())
        .unwrap()
        .with("v", rows.iter().map(|r| r.2).collect::<Vec<_>>())
        .unwrap()
}

/// What a plot shows and answers, compared between the two paths.
#[derive(Debug, PartialEq)]
struct Seen {
    image: Vec<u8>,
    x: Option<(f64, f64)>,
    y: Option<(f64, f64)>,
    colour: Option<(f64, f64)>,
    picks: Vec<Option<RowId>>,
    fields: Vec<Option<Vec<(String, String)>>>,
}

fn see(
    cx: &Context,
    plot: &mut Plot,
    x: &ScaleRef<Linear>,
    y: &ScaleRef<Linear>,
    colour: Option<&ScaleRef<Sequential>>,
) -> Seen {
    let image = plot.render(cx, W, H).unwrap().into_raw();
    let layout = plot.resolve(cx, W as f32, H as f32).unwrap().layout.clone();
    // The pixel of value `v` on an axis, from its tick pixels.
    let px = |ticks: &[(f64, f32)], v: f64| {
        let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
        (f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)) as f32
    };
    // Ten rows' positions, then a 5 x 2 grid over the plot rect.
    let rect = layout.plot;
    let points: Vec<Point> = rows_at()
        .map(|(x, y)| Point::new(px(&layout.x_ticks, x), px(&layout.y_ticks, y)))
        .chain((0..10).map(|k| {
            let (i, j) = ((k % 5) as f32, (k / 5) as f32);
            Point::new(
                rect.left() + rect.width * (i + 0.5) / 5.0,
                rect.top() + rect.height * (j + 0.5) / 2.0,
            )
        }))
        .collect();
    let picks: Vec<Option<RowId>> = points.into_iter().map(|at| plot.pick_row(at)).collect();
    let fields = picks
        .iter()
        .map(|p| p.and_then(|id| plot.row_fields(id)))
        .collect();
    Seen {
        image,
        x: x.read().current_domain(),
        y: y.read().current_domain(),
        colour: colour.and_then(|c| c.read().current_domain()),
        picks,
        fields,
    }
}

#[test]
fn a_columnar_scatter_matches_the_rows_one() {
    let cx = cx();
    let rows = rows();
    let cols = columns(&rows);

    let mut by_rows = Plot::new();
    let (x, y) = (by_rows.x(Linear::new()), by_rows.y(Linear::new()));
    let colour = ScaleRef::new(Sequential::viridis());
    by_rows
        .circles(rows.clone())
        .x(|r| r.0)
        .y(|r| r.1)
        .attr(Circle::FILL, colour.encode(|r: &(f64, f64, f64)| r.2))
        .attr(Circle::RADIUS, Px(3.0));
    let expected = see(&cx, &mut by_rows, &x, &y, Some(&colour));

    let mut by_columns = Plot::new();
    let (x, y) = (by_columns.x(Linear::new()), by_columns.y(Linear::new()));
    let colour = ScaleRef::new(Sequential::viridis());
    by_columns
        .add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .attr(Circle::FILL, colour.encode_column(&cols["v"]))
        .attr(Circle::RADIUS, Px(3.0));
    let actual = see(&cx, &mut by_columns, &x, &y, Some(&colour));

    let hits = expected.picks.iter().flatten().count();
    assert!(hits >= 10, "{hits} of 20 picks hit a row");
    assert!(expected.fields.iter().flatten().all(|f| f.len() == 3));
    assert!(expected.image.iter().any(|&b| b != 255));
    if actual != expected {
        let differing = (actual.image.iter().zip(&expected.image))
            .filter(|(a, b)| a != b)
            .count();
        panic!(
            "{differing} image bytes differ; domains {:?} vs {:?}, picks {:?} vs {:?}, fields {:?} vs {:?}",
            (actual.x, actual.y, actual.colour),
            (expected.x, expected.y, expected.colour),
            actual.picks,
            expected.picks,
            actual.fields,
            expected.fields
        );
    }
}

#[test]
fn a_columnar_line_matches_the_rows_one() {
    let cx = cx();
    let rows = rows();
    let cols = columns(&rows);

    let mut by_rows = Plot::new();
    let (x, y) = (by_rows.x(Linear::new()), by_rows.y(Linear::new()));
    by_rows.line(rows.clone()).x(|r| r.0).y(|r| r.1);
    let expected = see(&cx, &mut by_rows, &x, &y, None);

    let mut by_columns = Plot::new();
    let (x, y) = (by_columns.x(Linear::new()), by_columns.y(Linear::new()));
    by_columns
        .add_columns::<Line>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"]);
    let actual = see(&cx, &mut by_columns, &x, &y, None);

    assert!(expected.picks.iter().flatten().count() >= 1);
    assert!(actual == expected, "the columnar line differs");
}

#[test]
fn f32_columns_draw_as_their_widened_values() {
    let cx = cx();
    // Values an f32 holds exactly, so the f64 rows are the same numbers.
    let rows: Vec<(f64, f64, f64)> = rows()
        .into_iter()
        .map(|(x, y, v)| {
            let w = |a: f64| f64::from(a as f32);
            (w(x), w(y), w(v))
        })
        .collect();
    let narrow = |i: usize| -> Vec<f32> {
        rows.iter()
            .map(|r| [r.0, r.1, r.2][i] as f32)
            .collect::<Vec<_>>()
    };
    let cols = Columns::new()
        .with("x", narrow(0))
        .unwrap()
        .with("y", narrow(1))
        .unwrap()
        .with("v", narrow(2))
        .unwrap();
    assert!(matches!(cols["x"], Column::F32(_)));

    let draw = |plot: &mut Plot, colour: &ScaleRef<Sequential>, columnar: bool| {
        if columnar {
            plot.add_columns::<Circle>(&cols)
                .x_column(&cols["x"])
                .y_column(&cols["y"])
                .attr(Circle::FILL, colour.encode_column(&cols["v"]));
        } else {
            plot.circles(rows.clone())
                .x(|r| r.0)
                .y(|r| r.1)
                .attr(Circle::FILL, colour.encode(|r: &(f64, f64, f64)| r.2));
        }
    };
    let mut seen = Vec::new();
    for columnar in [false, true] {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
        let colour = ScaleRef::new(Sequential::viridis());
        draw(&mut plot, &colour, columnar);
        seen.push(see(&cx, &mut plot, &x, &y, Some(&colour)));
    }
    assert!(seen[0] == seen[1], "the f32 columnar scatter differs");
}

#[test]
fn a_column_of_another_length_is_an_error_naming_the_channel() {
    let cx = cx();
    let cols = Columns::new()
        .with("x", vec![1.0, 2.0, 3.0])
        .unwrap()
        .with("y", vec![1.0, 2.0, 3.0])
        .unwrap();
    let short: Column = vec![1.0, 2.0].into();
    let mut plot = Plot::new();
    plot.add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .attr(Circle::FILL, Sequential::viridis().encode_column(&short));
    let err = plot.resolve(&cx, 320.0, 200.0).unwrap_err().to_string();
    assert!(
        err.contains("`Circle::FILL` has 2 rows, but the layer has 3"),
        "{err}"
    );

    let long: Column = vec![1.0; 4].into();
    let mut plot = Plot::new();
    plot.add_columns::<Circle>(&cols)
        .x_column(&long)
        .y_column(&cols["y"]);
    let err = plot.resolve(&cx, 320.0, 200.0).unwrap_err().to_string();
    assert!(
        err.contains("`Circle::X` has 4 rows, but the layer has 3"),
        "{err}"
    );
}

#[test]
fn a_columnar_layer_cannot_be_appended_to() {
    let cx = cx();
    let cols = Columns::new().with("x", vec![1.0, 2.0]).unwrap();
    let mut plot = Plot::new();
    let layer = plot
        .add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["x"]);
    let err = layer.append([()]).unwrap_err().to_string();
    assert!(err.contains("columnar layer"), "{err}");
    let appender = layer.appender();
    appender.push(()).unwrap();
    let err = plot.resolve(&cx, 320.0, 200.0).unwrap_err().to_string();
    assert!(err.contains("cannot be appended to"), "{err}");
}

#[test]
fn a_columnar_row_is_announced_after_its_cpu_columns_are_released() {
    let cx = cx();
    let cols = Columns::new()
        .with("x", (0..3000).map(f64::from).collect::<Vec<_>>())
        .unwrap()
        .with("y", vec![2.5_f32; 3000])
        .unwrap();
    let mut plot = Plot::new();
    plot.add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .retain(Retain::GpuOnly)
        .max_chunk_rows(1024);
    plot.resolve(&cx, 320.0, 200.0).unwrap();
    let fields = plot
        .row_fields(RowId { layer: 0, row: 7 })
        .expect("row 7 has fields");
    assert_eq!(
        fields,
        [("x".into(), "7".into()), ("y".into(), "2.5".into())]
    );
}
