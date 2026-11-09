// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-628: a columnar layer (`Plot::add_columns`) draws, fits, picks and
//! announces exactly what the same rows through closures do.

use gup_core::prelude::*;
use gup_core::{Column, Retain, RowId, SvgTarget, VectorTarget};

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

/// GUP-664: a columnar layer whose rows and CPU columns its policy drops
/// (`GpuOnly` with small chunks, as `Retain::Auto` does above
/// `Retain::AUTO_MAX_ROWS` rows) still picks, re-encodes a channel, draws
/// on another context and writes SVG, reading its shared columns, exactly
/// as a layer of rows that keeps everything.
#[test]
fn a_released_columnar_layer_reads_its_shared_columns() {
    let cx = cx();
    let rows = rows();
    let cols = columns(&rows);

    let mut by_rows = Plot::new();
    let (rx, ry) = (by_rows.x(Linear::new()), by_rows.y(Linear::new()));
    let rows_colour = ScaleRef::new(Sequential::viridis());
    by_rows
        .circles(rows.clone())
        .x(|r| r.0)
        .y(|r| r.1)
        .attr(Circle::FILL, rows_colour.encode(|r: &(f64, f64, f64)| r.2))
        .attr(Circle::RADIUS, Px(3.0))
        .max_chunk_rows(1024);
    let expected = see(&cx, &mut by_rows, &rx, &ry, Some(&rows_colour));

    let mut by_columns = Plot::new();
    let (x, y) = (by_columns.x(Linear::new()), by_columns.y(Linear::new()));
    let colour = ScaleRef::new(Sequential::viridis());
    by_columns
        .add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .attr(Circle::FILL, colour.encode_column(&cols["v"]))
        .attr(Circle::RADIUS, Px(3.0))
        .retain(Retain::GpuOnly)
        .max_chunk_rows(1024);
    let actual = see(&cx, &mut by_columns, &x, &y, Some(&colour));
    let layer = by_columns.layer_mut::<(), Circle>().unwrap();
    assert!(layer.rows_released(), "GpuOnly dropped the rows");
    assert!(expected.picks.iter().flatten().count() >= 10);
    assert!(actual == expected, "the released columnar scatter differs");
    let v_domain = actual.colour;

    // Re-encoding a channel evaluates the columns again without the rows.
    by_rows
        .layer_mut::<(f64, f64, f64), Circle>()
        .unwrap()
        .attr(Circle::FILL, rows_colour.encode(|r: &(f64, f64, f64)| r.1));
    by_columns
        .layer_mut::<(), Circle>()
        .unwrap()
        .attr(Circle::FILL, colour.encode_column(&cols["y"]));
    let expected = see(&cx, &mut by_rows, &rx, &ry, Some(&rows_colour));
    let actual = see(&cx, &mut by_columns, &x, &y, Some(&colour));
    assert_ne!(actual.colour, v_domain, "the colour reads y now");
    assert!(
        actual == expected,
        "the re-encoded columnar scatter differs"
    );

    // Another context uploads from the shared columns.
    let other = Context::new_blocking().unwrap();
    let image = by_columns.render(&other, W, H).unwrap().into_raw();
    assert!(image == expected.image, "another context draws otherwise");

    // SVG reads the shared columns too.
    let svg = |plot: &mut Plot| {
        let scene = plot.resolve(&cx, W as f32, H as f32).unwrap().scene.clone();
        let mut target = SvgTarget::new();
        target.render(&scene).unwrap();
        target.into_string()
    };
    let (want, got) = (svg(&mut by_rows), svg(&mut by_columns));
    assert!(got.matches("<circle ").count() > N / 2, "{got:.400}");
    assert!(got == want, "the SVG differs");
}

/// GUP-629: key columns.
mod keys {
    use super::*;
    use gup_core::{KeyColumn, PointScale};
    use std::sync::Arc;

    type Row = (Option<&'static str>, Option<&'static str>, f64);

    const SPECIES: [&str; 12] = [
        "wren", "ash", "kite", "emu", "owl", "dove", "finch", "crow", "lark", "tern", "swift",
        "heron",
    ];
    const COLOURS: [&str; 5] = ["red", "teal", "amber", "plum", "moss"];

    /// (species, colour, y) per row: labels drawn in no sorted order, with
    /// nulls in both keys.
    fn keyed_rows() -> Vec<Row> {
        let mut s = 7u64;
        let mut next = move || {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (s >> 33) as usize
        };
        (0..N)
            .map(|i| {
                let species = (i % 211 != 5).then(|| SPECIES[next() % 12]);
                let colour = (i % 97 != 3).then(|| COLOURS[next() % 5]);
                (species, colour, (next() % 1000) as f64 / 10.0)
            })
            .collect()
    }

    /// Codes over `labels` listed in alphabetical order, plus one label no
    /// row uses, so the codes are not in first-seen order.
    fn key_column(rows: &[Option<&'static str>]) -> KeyColumn {
        let mut labels: Vec<&str> = rows.iter().flatten().copied().collect();
        labels.sort_unstable();
        labels.dedup();
        labels.push("zz-unused");
        let codes: Vec<u32> = rows
            .iter()
            .map(|r| match r {
                Some(l) => labels.iter().position(|x| x == l).unwrap() as u32,
                None => gup_core::column::NULL_CODE,
            })
            .collect();
        let labels: Vec<Arc<str>> = labels.into_iter().map(Arc::from).collect();
        KeyColumn::new(codes, labels).unwrap()
    }

    struct Rendered {
        image: Vec<u8>,
        x_keys: Vec<Arc<str>>,
        colour_keys: Vec<Arc<str>>,
        legends: String,
    }

    fn render(
        cx: &Context,
        mut plot: Plot,
        x: &ScaleRef<PointScale>,
        c: &ScaleRef<Categorical>,
    ) -> Rendered {
        let image = plot.render(cx, W, H).unwrap().into_raw();
        let legends = format!("{:?}", plot.resolve(cx, W as f32, H as f32).unwrap().layout);
        let keys = (x.read().keys().to_vec(), c.read().keys().to_vec());
        Rendered {
            image,
            x_keys: keys.0,
            colour_keys: keys.1,
            legends,
        }
    }

    #[test]
    fn a_key_column_scatter_matches_the_key_closure_one() {
        let cx = cx();
        let rows = keyed_rows();

        let mut plot = Plot::new();
        let (x, _y) = (plot.x(PointScale::new()), plot.y(Linear::new()));
        let c = ScaleRef::new(Categorical::okabe_ito());
        plot.circles(rows.clone())
            .attr(Circle::X, x.encode_nullable_key(|r: &Row| r.0))
            .y(|r| r.2)
            .attr(Circle::FILL, c.encode_nullable_key(|r: &Row| r.1))
            .attr(Circle::RADIUS, Px(3.0));
        let expected = render(&cx, plot, &x, &c);

        let species: Vec<_> = rows.iter().map(|r| r.0).collect();
        let colours: Vec<_> = rows.iter().map(|r| r.1).collect();
        let cols = Columns::new()
            .with("species", key_column(&species))
            .unwrap()
            .with("colour", key_column(&colours))
            .unwrap()
            .with("y", rows.iter().map(|r| r.2).collect::<Vec<_>>())
            .unwrap();
        let mut plot = Plot::new();
        let (x, _y) = (plot.x(PointScale::new()), plot.y(Linear::new()));
        let c = ScaleRef::new(Categorical::okabe_ito());
        plot.add_columns::<Circle>(&cols)
            .x_column(&cols["species"])
            .y_column(&cols["y"])
            .attr(Circle::FILL, c.encode_column(&cols["colour"]))
            .attr(Circle::RADIUS, Px(3.0));
        let actual = render(&cx, plot, &x, &c);

        // First-seen order, not the column's label order; no unused label.
        assert_eq!(actual.x_keys.len(), 12);
        assert_eq!(actual.colour_keys.len(), 5);
        assert!(!actual.x_keys.iter().any(|k| &**k == "zz-unused"));
        assert_eq!(actual.x_keys, expected.x_keys);
        assert_eq!(actual.colour_keys, expected.colour_keys);
        assert_eq!(actual.legends, expected.legends);
        assert!(expected.image.iter().any(|&b| b != 255));
        assert!(actual.image == expected.image, "the images differ");
    }

    #[test]
    fn a_released_key_column_layer_announces_its_rows_like_the_closure_one() {
        let cx = cx();
        let rows = keyed_rows();
        let rows = &rows[..3000];

        let mut by_rows = Plot::new();
        let (x, _y) = (by_rows.x(PointScale::new()), by_rows.y(Linear::new()));
        let c = ScaleRef::new(Categorical::okabe_ito());
        by_rows
            .circles(rows.to_vec())
            .attr(Circle::X, x.encode_nullable_key(|r: &Row| r.0))
            .y(|r| r.2)
            .attr(Circle::FILL, c.encode_nullable_key(|r: &Row| r.1));
        by_rows.resolve(&cx, 320.0, 200.0).unwrap();

        let species: Vec<_> = rows.iter().map(|r| r.0).collect();
        let colours: Vec<_> = rows.iter().map(|r| r.1).collect();
        let cols = Columns::new()
            .with("species", key_column(&species))
            .unwrap()
            .with("colour", key_column(&colours))
            .unwrap()
            .with("y", rows.iter().map(|r| r.2).collect::<Vec<_>>())
            .unwrap();
        let mut by_columns = Plot::new();
        let (_x, _y) = (by_columns.x(PointScale::new()), by_columns.y(Linear::new()));
        let c = ScaleRef::new(Categorical::okabe_ito());
        by_columns
            .add_columns::<Circle>(&cols)
            .x_column(&cols["species"])
            .y_column(&cols["y"])
            .attr(Circle::FILL, c.encode_column(&cols["colour"]))
            .retain(Retain::GpuOnly)
            .max_chunk_rows(1024);
        by_columns.resolve(&cx, 320.0, 200.0).unwrap();

        for row in [3, 5, 7, 1500, 2999] {
            let id = RowId { layer: 0, row };
            let (a, b) = (by_columns.row_fields(id), by_rows.row_fields(id));
            assert!(b.is_some(), "row {row}");
            assert_eq!(a, b, "row {row}");
        }
    }

    #[test]
    fn key_column_constructors() {
        let k = KeyColumn::from_labels(["a", "b", "a"]);
        assert_eq!(k.codes(), [0, 1, 0]);
        assert_eq!(k.labels(), [Arc::<str>::from("a"), Arc::from("b")]);
        assert_eq!((k.len(), k.is_empty()), (3, false));
        let k = KeyColumn::from_optional_labels([Some("a"), None, Some("b")]);
        assert_eq!(k.codes(), [0, gup_core::column::NULL_CODE, 1]);

        let labels = |l: &[&str]| l.iter().map(|&s| Arc::from(s)).collect::<Vec<Arc<str>>>();
        let err = KeyColumn::new(vec![0, 1, 2], labels(&["a", "b"])).unwrap_err();
        assert!(err.to_string().contains("row 2 has code 2"), "{err}");
        let err = KeyColumn::new(vec![0], labels(&["a", "b", "a"])).unwrap_err();
        assert!(err.to_string().contains("`a` is listed twice"), "{err}");
        assert!(KeyColumn::new(vec![gup_core::column::NULL_CODE], labels(&["a"])).is_ok());
    }

    #[test]
    fn a_column_of_the_wrong_kind_is_an_error_naming_channel_and_kinds() {
        let cx = cx();
        let cols = Columns::new()
            .with("n", vec![1.0, 2.0])
            .unwrap()
            .with("k", KeyColumn::from_labels(["a", "b"]))
            .unwrap();
        let resolve = |plot: &mut Plot| plot.resolve(&cx, 320.0, 200.0).unwrap_err().to_string();
        let has = |err: &str, parts: &[&str]| {
            assert!(parts.iter().all(|p| err.contains(p)), "{err}");
        };

        // A key column on a numeric axis.
        let mut plot = Plot::new();
        plot.add_columns::<Circle>(&cols)
            .x_column(&cols["k"])
            .y_column(&cols["n"]);
        has(
            &resolve(&mut plot),
            &["`Circle::X`", "numeric", "a key column"],
        );

        // A numeric column on a key axis.
        let mut plot = Plot::new();
        plot.x(PointScale::new());
        plot.add_columns::<Circle>(&cols)
            .x_column(&cols["n"])
            .y_column(&cols["n"]);
        has(
            &resolve(&mut plot),
            &["`Circle::X`", "Band or PointScale", "an f64 column"],
        );

        // Categorical over a numeric column; Sequential over a key column.
        let mut plot = Plot::new();
        plot.add_columns::<Circle>(&cols)
            .x_column(&cols["n"])
            .y_column(&cols["n"])
            .attr(
                Circle::FILL,
                Categorical::okabe_ito().encode_column(&cols["n"]),
            );
        has(
            &resolve(&mut plot),
            &["`Circle::FILL`", "a key column", "an f64 column"],
        );
        let mut plot = Plot::new();
        plot.add_columns::<Circle>(&cols)
            .x_column(&cols["n"])
            .y_column(&cols["n"])
            .attr(
                Circle::FILL,
                Sequential::viridis().encode_column(&cols["k"]),
            );
        has(
            &resolve(&mut plot),
            &["`Circle::FILL`", "numeric column", "a key column"],
        );
    }
}
