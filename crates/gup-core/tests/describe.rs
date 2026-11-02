// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-592: `Plot::describe`, the text alternative: one chart per builder,
//! tick formats, the override, and the cache.

use gup_core::Description;
use gup_core::prelude::*;

fn cx() -> Context {
    Context::new_blocking().expect("a GPU context")
}

/// Resolve `chart` at 640 × 400 and return its description.
fn described(chart: &mut impl gup_core::Chart) -> Description {
    chart.resolve(&cx(), 640.0, 400.0).unwrap();
    let d = chart.describe().expect("resolved").clone();
    println!("{}", d.alt_text());
    d
}

#[test]
fn scatter() {
    let mut chart = gup_core::scatter(vec![(1.0, 2.0), (3.0, 4.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .x_title("Hours");
    let d = described(&mut chart);
    assert_eq!(d.title, "Scatter chart");
    assert!(d.text.starts_with("2 data rows in 1 layer. "), "{}", d.text);
    assert!(d.text.contains("X axis \"Hours\": "), "{}", d.text);
    assert!(d.text.contains(" Y axis: "), "{}", d.text);
}

#[test]
fn line() {
    let mut chart = gup_core::line((0..30).map(|i| (f64::from(i), f64::from(i * i))))
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Growth")
        .subtitle("Squares");
    let d = described(&mut chart);
    assert_eq!(d.title, "Growth");
    assert!(
        d.text
            .starts_with("Squares. Line chart, 30 data rows in 1 layer. X axis: "),
        "{}",
        d.text
    );
}

struct Sale {
    fruit: &'static str,
    sold: f64,
}

fn sales() -> Vec<Sale> {
    vec![
        Sale {
            fruit: "Apple",
            sold: 1200.0,
        },
        Sale {
            fruit: "Pear",
            sold: 800.0,
        },
        Sale {
            fruit: "Plum",
            sold: 400.0,
        },
    ]
}

#[test]
fn bar_with_tick_format() {
    let mut chart = gup_core::bar(sales())
        .x(|s| s.fruit)
        .y(|s| s.sold)
        .title("Sales")
        .y_tick_format(|v| format!("${v:.0}"));
    let d = described(&mut chart);
    assert!(
        d.text.starts_with(
            "Bar chart, 3 data rows in 1 layer. X axis: 3 categories, Apple, Pear and Plum."
        ),
        "{}",
        d.text
    );
    let y = d.text.split("Y axis").nth(1).expect("a y axis sentence");
    assert!(y.contains('$'), "tick formats reach the text: {y}");
}

#[test]
fn barh() {
    let mut chart = gup_core::barh(sales())
        .y(|s| s.fruit)
        .x(|s| s.sold)
        .title("Sales");
    let d = described(&mut chart);
    assert!(
        d.text
            .starts_with("Horizontal bar chart, 3 data rows in 1 layer. "),
        "{}",
        d.text
    );
    assert!(
        d.text
            .contains("Y axis: 3 categories, Plum, Pear and Apple."),
        "{}",
        d.text
    );
}

#[test]
fn area() {
    let mut chart = gup_core::area((0..12).map(|m| (f64::from(m), f64::from(m) * 2.0)))
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Area");
    let d = described(&mut chart);
    assert!(
        d.text
            .starts_with("Area chart, 12 data rows in 1 layer. X axis: "),
        "{}",
        d.text
    );
}

#[test]
fn stacked_area_legend() {
    let rows: Vec<(f64, f64, &str)> = (0..6)
        .flat_map(|m| [(f64::from(m), 1.0, "web"), (f64::from(m), 2.0, "app")])
        .collect();
    let mut chart = gup_core::area(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .color(|r| r.2)
        .legend_title("Source");
    let d = described(&mut chart);
    assert!(
        d.text.ends_with("Colour legend \"Source\": web and app."),
        "{}",
        d.text
    );
}

#[test]
fn histogram() {
    let mut chart = gup_core::histogram((0..100).map(|i| f64::from(i % 17)))
        .x(|v| *v)
        .title("Trips");
    let d = described(&mut chart);
    assert!(d.text.starts_with("Histogram, "), "{}", d.text);
    assert!(d.text.contains(" X axis: "), "{}", d.text);
}

#[test]
fn heatmap() {
    let rows = ["Mon", "Tue", "Wed"]
        .into_iter()
        .flat_map(|d| (0..4u8).map(move |h| (d, h, f64::from(h) * 10.0)));
    let mut chart = gup_core::heatmap(rows)
        .x(|r| r.1)
        .y(|r| r.0)
        .color_value(|r| r.2)
        .legend_title("Visits")
        .title("Visits");
    let d = described(&mut chart);
    assert!(
        d.text.starts_with("Heatmap, 12 data rows in 1 layer. "),
        "{}",
        d.text
    );
    assert!(
        d.text.contains("Y axis: 3 categories, Wed, Tue and Mon."),
        "{}",
        d.text
    );
    assert!(
        d.text.contains("Colour legend \"Visits\": 0 to "),
        "{}",
        d.text
    );
}

#[test]
fn boxplot() {
    let rows: Vec<(&str, f64)> = (0..40)
        .map(|i| (if i % 2 == 0 { "api" } else { "web" }, f64::from(i) * 3.0))
        .collect();
    let mut chart = gup_core::boxplot(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Latency");
    let d = described(&mut chart);
    assert!(d.text.starts_with("Box plot, "), "{}", d.text);
    assert!(
        d.text.contains("X axis: 2 categories, api and web."),
        "{}",
        d.text
    );
}

#[test]
fn plot_layers_and_right_axis() {
    let cx = cx();
    let mut plot = Plot::new();
    plot.title("Climate").y_right_title("Rain (mm)");
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let right = plot.y_right(Linear::new());
    plot.add(Selection::<(f64, f64), Circle>::new(
        (0..20)
            .map(|i| (f64::from(i), f64::from(i)))
            .collect::<Vec<_>>(),
    ))
    .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    .attr(Circle::RADIUS, Px(3.0));
    plot.add(Selection::<(f64, f64), Line>::new(
        (0..10)
            .map(|i| (f64::from(i), f64::from(i)))
            .collect::<Vec<_>>(),
    ))
    .attr(Line::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Line::Y, right.encode(|r: &(f64, f64)| r.1));
    plot.resolve(&cx, 640.0, 400.0).unwrap();
    let d = plot.describe().unwrap();
    assert_eq!(d.title, "Climate");
    assert!(
        d.text
            .starts_with("Chart, 30 data rows in 2 layers (circle 20 and line 10). "),
        "{}",
        d.text
    );
    assert!(
        d.text.contains("Right y axis \"Rain (mm)\": "),
        "{}",
        d.text
    );
    println!("{}", d.alt_text());
}

#[test]
fn override_keeps_derived_title() {
    let mut chart = gup_core::scatter(vec![(1.0, 2.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Mine")
        .description("Two numbers rising together.");
    let d = described(&mut chart);
    assert_eq!(d.title, "Mine");
    assert_eq!(d.text, "Two numbers rising together.");
    assert_eq!(d.alt_text(), "Mine. Two numbers rising together.");
}

#[test]
fn none_before_resolve_and_cached_after() {
    let cx = cx();
    let mut chart = gup_core::scatter(vec![(1.0, 2.0)]).x(|r| r.0).y(|r| r.1);
    assert!(chart.describe().is_none());
    chart.resolve(&cx, 320.0, 200.0).unwrap();
    let first = chart.describe().unwrap() as *const Description;
    chart.resolve(&cx, 320.0, 200.0).unwrap();
    let second = chart.describe().unwrap();
    assert!(std::ptr::eq(first, second), "a cached resolve keeps it");
}

#[test]
fn scene_carries_it() {
    let cx = cx();
    let mut chart = gup_core::scatter(vec![(1.0, 2.0)]).x(|r| r.0).y(|r| r.1);
    let scene = chart.resolve(&cx, 320.0, 200.0).unwrap().scene.clone();
    assert_eq!(scene.description.as_ref(), chart.describe());
    assert_eq!(scene.guides().description, scene.description);
}
