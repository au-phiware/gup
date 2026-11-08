// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 2 of the tour: one builder for each kind of chart.

use gup::prelude::*;

struct Student {
    hours: f64,
    score: f64,
    shift: &'static str,
}

struct Temperature {
    month: f64,
    city: &'static str,
    celsius: f64,
}

struct Sale {
    quarter: &'static str,
    product: &'static str,
    units: f64,
}

struct Language {
    name: &'static str,
    share: f64,
}

struct Visit {
    month: f64,
    source: &'static str,
    count: f64,
}

struct Exam {
    score: f64,
}

struct Customer {
    hour: i32,
    day: &'static str,
    visits: f64,
}

struct Request {
    service: &'static str,
    ms: f64,
}

fn students() -> Vec<Student> {
    (0..24)
        .map(|i| {
            let hours = f64::from(i) / 3.0 + 1.0;
            Student {
                hours,
                score: 40.0 + 5.0 * hours + f64::from(i * 7 % 11),
                shift: ["Morning", "Evening"][i as usize % 2],
            }
        })
        .collect()
}

fn temperatures() -> Vec<Temperature> {
    (1..=12)
        .flat_map(|m| {
            // Hottest in January, coolest in July.
            let season = (f64::from(m - 1) * std::f64::consts::TAU / 12.0).cos();
            [
                ("Perth", 25.0 + 7.0 * season),
                ("Hobart", 17.0 + 5.0 * season),
            ]
            .map(|(city, celsius)| Temperature {
                month: f64::from(m),
                city,
                celsius,
            })
        })
        .collect()
}

fn sales() -> Vec<Sale> {
    [
        ("Q1", "Phones", 12.0),
        ("Q1", "Laptops", 8.0),
        ("Q2", "Phones", 15.0),
        ("Q2", "Laptops", 9.0),
        ("Q3", "Phones", 11.0),
        ("Q3", "Laptops", 14.0),
    ]
    .map(|(quarter, product, units)| Sale {
        quarter,
        product,
        units,
    })
    .into()
}

fn languages() -> Vec<Language> {
    [("Rust", 41.0), ("Go", 27.0), ("Python", 19.0), ("Zig", 8.0)]
        .map(|(name, share)| Language { name, share })
        .into()
}

fn visits() -> Vec<Visit> {
    (1..=12)
        .flat_map(|month| {
            [("Search", 5.0), ("Social", 3.0), ("Direct", 2.0)].map(move |(source, base)| {
                let m = f64::from(month);
                Visit {
                    month: m,
                    source,
                    count: base * (1.0 + m / 12.0 + 0.4 * (m / 2.0).sin()),
                }
            })
        })
        .collect()
}

fn exams() -> Vec<Exam> {
    (0..300)
        .map(|i| Exam {
            score: f64::from((i * 37 % 100 + i * 61 % 100 + i * 17 % 100) / 3),
        })
        .collect()
}

fn customers() -> Vec<Customer> {
    ["Mon", "Tue", "Wed", "Thu", "Fri"]
        .into_iter()
        .enumerate()
        .flat_map(|(d, day)| {
            [9, 12, 15, 18].map(move |hour| Customer {
                hour,
                day,
                visits: ((d * 7 + hour as usize * 3) % 23) as f64,
            })
        })
        .collect()
}

fn requests() -> Vec<Request> {
    (0..90)
        .map(|i| Request {
            service: ["auth", "search", "cart"][i % 3],
            ms: 25.0 + (i * 7 % 31) as f64 * (1 + i % 3) as f64,
        })
        .chain([Request {
            service: "auth",
            ms: 160.0,
        }])
        .collect()
}

pub fn scatter() -> impl Chart {
    // tour:begin scatter
    gup::scatter(students())
        .x(|s| s.hours)
        .y(|s| s.score)
        .color(|s| s.shift)
        .title("Exam score by study time")
        .x_title("Hours studied per week")
        .y_title("Exam score")
    // tour:end scatter
}

pub fn line() -> impl Chart {
    // tour:begin line
    gup::line(temperatures())
        .x(|t| t.month)
        .y(|t| t.celsius)
        .color(|t| t.city)
        .curve(Curve::MonotoneX)
        .title("Average daily maximum")
        .x_title("Month")
        .y_title("Temperature (°C)")
    // tour:end line
}

pub fn bar() -> impl Chart {
    // tour:begin bar
    gup::bar(sales())
        .x(|s| s.quarter)
        .y(|s| s.units)
        .group(|s| s.product)
        .title("Units sold, grouped")
        .y_title("Units (thousands)")
    // tour:end bar
}

pub fn bar_stacked() -> impl Chart {
    // tour:begin bar_stacked
    gup::bar(sales())
        .x(|s| s.quarter)
        .y(|s| s.units)
        .stack(|s| s.product)
        .title("Units sold, stacked")
        .y_title("Units (thousands)")
    // tour:end bar_stacked
}

pub fn barh() -> impl Chart {
    // tour:begin barh
    gup::barh(languages())
        .y(|l| l.name)
        .x(|l| l.share)
        .title("Services by language")
        .x_title("Share of services (%)")
    // tour:end barh
}

pub fn area() -> impl Chart {
    // tour:begin area
    gup::area(visits())
        .x(|v| v.month)
        .y(|v| v.count)
        .stack(|v| v.source)
        .title("Visits by source")
        .x_title("Month")
        .y_title("Visits (thousands)")
    // tour:end area
}

pub fn histogram() -> impl Chart {
    // tour:begin histogram
    gup::histogram(exams())
        .x(|e| e.score)
        .bins(Bins::Width(10.0))
        .title("Exam scores")
        .x_title("Score")
        .y_title("Students")
    // tour:end histogram
}

pub fn heatmap() -> impl Chart {
    // tour:begin heatmap
    gup::heatmap(customers())
        .x(|c| c.hour)
        .y(|c| c.day)
        .color_value(|c| c.visits)
        .legend_title("Visits")
        .title("When customers visit")
        .x_title("Hour of day")
    // tour:end heatmap
}

pub fn boxplot() -> impl Chart {
    // tour:begin boxplot
    gup::boxplot(requests())
        .x(|r| r.service)
        .y(|r| r.ms)
        .title("Response time by service")
        .y_title("Milliseconds")
    // tour:end boxplot
}

fn main() -> gup::Result<()> {
    gup::show(scatter())
}
