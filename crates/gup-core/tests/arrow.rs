// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-630: a layer built from an Arrow record batch draws the same bytes as
//! the same data through `Columns::new().with(..)`.

#![cfg(feature = "arrow")]

use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray};
use gup_core::KeyColumn;
use gup_core::prelude::*;
use std::sync::Arc;

const N: usize = 2_000;

fn cx() -> Context {
    Context::new_blocking().expect("headless GPU context")
}

fn xs() -> Vec<f64> {
    (0..N).map(|i| i as f64 * 0.5 + 10.0).collect()
}

fn ys() -> Vec<f64> {
    (0..N).map(|i| ((i * 37) % 101) as f64 - 50.0).collect()
}

fn scatter(cx: &Context, cols: &Columns) -> Vec<u8> {
    let mut plot = Plot::new();
    plot.x(Linear::new());
    plot.y(Linear::new());
    plot.add_columns::<Circle>(cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .attr(Circle::RADIUS, Px(3.0));
    plot.render(cx, 320, 240).unwrap().into_raw()
}

#[test]
fn an_arrow_scatter_matches_the_columns_one() {
    let cx = cx();
    let batch = RecordBatch::try_from_iter([
        ("x", Arc::new(Float64Array::from(xs())) as ArrayRef),
        ("y", Arc::new(Float64Array::from(ys()))),
    ])
    .unwrap();
    let from_arrow = Columns::from_arrow(&batch).unwrap();
    let by_hand = Columns::new()
        .with("x", xs())
        .unwrap()
        .with("y", ys())
        .unwrap();
    let (a, b) = (scatter(&cx, &from_arrow), scatter(&cx, &by_hand));
    assert!(a.iter().any(|&p| p != 255), "the image is blank");
    assert!(
        a == b,
        "{} bytes differ",
        a.iter().zip(&b).filter(|(a, b)| a != b).count()
    );
}

#[test]
fn an_arrow_string_column_colours_like_a_key_column() {
    let cx = cx();
    let kinds: Vec<&str> = (0..N).map(|i| ["a", "b", "c"][i % 3]).collect();
    let batch = RecordBatch::try_from_iter([
        ("x", Arc::new(Float64Array::from(xs())) as ArrayRef),
        ("y", Arc::new(Float64Array::from(ys()))),
        ("kind", Arc::new(StringArray::from(kinds.clone()))),
    ])
    .unwrap();
    let draw = |cols: &Columns| {
        let mut plot = Plot::new();
        plot.x(Linear::new());
        plot.y(Linear::new());
        let colour = ScaleRef::new(Categorical::okabe_ito());
        plot.add_columns::<Circle>(cols)
            .x_column(&cols["x"])
            .y_column(&cols["y"])
            .attr(Circle::FILL, colour.encode_column(&cols["kind"]))
            .attr(Circle::RADIUS, Px(3.0));
        plot.render(&cx, 320, 240).unwrap().into_raw()
    };
    let by_hand = Columns::new()
        .with("x", xs())
        .unwrap()
        .with("y", ys())
        .unwrap()
        .with("kind", KeyColumn::from_labels(kinds))
        .unwrap();
    let (a, b) = (draw(&Columns::from_arrow(&batch).unwrap()), draw(&by_hand));
    assert!(a == b, "the images differ");
}
