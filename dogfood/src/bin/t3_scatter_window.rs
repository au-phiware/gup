// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 3b: the 200k scatter in a window with a hover tooltip showing the
//! datum.
//!
//! DOGFOOD_AUTO=1 resolves the chart headless, takes a row's tooltip anchor,
//! and asks the window to hover there and save a screenshot to
//! /tmp/gup-dogfood/t3_window.png, then exit.

use gup_core::prelude::*;
use gup_core::{RowId, ScatterChart};
use gup_dogfood::{PALETTE_HEX, Pt, load_points};

const W: f32 = 1000.0;
const H: f32 = 700.0;

fn chart(data: Vec<Pt>) -> ScatterChart<Pt> {
    gup_core::scatter(data)
        .x(|p| p.income)
        .x_scale(Log::new())
        .y(|p| p.spend)
        .color(|p| p.segment.as_str())
        .color_scale(Categorical::from_palette(PALETTE_HEX.map(Color::hex)))
        .size(|p| p.weight)
        .opacity(0.35)
        .grid(Axes::Both)
        .title("Spend vs income (200k customers), log x")
        .tooltip(|p| {
            format!(
                "id {}\nincome ${:.0}\nspend {:.2}\nsegment {}\nweight {:.2}",
                p.id, p.income, p.spend, p.segment, p.weight
            )
        })
}

fn main() -> gup_core::Result<()> {
    env_logger::init();
    let mut chart = chart(load_points().expect("points.csv"));
    if std::env::var("DOGFOOD_AUTO").is_ok() {
        let cx = Context::new_blocking()?;
        let plot = chart.resolve(&cx, W, H)?.layout.plot;
        // The first row whose tooltip anchor is well inside the plot.
        let anchor = (0..200_000)
            .filter_map(|row| chart.plot_mut().pick_anchor(RowId { layer: 0, row }))
            .find(|a| {
                let (cx, cy) = (
                    plot.left() + plot.width * 0.5,
                    plot.top() + plot.height * 0.5,
                );
                (a.x - cx).abs() < plot.width * 0.2 && (a.y - cy).abs() < plot.height * 0.2
            })
            .expect("a row near the plot centre");
        println!("hover anchor {anchor:?}");
        // SAFETY: nothing else has started a thread yet and nothing reads
        // the environment concurrently; `show` runs after this.
        unsafe {
            std::env::set_var("GUP_SCREENSHOT_PATH", "/tmp/gup-dogfood/t3_window.png");
            std::env::set_var(
                "GUP_SCREENSHOT_POINTER",
                format!("{},{}", anchor.x, anchor.y),
            );
            std::env::set_var("GUP_SCREENSHOT_WIDTH", "1000");
            std::env::set_var("GUP_SCREENSHOT_HEIGHT", "700");
        }
    }
    gup_core::show(chart)
}
