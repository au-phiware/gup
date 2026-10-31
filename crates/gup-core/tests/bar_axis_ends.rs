// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-561: a bar's value axis ends on a labelled tick, and band axes end
//! with their last band.

mod common;

use gup_core::prelude::*;
use gup_core::transform::bin;
use gup_core::{Bins, Layout};

/// A seeded xorshift in 0..1.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

const CATS: [&str; 5] = ["A", "B", "C", "D", "E"];
const KEYS: [&str; 3] = ["x", "y", "z"];

type Row = (&'static str, &'static str, f64);

/// Rows for every category and key; the largest simple bar is `top`.
fn rows(rng: &mut Rng, top: f64, negative: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, c) in CATS.iter().enumerate() {
        for k in KEYS {
            let mut v = (rng.next() * top).max(0.01 * top);
            if negative && i == 2 && k == "y" {
                v = -v * 0.6;
            }
            rows.push((*c, k, v));
        }
    }
    rows
}

/// The extreme bar ends of `rows`: per category, the largest value (grouped
/// and simple) or the positive and negative sums (stacked).
fn extremes(rows: &[Row], stacked: bool) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for c in CATS {
        let vs: Vec<f64> = rows.iter().filter(|r| r.0 == c).map(|r| r.2).collect();
        if stacked {
            hi = hi.max(vs.iter().filter(|v| **v > 0.0).sum());
            lo = lo.min(vs.iter().filter(|v| **v < 0.0).sum());
        } else {
            for v in vs {
                hi = hi.max(v);
                lo = lo.min(v);
            }
        }
    }
    (lo, hi)
}

fn check(name: &str, layout: &Layout, vertical: bool, (lo, hi): (f64, f64)) {
    let ticks = if vertical {
        &layout.y_ticks
    } else {
        &layout.x_ticks
    };
    let first = ticks.first().expect("ticks").0;
    let last = ticks.last().expect("ticks").0;
    let eps = 1e-9 * (hi - lo).abs();
    // Strictly inside, so no bar touches the plot edge.
    assert!(
        hi <= 0.0 || hi < last - eps,
        "{name}: bar end {hi} not below last tick {last}: {ticks:?}"
    );
    assert!(
        lo >= 0.0 || lo > first + eps,
        "{name}: bar end {lo} not above first tick {first}"
    );
    assert!(
        ticks.iter().any(|t| t.0.abs() <= eps),
        "{name}: 0 is not a tick of {ticks:?}"
    );
}

#[test]
fn bars_end_inside_the_last_tick() {
    let cx = Context::new_blocking().unwrap();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut cases = 0;
    for w in (240..=800).step_by(80) {
        for h in (160..=560).step_by(80) {
            for labels in [false, true] {
                let top = 1.0 + rng.next() * 999.0;
                let plain = rows(&mut rng, top, false);
                let signed = rows(&mut rng, top, true);
                let size = format!("{w}x{h} labels={labels} top={top:.2}");

                macro_rules! run {
                    ($name:literal, $chart:expr, $vertical:expr, $data:expr, $stacked:expr) => {{
                        let mut chart = $chart;
                        let resolved = chart.resolve(&cx, w as f32, h as f32).unwrap();
                        check(
                            &format!("{} {size}", $name),
                            &resolved.layout,
                            $vertical,
                            extremes($data, $stacked),
                        );
                        cases += 1;
                    }};
                }
                macro_rules! labelled {
                    ($chart:expr) => {{
                        let c = $chart;
                        if labels { c.value_labels() } else { c }
                    }};
                }

                // Simple: one value per category.
                let simple: Vec<Row> = plain.iter().filter(|r| r.1 == "x").copied().collect();
                run!(
                    "bar",
                    labelled!(gup_core::bar(simple.clone()).x(|r| r.0).y(|r| r.2)),
                    true,
                    &simple,
                    false
                );
                run!(
                    "barh",
                    labelled!(gup_core::barh(simple.clone()).y(|r| r.0).x(|r| r.2)),
                    false,
                    &simple,
                    false
                );
                run!(
                    "grouped",
                    labelled!(
                        gup_core::bar(signed.clone())
                            .x(|r| r.0)
                            .y(|r| r.2)
                            .group(|r| r.1)
                    ),
                    true,
                    &signed,
                    false
                );
                run!(
                    "stacked",
                    labelled!(
                        gup_core::bar(plain.clone())
                            .x(|r| r.0)
                            .y(|r| r.2)
                            .stack(|r| r.1)
                    ),
                    true,
                    &plain,
                    true
                );
                run!(
                    "barh stacked",
                    labelled!(
                        gup_core::barh(plain.clone())
                            .y(|r| r.0)
                            .x(|r| r.2)
                            .stack(|r| r.1)
                    ),
                    false,
                    &plain,
                    true
                );

                // A histogram: the tallest bin.
                let values: Vec<f64> = (0..200 + (top as usize % 400))
                    .map(|_| 50.0 + 15.0 * (rng.next() + rng.next() + rng.next() - 1.5))
                    .collect();
                let tallest = bin(values.clone(), &Bins::Auto)
                    .iter()
                    .map(|b| b.count)
                    .max()
                    .unwrap() as f64;
                let mut chart = gup_core::histogram(values).x(|v| *v);
                let resolved = chart.resolve(&cx, w as f32, h as f32).unwrap();
                check(
                    &format!("histogram {size}"),
                    &resolved.layout,
                    true,
                    (0.0, tallest),
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 8 * 6 * 2 * 6, "cases");
}
