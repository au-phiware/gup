// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 5: append points every 100 ms to a line chart and a 100k scatter.
//!
//! Each chart is built once; every tick feeds the new samples through its
//! `appender()`, and the next frame uploads only those rows (GUP-480). Top:
//! a random-walk line. Bottom: a 100k-point scatter that gets 50 new points
//! per tick. Domains are fixed so the view does not rescale.
//!
//! DOGFOOD_AUTO=1 runs ~6 s, prints timings, saves a screenshot, exits.

use eframe::egui;
use gup_core::prelude::*;
use gup_core::{Appender, LineChart, ScatterChart};
use gup_dogfood::*;
use std::time::{Duration, Instant};

const W: f32 = 900.0;
const H: f32 = 300.0;
const X_MAX: f64 = 600.0; // 60 s at 10 Hz; fixed domain (no rescale)

#[derive(Clone, Copy)]
struct S {
    t: f64,
    v: f64,
}

fn line_chart(pts: &[S]) -> LineChart<S> {
    gup_core::line(pts.to_vec())
        .x(|s| s.t)
        .x_scale(Linear::new().domain(0.0, X_MAX))
        .y(|s| s.v)
        .y_scale(Linear::new().domain(-10.0, 10.0))
        .stroke(Color::hex(0xd62728))
        .width(2.0)
}

fn scatter_chart(pts: &[S]) -> ScatterChart<S> {
    gup_core::scatter(pts.to_vec())
        .x(|s| s.t)
        .x_scale(Linear::new().domain(0.0, X_MAX))
        .y(|s| s.v)
        .y_scale(Linear::new().domain(-10.0, 10.0))
        .fill(Color::hex(0x1f77b4))
        .opacity(0.2)
        .radius(1.5)
}

struct App {
    line: LineChart<S>,
    line_w: ChartTexture,
    line_feed: Appender<S>,
    /// The last line sample and how many there are.
    last_pt: S,
    n_pts: usize,
    big: ScatterChart<S>,
    big_w: ChartTexture,
    big_feed: Appender<S>,
    n_big: usize,
    last: Instant,
    started: Instant,
    rng: u64,
    timings: Vec<(Duration, Duration)>,
    auto: bool,
    frame: u32,
}

impl App {
    fn rand(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.rng >> 40) as f64 / (1u64 << 24) as f64) - 0.5
    }

    fn new() -> Self {
        let pts: Vec<S> = (0..20)
            .map(|i| S {
                t: i as f64,
                v: (i as f64 * 0.7).sin() * 3.0,
            })
            .collect();
        let mut big_data = Vec::with_capacity(200_000);
        let mut r = 7u64;
        for _ in 0..100_000 {
            r = r.wrapping_mul(6364136223846793005).wrapping_add(1);
            let a = (r >> 40) as f64 / (1u64 << 24) as f64;
            r = r.wrapping_mul(6364136223846793005).wrapping_add(1);
            let b = (r >> 40) as f64 / (1u64 << 24) as f64;
            big_data.push(S {
                t: a * X_MAX,
                v: (b - 0.5) * 20.0,
            });
        }
        let (n_pts, n_big) = (pts.len(), big_data.len());
        let last_pt = *pts.last().unwrap();
        let mut line = line_chart(&pts);
        let mut big = scatter_chart(&big_data);
        Self {
            line_feed: line.appender(),
            line,
            line_w: ChartTexture::new(),
            last_pt,
            n_pts,
            big_feed: big.appender(),
            big,
            big_w: ChartTexture::new(),
            n_big,
            last: Instant::now(),
            started: Instant::now(),
            rng: 99,
            timings: vec![],
            auto: std::env::var("DOGFOOD_AUTO").is_ok(),
            frame: 0,
        }
    }

    fn tick(&mut self) {
        // Line: append one point.
        let prev = self.last_pt;
        let next = S {
            t: prev.t + 1.0,
            v: (prev.v + 2.0 * self.rand()).clamp(-10.0, 10.0),
        };
        self.last_pt = next;
        self.n_pts += 1;
        let t0 = Instant::now();
        self.line_feed.push(next).expect("the chart is alive");
        let line_cost = t0.elapsed();
        self.line_w.dirty = true;

        // Scatter: append 50 points.
        let batch: Vec<S> = (0..50)
            .map(|_| {
                let (a, b) = (self.rand() + 0.5, self.rand());
                S {
                    t: a * X_MAX,
                    v: b * 20.0,
                }
            })
            .collect();
        self.n_big += batch.len();
        let t1 = Instant::now();
        self.big_feed.extend(batch).expect("the chart is alive");
        self.timings.push((line_cost, t1.elapsed()));
        self.big_w.dirty = true;
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        if self.last.elapsed() >= Duration::from_millis(100) {
            self.last = Instant::now();
            self.tick();
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let (lw, bw) = self.timings.last().copied().unwrap_or_default();
            ui.label(format!(
                "line: {} pts, update {:?} | scatter: {} pts, update {:?}",
                self.n_pts, lw, self.n_big, bw
            ));
            let (r1, _) = ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::hover());
            self.line_w.show(ui, r1, &mut self.line);
            let (r2, _) = ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::hover());
            self.big_w.show(ui, r2, &mut self.big);
        });
        if self.auto {
            if self.started.elapsed() > Duration::from_secs(6) && self.frame < 1_000_000 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.frame = 1_000_000; // only once
            }
            if save_screenshot(ctx, "/tmp/gup-dogfood/t5_live.png") {
                let n = self.timings.len() as u32;
                let (sl, sb) = self
                    .timings
                    .iter()
                    .fold((Duration::ZERO, Duration::ZERO), |a, t| {
                        (a.0 + t.0, a.1 + t.1)
                    });
                println!(
                    "ticks={n} avg line update={:?} avg 100k-scatter update={:?}; last render+upload line={:?} scatter={:?}",
                    sl / n,
                    sb / n,
                    self.line_w.last_render,
                    self.big_w.last_render
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([W + 20.0, 2.0 * H + 60.0]),
        ..Default::default()
    };
    eframe::run_native(
        "gup dogfood t5",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
