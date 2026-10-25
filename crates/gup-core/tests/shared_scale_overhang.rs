// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-529: marks on automatic scales are drawn whole, whatever kind of
//! layer shares the scales and in whatever order they were added.

use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scatter;

#[derive(Clone)]
struct Month {
    n: f64,
    mean: f64,
}

fn months() -> Vec<Month> {
    [
        22.0, 22.0, 21.0, 18.0, 15.0, 13.0, 12.0, 13.0, 16.0, 18.0, 20.0, 21.0,
    ]
    .iter()
    .enumerate()
    .map(|(i, &mean)| Month {
        n: i as f64 + 1.0,
        mean,
    })
    .collect()
}

/// Assert the pixel 5 px from `(x, y)` in each direction is inside `plot`.
fn assert_whole(plot: Rect, what: &str, x: f32, y: f32) {
    for (px, py) in [(x, y - 5.0), (x, y + 5.0), (x - 5.0, y), (x + 5.0, y)] {
        assert!(
            px >= plot.left() && px <= plot.right() && py >= plot.top() && py <= plot.bottom(),
            "{what}: disc at ({x}, {y}) reaches ({px}, {py}) outside plot {plot:?}"
        );
    }
}

fn layered(circles_first: bool) {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.title("Monthly temperature")
        .x_title("Month")
        .y_title("Mean temperature (°C)");
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let heat = Sequential::viridis();
    let circles = |plot: &mut Plot| {
        plot.add(Selection::<Month, Circle>::new(months()))
            .attr(Circle::X, x.encode(|m: &Month| m.n))
            .attr(Circle::Y, y.encode(|m: &Month| m.mean))
            .attr(Circle::FILL, heat.encode(|m: &Month| m.mean))
            .attr(Circle::RADIUS, Px(6.0))
            .legend_title(Circle::FILL, "Mean (°C)");
    };
    let line = |plot: &mut Plot| {
        plot.line(months())
            .x(|m| m.n)
            .y(|m| m.mean)
            .color(Color::hex(0xb0b0b0));
    };
    if circles_first {
        circles(&mut plot);
        line(&mut plot);
    } else {
        line(&mut plot);
        circles(&mut plot);
    }
    let (_, layout) = plot.render_resolved(&cx, 640, 400).unwrap();
    for m in months() {
        let (px, py) = (x.read().eval(m.n) as f32, y.read().eval(m.mean) as f32);
        assert_whole(layout.plot, "layered", px, py);
    }
}

#[test]
fn line_then_circles_on_shared_scales_are_whole() {
    layered(false);
}

#[test]
fn circles_then_line_on_shared_scales_are_whole() {
    layered(true);
}

#[test]
fn bird_scatter_discs_are_whole() {
    let birds = [
        (0.03, 0.20),
        (0.11, 0.35),
        (0.17, 0.55),
        (0.50, 0.75),
        (0.90, 0.95),
        (1.00, 1.30),
        (1.50, 0.55),
        (2.10, 0.30),
        (2.70, 0.40),
    ];
    let mut chart = scatter(birds)
        .x(|b| b.0)
        .y(|b| b.1)
        .radius(6.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)");
    let cx = Context::new_blocking().unwrap();
    let layout = chart.resolve(&cx, 640.0, 400.0).unwrap().layout.clone();
    let along = |ticks: &[(f64, f32)], v: f64| {
        let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
        a.1 + ((v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)) as f32
    };
    for (bx, by) in birds {
        let (px, py) = (along(&layout.x_ticks, bx), along(&layout.y_ticks, by));
        assert_whole(layout.plot, "birds", px, py);
    }
}
