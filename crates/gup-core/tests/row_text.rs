// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-621: what a screen reader says when keyboard focus lands on a row.

use gup_core::prelude::*;
use gup_core::{Input, Key, Retain, RowId};

const W: f32 = 480.0;
const H: f32 = 320.0;

fn id(layer: usize, row: usize) -> RowId {
    RowId { layer, row }
}

fn cx() -> Context {
    Context::new_blocking().expect("headless GPU context")
}

fn fields(plot: &Plot, layer: usize, row: usize) -> Vec<(&str, String)> {
    let f = plot
        .row_fields(id(layer, row))
        .unwrap_or_else(|| panic!("row {layer}/{row} has fields"));
    // Borrow the names from a leaked copy: test-only.
    f.into_iter()
        .map(|(n, v)| (&*Box::leak(n.into_boxed_str()), v))
        .collect()
}

fn pairs<'a>(f: &'a [(&'a str, String)]) -> Vec<(&'a str, &'a str)> {
    f.iter().map(|(n, v)| (*n, v.as_str())).collect()
}

#[test]
fn a_scatter_row_is_named_by_the_axis_titles_or_x_and_y() {
    let cx = cx();
    let rows = vec![(1.0, 2.5), (3.0, 4.0)];
    let mut titled = Plot::new();
    titled.circles(rows.clone()).x(|r| r.0).y(|r| r.1);
    titled.x_title("Mass (kg)").y_title("Span (m)");
    titled.resolve(&cx, W, H).unwrap();
    assert_eq!(
        pairs(&fields(&titled, 0, 1)),
        [("Mass (kg)", "3"), ("Span (m)", "4")]
    );
    let mut bare = Plot::new();
    bare.circles(rows).x(|r| r.0).y(|r| r.1);
    bare.resolve(&cx, W, H).unwrap();
    assert_eq!(pairs(&fields(&bare, 0, 0)), [("x", "1"), ("y", "2.5")]);
    assert_eq!(bare.row_text(id(0, 0)).as_deref(), Some("x: 1; y: 2.5"));
    // A row that does not exist, and a layer that does not.
    assert_eq!(bare.row_fields(id(0, 2)), None);
    assert_eq!(bare.row_fields(id(1, 0)), None);
}

#[test]
fn values_use_three_significant_digits_and_the_axis_format() {
    let cx = cx();
    let mut plot = Plot::new();
    plot.circles(vec![(1234.5678, 0.123_456)])
        .x(|r| r.0)
        .y(|r| r.1);
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(pairs(&fields(&plot, 0, 0)), [("x", "1230"), ("y", "0.123")]);
    plot.x_tick_format(|v| format!("${v:.0}"));
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(
        pairs(&fields(&plot, 0, 0)),
        [("x", "$1235"), ("y", "0.123")]
    );
    assert_eq!(
        plot.row_text(id(0, 0)).as_deref(),
        Some("x: $1235; y: 0.123")
    );
}

#[test]
fn a_chart_builders_format_applies_too() {
    let cx = cx();
    let mut chart = gup_core::scatter(vec![(1500.0, 2.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .x_tick_format(|v| format!("${v:.0}"))
        .x_title("Price");
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(
        chart.plot_mut().row_text(id(0, 0)).as_deref(),
        Some("Price: $1500; y: 2")
    );
}

#[test]
fn a_time_axis_says_an_iso_date() {
    let cx = cx();
    // 2024-02-29T00:00Z, then 14:05 and 14:05:09 the same day.
    let t0 = 1_709_164_800.0;
    let rows = vec![(t0, 1.0), (t0 + 14.0 * 3600.0 + 300.0, 2.0)];
    let mut chart = gup_core::line(rows.clone())
        .x(|r| r.0)
        .y(|r| r.1)
        .x_scale(Time::new());
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(
        chart.plot_mut().row_text(id(0, 0)).as_deref(),
        Some("x: 2024-02-29; y: 1")
    );
    assert_eq!(
        chart.plot_mut().row_text(id(0, 1)).as_deref(),
        Some("x: 2024-02-29T14:05; y: 2")
    );
    let mut seconds = gup_core::line(vec![(t0 + 9.0, 1.0), (t0 + 90_009.0, 3.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .x_scale(Time::new());
    seconds.resolve(&cx, W, H).unwrap();
    assert_eq!(
        seconds.plot_mut().row_text(id(0, 1)).as_deref(),
        Some("x: 2024-03-01T01:00:09; y: 3")
    );
}

#[test]
fn a_colour_key_is_the_series_field_and_a_colour_value_is_last() {
    let cx = cx();
    let rows = vec![
        (1.0, 2.0, "Africa", 10.0),
        (2.0, 3.0, "Asia", 20.0),
        (3.0, 1.0, "Africa", 30.0),
    ];
    let mut keyed = gup_core::scatter(rows.clone())
        .x(|r| r.0)
        .y(|r| r.1)
        .color(|r| r.2)
        .legend_title("Region");
    keyed.resolve(&cx, W, H).unwrap();
    assert_eq!(
        pairs(&fields(keyed.plot_mut(), 0, 1)),
        [("Region", "Asia"), ("x", "2"), ("y", "3")]
    );
    let mut valued = gup_core::scatter(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .color_value(|r| r.3);
    valued.resolve(&cx, W, H).unwrap();
    assert_eq!(
        pairs(&fields(valued.plot_mut(), 0, 2)),
        [("x", "3"), ("y", "1"), ("Value", "30")]
    );
    // Without a legend title the series is just that.
    let mut plain = gup_core::scatter(vec![(1.0, 2.0, "Asia")])
        .x(|r| r.0)
        .y(|r| r.1)
        .color(|r| r.2);
    plain.resolve(&cx, W, H).unwrap();
    assert_eq!(
        plain.plot_mut().row_text(id(0, 0)).as_deref(),
        Some("Series: Asia; x: 1; y: 2")
    );
}

#[test]
fn a_line_series_leads_its_rows() {
    let cx = cx();
    let rows = vec![("A", 0.0, 1.0), ("B", 0.0, 5.0), ("A", 1.0, 2.0)];
    let mut chart = gup_core::line(rows).x(|r| r.1).y(|r| r.2).color(|r| r.0);
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(
        chart.plot_mut().row_text(id(1, 0)).as_deref(),
        Some("Series: B; x: 0; y: 5")
    );
    assert_eq!(
        chart.plot_mut().row_text(id(0, 1)).as_deref(),
        Some("Series: A; x: 1; y: 2")
    );
}

#[test]
fn a_heatmap_cell_says_its_two_keys_and_its_value() {
    let cx = cx();
    let rows = vec![
        ("Mon", "AM", 3.0),
        ("Tue", "AM", 5.0),
        ("Mon", "PM", 8.5),
        ("Tue", "PM", 1.0),
    ];
    let mut chart = gup_core::heatmap(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .color_value(|r| r.2)
        .x_title("Day")
        .y_title("Shift");
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(
        pairs(&fields(chart.plot_mut(), 0, 2)),
        [("Day", "Mon"), ("Shift", "PM"), ("Value", "8.5")]
    );
}

#[test]
fn a_stacked_bar_segment_says_its_own_value_not_its_top() {
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
        .stack(|r| r.1)
        .x_title("Quarter")
        .y_title("Sales");
    chart.resolve(&cx, W, H).unwrap();
    // The south segment of Q1 sits on top of north's 3, ending at 8.
    assert_eq!(
        pairs(&fields(chart.plot_mut(), 1, 0)),
        [("Series", "south"), ("Quarter", "Q1"), ("Sales", "5")]
    );
    // A plain bar chart: no series.
    let mut plain = gup_core::bar([("a", 2.0), ("b", 7.0)])
        .x(|r| r.0)
        .y(|r| r.1);
    plain.resolve(&cx, W, H).unwrap();
    assert_eq!(
        plain.plot_mut().row_text(id(0, 1)).as_deref(),
        Some("x: b; y: 7")
    );
    // A horizontal bar chart reads the band from y.
    let mut wide = gup_core::barh([("a", 2.0), ("b", 7.0)])
        .y(|r| r.0)
        .x(|r| r.1)
        .y_title("Fruit")
        .x_title("Count");
    wide.resolve(&cx, W, H).unwrap();
    assert_eq!(
        wide.plot_mut().row_text(id(0, 0)).as_deref(),
        Some("Fruit: a; Count: 2")
    );
}

#[test]
fn a_histogram_bin_says_its_range_and_count() {
    let cx = cx();
    let values: Vec<f64> = (0..20).map(|i| f64::from(i) + 10.0).collect();
    let mut chart = gup_core::histogram(values)
        .x(|v| *v)
        .bins(Bins::Width(10.0));
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(
        pairs(&fields(chart.plot_mut(), 0, 0)),
        [("Bin", "10\u{2013}20"), ("Count", "10")]
    );
    let mut keyed = gup_core::histogram((0..20).map(|i| (f64::from(i) + 10.0, i % 2 == 0)))
        .x(|v| v.0)
        .color(|v| if v.1 { "even" } else { "odd" })
        .bins(Bins::Width(10.0));
    keyed.resolve(&cx, W, H).unwrap();
    assert_eq!(
        keyed.plot_mut().row_text(id(1, 0)).as_deref(),
        Some("Series: odd; Bin: 10\u{2013}20; Count: 5")
    );
}

#[test]
fn a_box_says_its_five_numbers_and_an_outlier_its_value() {
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
    let plot = chart.plot_mut();
    assert_eq!(
        pairs(&fields(plot, 0, 1)),
        [
            ("x", "b"),
            ("Median", "15"),
            ("Q1", "12.5"),
            ("Q3", "17.5"),
            ("Whiskers", "10\u{2013}19"),
            ("Outliers", "1"),
        ]
    );
    // The outlier discs are the last layer.
    let outliers = (0..8)
        .rev()
        .find(|&l| plot.row_fields(id(l, 0)).is_some())
        .unwrap();
    assert_eq!(
        plot.row_text(id(outliers, 0)).as_deref(),
        Some("x: b; Latency (ms): 60")
    );
}

#[test]
fn every_row_reads_back_whatever_the_storage() {
    let cx = cx();
    // Sizes shuffled, so the draw order is not the row order; many chunks.
    let rows: Vec<(f64, f64, f64)> = (0..40)
        .map(|i| (f64::from(i), f64::from(i) * 2.0, f64::from((i * 7) % 40)))
        .collect();
    for retain in [Retain::Auto, Retain::Rows, Retain::Columns] {
        let mut plot = Plot::new();
        plot.circles(rows.clone())
            .x(|r| r.0)
            .y(|r| r.1)
            .radius(
                Pow::sqrt()
                    .range(Px(2.0), Px(12.0))
                    .encode(|r: &(f64, f64, f64)| r.2),
            )
            .max_chunk_rows(8)
            .retain(retain);
        plot.resolve(&cx, W, H).unwrap();
        for (i, r) in rows.iter().enumerate() {
            let want = [("x", format!("{}", r.0)), ("y", format!("{}", r.1))];
            let got = fields(&plot, 0, i);
            let got: Vec<_> = got.iter().map(|(n, v)| (*n, v.clone())).collect();
            // Three significant digits: 40 -> 40, 78 -> 78.
            assert_eq!(got, want, "{retain:?} row {i}");
        }
    }
}

#[test]
fn a_layer_with_neither_rows_nor_cpu_columns_says_nothing() {
    let cx = cx();
    let rows: Vec<(f64, f64)> = (0..41).map(|i| (f64::from(i), 1.0)).collect();
    let mut plot = Plot::new();
    plot.circles(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .max_chunk_rows(8)
        .retain(Retain::GpuOnly);
    plot.resolve(&cx, W, H).unwrap();
    // The last chunk keeps its CPU copy; the full ones do not.
    assert_eq!(plot.row_fields(id(0, 2)), None);
    assert!(plot.row_fields(id(0, 40)).is_some());
}

#[test]
fn a_tooltip_is_the_row_text_with_its_lines_joined() {
    let cx = cx();
    let mut plot = Plot::new();
    plot.circles(vec![(1.0, 2.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .tooltip(|r| format!("Point\nx = {}", r.0));
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(plot.row_text(id(0, 0)).as_deref(), Some("Point; x = 1"));
}

#[test]
fn focus_text_follows_focus_and_counts_the_layers_rows() {
    let cx = cx();
    let mut plot = Plot::new();
    plot.circles(vec![(0.8, 0.9), (0.9, 0.95), (1.0, 0.99)])
        .x(|r| r.0)
        .y(|r| r.1);
    plot.x_title("Mass").y_title("Span");
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(plot.focus_text(), None);
    let _ = plot.handle(Input::key(Key::Right));
    assert_eq!(plot.focus_text(), Some("Mass: 0.8; Span: 0.9 (1 of 3)"));
    let _ = plot.handle(Input::key(Key::Right));
    assert_eq!(plot.focus_text(), Some("Mass: 0.9; Span: 0.95 (2 of 3)"));
    let _ = plot.handle(Input::key(Key::End));
    assert_eq!(plot.focus_text(), Some("Mass: 1; Span: 0.99 (3 of 3)"));
    let _ = plot.handle(Input::key(Key::Escape));
    assert_eq!(plot.focus_text(), None);
    // Through the Chart trait, on a builder.
    let mut chart: Box<dyn Chart> = Box::new(
        gup_core::scatter(vec![(1.0, 2.0), (2.0, 3.0)])
            .x(|r| r.0)
            .y(|r| r.1),
    );
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(chart.focus_text(), None);
    let _ = chart.handle(Input::key(Key::Right));
    assert_eq!(chart.focus_text(), Some("x: 1; y: 2 (1 of 2)"));
}

#[test]
fn an_append_while_focused_updates_the_count() {
    let cx = cx();
    let mut chart = gup_core::scatter(vec![(1.0, 2.0), (2.0, 3.0)])
        .x(|r| r.0)
        .y(|r| r.1);
    let feed = chart.appender();
    chart.resolve(&cx, W, H).unwrap();
    let _ = chart.handle(Input::key(Key::Right));
    assert_eq!(chart.focus_text(), Some("x: 1; y: 2 (1 of 2)"));
    feed.push((3.0, 4.0)).unwrap();
    chart.resolve(&cx, W, H).unwrap();
    assert_eq!(chart.focus_text(), Some("x: 1; y: 2 (1 of 3)"));
}
