// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cost of category keys (GUP-566): 1,000,000 rows `{ x, y, key }` with
//! 8 distinct `String` keys in a seeded pseudo-random order.
//!
//! ```text
//! cargo run -p gup-core --release --example keyed_resolve_bench
//! ```
//!
//! Prints `metric keyed.<mode>.first_resolve_ms <median of 5>` for a fresh
//! chart's first `resolve`, where `<mode>` is `borrowed` (`encode_key` of a
//! `&str`), `owned_plot` (`encode_owned_key` of a clone) or `builder`
//! (`scatter(..).color(|r| r.key.clone())`); and
//! `metric keyed.dict.<path>.ns_per_row` for 1,000,000 key encodes without
//! the GPU. The `builder` dictionary path models the builder's per-row work
//! through public API, as it is at the commit measured.

use gup_core::column::Dictionary;
use gup_core::prelude::*;
use std::sync::Arc;
use std::time::Instant;

const ROWS: usize = 1_000_000;
const RUNS: usize = 5;
const KEYS: [&str; 8] = [
    "north", "south", "east", "west", "up", "down", "left", "right",
];

#[derive(Clone)]
struct Row {
    x: f64,
    y: f64,
    key: String,
}

fn rows() -> Vec<Row> {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    (0..ROWS)
        .map(|_| Row {
            x: (next() >> 11) as f64 / (1u64 << 53) as f64,
            y: (next() >> 11) as f64 / (1u64 << 53) as f64,
            key: KEYS[(next() >> 33) as usize % KEYS.len()].to_string(),
        })
        .collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn plot(rows: Vec<Row>, owned: bool) -> Plot {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    let layer = plot.add(Selection::<Row, Circle>::new(rows));
    layer
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y));
    if owned {
        layer.attr(
            Circle::FILL,
            colour.encode_owned_key(|r: &Row| r.key.clone()),
        );
    } else {
        layer.attr(Circle::FILL, colour.encode_key(|r: &Row| r.key.as_str()));
    }
    plot
}

fn time_resolve(cx: &Context, rows: &[Row], mode: &str) -> f64 {
    let mut samples = Vec::new();
    for _ in 0..RUNS {
        let rows = rows.to_vec();
        let mut chart: Box<dyn Chart> = match mode {
            "borrowed" => Box::new(plot(rows, false)),
            "owned_plot" => Box::new(plot(rows, true)),
            _ => Box::new(
                gup_core::scatter(rows)
                    .x(|r: &Row| r.x)
                    .y(|r: &Row| r.y)
                    .color(|r: &Row| r.key.clone()),
            ),
        };
        let start = Instant::now();
        chart.resolve(cx, 640.0, 400.0).expect("resolve");
        samples.push(start.elapsed().as_secs_f64() * 1e3);
    }
    median(samples)
}

fn ns_per_row(rows: &[Row], mut f: impl FnMut(&Row)) -> f64 {
    let start = Instant::now();
    for r in rows {
        f(r);
    }
    start.elapsed().as_secs_f64() * 1e9 / rows.len() as f64
}

fn main() -> gup_core::Result<()> {
    let rows = rows();
    let mut d = Dictionary::<Arc<str>>::new();
    let borrowed = median(
        (0..RUNS)
            .map(|_| ns_per_row(&rows, |r| drop(d.encode_borrowed(r.key.as_str()))))
            .collect(),
    );
    let mut d = Dictionary::<String>::new();
    let owned = median(
        (0..RUNS)
            .map(|_| ns_per_row(&rows, |r| drop(d.encode(r.key.clone()))))
            .collect(),
    );
    let mut d = Dictionary::<Arc<str>>::new();
    // The builder's per-row work: the user's clone, its label borrowed by
    // the dictionary (copied only when new).
    let builder = median(
        (0..RUNS)
            .map(|_| {
                ns_per_row(&rows, |r| {
                    drop({
                        let k = r.key.clone();
                        d.encode_borrowed(k.as_str())
                    })
                })
            })
            .collect(),
    );
    println!("metric keyed.dict.borrowed.ns_per_row {borrowed:.1}");
    println!("metric keyed.dict.owned.ns_per_row {owned:.1}");
    println!("metric keyed.dict.builder.ns_per_row {builder:.1}");

    let cx = Context::new_blocking()?;
    for mode in ["borrowed", "owned_plot", "builder"] {
        let ms = time_resolve(&cx, &rows, mode);
        println!("metric keyed.{mode}.first_resolve_ms {ms:.1}");
    }
    Ok(())
}
