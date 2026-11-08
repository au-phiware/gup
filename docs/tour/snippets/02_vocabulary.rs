// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 2 of the tour: one builder for each kind of chart.

use gup::prelude::*;

fn students() -> Vec<(f64, f64, &'static str)> {
    (0..24)
        .map(|i| {
            let hours = f64::from(i) / 3.0 + 1.0;
            let score = 40.0 + 5.0 * hours + f64::from(i * 7 % 11);
            (hours, score, ["Morning", "Evening"][i as usize % 2])
        })
        .collect()
}

fn temperatures() -> Vec<(u32, &'static str, f64)> {
    (1..=12)
        .flat_map(|m| {
            // Hottest in January, coolest in July.
            let season = (f64::from(m - 1) * std::f64::consts::TAU / 12.0).cos();
            [
                (m, "Perth", 25.0 + 7.0 * season),
                (m, "Hobart", 17.0 + 5.0 * season),
            ]
        })
        .collect()
}

fn sales() -> [(&'static str, &'static str, f64); 6] {
    [
        ("Q1", "Phones", 12.0),
        ("Q1", "Laptops", 8.0),
        ("Q2", "Phones", 15.0),
        ("Q2", "Laptops", 9.0),
        ("Q3", "Phones", 11.0),
        ("Q3", "Laptops", 14.0),
    ]
}

fn languages() -> [(&'static str, f64); 4] {
    [("Rust", 41.0), ("Go", 27.0), ("Python", 19.0), ("Zig", 8.0)]
}

fn visits() -> Vec<(f64, &'static str, f64)> {
    (1..=12)
        .flat_map(|month| {
            [("Search", 5.0), ("Social", 3.0), ("Direct", 2.0)].map(move |(source, base)| {
                let m = f64::from(month);
                (m, source, base * (1.0 + m / 12.0 + 0.4 * (m / 2.0).sin()))
            })
        })
        .collect()
}

fn scores() -> Vec<f64> {
    (0..300)
        .map(|i| f64::from((i * 37 % 100 + i * 61 % 100 + i * 17 % 100) / 3))
        .collect()
}

fn customers() -> Vec<(i32, &'static str, f64)> {
    ["Mon", "Tue", "Wed", "Thu", "Fri"]
        .into_iter()
        .enumerate()
        .flat_map(|(d, day)| {
            [9, 12, 15, 18].map(move |hour| (hour, day, ((d * 7 + hour as usize * 3) % 23) as f64))
        })
        .collect()
}

fn requests() -> Vec<(&'static str, f64)> {
    (0..90)
        .map(|i| {
            let service = ["auth", "search", "cart"][i % 3];
            (service, 25.0 + (i * 7 % 31) as f64 * (1 + i % 3) as f64)
        })
        .chain([("auth", 160.0)])
        .collect()
}

pub fn scatter() -> impl Chart {
    // tour:begin scatter
    gup::scatter(students())
        .x(|s| s.0)
        .y(|s| s.1)
        .color(|s| s.2)
        .title("Exam score by study time")
        .x_title("Hours studied per week")
        .y_title("Exam score")
    // tour:end scatter
}

pub fn line() -> impl Chart {
    // tour:begin line
    gup::line(temperatures())
        .x(|r| f64::from(r.0))
        .y(|r| r.2)
        .color(|r| r.1)
        .curve(Curve::MonotoneX)
        .title("Average daily maximum")
        .x_title("Month")
        .y_title("Temperature (°C)")
    // tour:end line
}

pub fn bar() -> impl Chart {
    // tour:begin bar
    gup::bar(sales())
        .x(|s| s.0)
        .y(|s| s.2)
        .group(|s| s.1)
        .title("Units sold, grouped")
        .y_title("Units (thousands)")
    // tour:end bar
}

pub fn bar_stacked() -> impl Chart {
    // tour:begin bar_stacked
    gup::bar(sales())
        .x(|s| s.0)
        .y(|s| s.2)
        .stack(|s| s.1)
        .title("Units sold, stacked")
        .y_title("Units (thousands)")
    // tour:end bar_stacked
}

pub fn barh() -> impl Chart {
    // tour:begin barh
    gup::barh(languages())
        .y(|l| l.0)
        .x(|l| l.1)
        .title("Services by language")
        .x_title("Share of services (%)")
    // tour:end barh
}

pub fn area() -> impl Chart {
    // tour:begin area
    gup::area(visits())
        .x(|v| v.0)
        .y(|v| v.2)
        .stack(|v| v.1)
        .title("Visits by source")
        .x_title("Month")
        .y_title("Visits (thousands)")
    // tour:end area
}

pub fn histogram() -> impl Chart {
    // tour:begin histogram
    gup::histogram(scores())
        .x(|s| *s)
        .bins(Bins::Width(10.0))
        .title("Exam scores")
        .x_title("Score")
        .y_title("Students")
    // tour:end histogram
}

pub fn heatmap() -> impl Chart {
    // tour:begin heatmap
    gup::heatmap(customers())
        .x(|v| v.0)
        .y(|v| v.1)
        .color_value(|v| v.2)
        .legend_title("Visits")
        .title("When customers visit")
        .x_title("Hour of day")
    // tour:end heatmap
}

pub fn boxplot() -> impl Chart {
    // tour:begin boxplot
    gup::boxplot(requests())
        .x(|r| r.0)
        .y(|r| r.1)
        .title("Response time by service")
        .y_title("Milliseconds")
    // tour:end boxplot
}

fn main() -> gup::Result<()> {
    gup::show(scatter())
}
