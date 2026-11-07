// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-624: the chart's data as a table.

use gup_core::prelude::*;
use gup_core::{Chart, DataTable};

const W: f32 = 480.0;
const H: f32 = 320.0;

fn cx() -> Context {
    Context::new_blocking().expect("headless GPU context")
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn nothing_before_the_first_resolve() {
    let mut plot = Plot::new();
    plot.circles(vec![(1.0, 2.0)]).x(|r| r.0).y(|r| r.1);
    assert_eq!(plot.data_table(10), None);
}

#[test]
fn a_two_series_line_chart_has_a_series_column_and_rows_in_layer_order() {
    let cx = cx();
    let rows = vec![("A", 0.0, 1.0), ("B", 0.0, 5.0), ("A", 1.0, 2.0)];
    let mut chart = gup_core::line(rows)
        .x(|r| r.1)
        .y(|r| r.2)
        .color(|r| r.0)
        .x_title("Day")
        .y_title("Load");
    chart.resolve(&cx, W, H).unwrap();
    let table = Chart::data_table(&chart, 100).unwrap();
    assert_eq!(table.headers, strs(&["Series", "Day", "Load"]));
    assert_eq!(
        table.rows,
        [
            strs(&["A", "0", "1"]),
            strs(&["A", "1", "2"]),
            strs(&["B", "0", "5"]),
        ]
    );
    assert_eq!(table.omitted, 0);
    assert_eq!(table.caption, chart.describe().unwrap().title);
}

#[test]
fn a_grouped_bar_chart_has_a_row_per_bar() {
    let cx = cx();
    let rows = vec![
        ("Q1", "north", 3.0),
        ("Q1", "south", 5.0),
        ("Q2", "north", 4.0),
        ("Q2", "south", 1.0),
    ];
    let mut chart = gup_core::bar(rows)
        .x(|r| r.0)
        .y(|r| r.2)
        .group(|r| r.1)
        .x_title("Quarter")
        .y_title("Sales");
    chart.resolve(&cx, W, H).unwrap();
    let table = Chart::data_table(&chart, 100).unwrap();
    assert_eq!(table.rows.len(), 4);
    assert!(table.headers.contains(&"Quarter".to_owned()));
    assert!(table.headers.contains(&"Sales".to_owned()));
    let q = table.headers.iter().position(|h| h == "Quarter").unwrap();
    let s = table.headers.iter().position(|h| h == "Sales").unwrap();
    let mut seen: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|r| (r[q].clone(), r[s].clone()))
        .collect();
    seen.sort();
    assert_eq!(
        seen,
        [
            ("Q1".into(), "3".into()),
            ("Q1".into(), "5".into()),
            ("Q2".into(), "1".into()),
            ("Q2".into(), "4".into()),
        ]
    );
}

#[test]
fn a_box_plot_has_a_row_per_box_then_the_outliers() {
    let cx = cx();
    let mut data: Vec<(&'static str, f64)> = Vec::new();
    for c in ["a", "b"] {
        for v in 10..=19 {
            data.push((c, f64::from(v)));
        }
    }
    data.push(("b", 60.0));
    let mut chart = gup_core::boxplot(data)
        .x(|r| r.0)
        .y(|r| r.1)
        .y_title("Latency (ms)");
    chart.resolve(&cx, W, H).unwrap();
    let table = Chart::data_table(&chart, 100).unwrap();
    assert_eq!(
        table.headers,
        strs(&[
            "x",
            "Median",
            "Q1",
            "Q3",
            "Whiskers",
            "Outliers",
            "Latency (ms)"
        ])
    );
    assert_eq!(table.rows.len(), 3);
    assert_eq!(table.rows[0][0], "a");
    assert_eq!(table.rows[0][4], "10\u{2013}19");
    assert_eq!(table.rows[0][5], "0");
    assert_eq!(table.rows[1][4], "10\u{2013}19");
    assert_eq!(table.rows[1][5], "1");
    // The outlier: its x and its value only.
    assert_eq!(table.rows[2][0], "b");
    assert_eq!(table.rows[2][1], "");
    assert_eq!(table.rows[2][6], "60");
}

#[test]
fn the_cap_includes_the_first_rows_and_counts_the_rest() {
    let cx = cx();
    let rows: Vec<(f64, f64)> = (0..10).map(|i| (f64::from(i), f64::from(i * i))).collect();
    let mut plot = Plot::new();
    plot.circles(rows).x(|r| r.0).y(|r| r.1);
    plot.resolve(&cx, W, H).unwrap();
    let table = plot.data_table(3).unwrap();
    assert_eq!(table.rows.len(), 3);
    assert_eq!(table.omitted, 7);
    assert!(
        table.caption.ends_with(" (first 3 of 10 rows)"),
        "{}",
        table.caption
    );
    assert_eq!(table.rows[2], strs(&["2", "4"]));
    let all = plot.data_table(10).unwrap();
    assert_eq!((all.rows.len(), all.omitted), (10, 0));
    assert!(!all.caption.contains("first"));
}

#[test]
fn a_columnar_layer_has_a_table_too() {
    let cx = cx();
    let cols = Columns::new()
        .with("x", vec![1.0_f64, 2.0, 3.0])
        .unwrap()
        .with("y", vec![0.5_f32, 1.5, 2.5])
        .unwrap();
    let mut plot = Plot::new();
    plot.add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"]);
    plot.resolve(&cx, W, H).unwrap();
    let table: DataTable = plot.data_table(100).unwrap();
    assert_eq!(table.headers, strs(&["x", "y"]));
    assert_eq!(table.rows[1], strs(&["2", "1.5"]));
    assert_eq!(table.rows.len(), 3);
}

#[test]
fn an_appended_row_is_in_the_next_table() {
    let cx = cx();
    let mut plot = Plot::new();
    let layer = plot.circles(vec![(1.0, 2.0)]);
    layer.x(|r| r.0).y(|r| r.1);
    let appender = layer.appender();
    plot.resolve(&cx, W, H).unwrap();
    let before = plot.describe().unwrap().alt_text();
    assert_eq!(plot.data_table(10).unwrap().rows.len(), 1);
    appender.push((3.0, 4.0)).unwrap();
    plot.resolve(&cx, W, H).unwrap();
    assert_ne!(plot.describe().unwrap().alt_text(), before);
    assert_eq!(plot.data_table(10).unwrap().rows.len(), 2);
}
