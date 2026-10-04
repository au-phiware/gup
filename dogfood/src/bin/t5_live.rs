// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 5: append points every 100 ms without rebuilding the chart.
//!
//! Top: a line chart (random walk) whose LineSegment data is extended in
//! place via `visualization.set_data` + `prepare_render_bound`.
//! Bottom: a 100k-point scatter that gets 50 new points per tick, to measure
//! the per-append cost (there is no append API; every tick re-sends all).
//!
//! DOGFOOD_AUTO=1 runs ~3 s, prints timings, saves a screenshot, exits.

use eframe::egui;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::line::LineSegment;
use gup::chart_builder::builders::{AccessorFunction, line, scatter};
use gup::chart_builder::{ChartBuilder, ComposedChart, ConfigurableBuilder};
use gup::shader_function::math::LinearScale;
use gup_dogfood::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

const W: f32 = 900.0;
const H: f32 = 300.0;
const X_MAX: f32 = 600.0; // 60 s at 10 Hz; fixed domain (no rebuild => no rescale)

#[derive(Debug, Clone)]
struct S {
    t: f32,
    v: f32,
}

struct App {
    line: ComposedChart<LineSegment<S>, gup::Line>,
    line_w: ChartTexture,
    pts: Vec<S>,
    big: ComposedChart<S, gup::Circle>,
    big_w: ChartTexture,
    big_data: Vec<S>,
    last: Instant,
    started: Instant,
    rng: u64,
    timings: Vec<(Duration, Duration)>,
    auto: bool,
    frame: u32,
}

impl App {
    fn rand(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.rng >> 40) as f32 / (1u64 << 24) as f32) - 0.5
    }

    fn new() -> Self {
        let g = Arc::new(pollster::block_on(gup::RenderContext::new()).expect("gpu"));
        let pts: Vec<S> = (0..20)
            .map(|i| S {
                t: i as f32,
                v: if std::env::var("FLAT").is_ok() {
                    0.0
                } else {
                    (i as f32 * 0.7).sin() * 3.0
                },
            })
            .collect();
        let line_chart = line()
            .x(AccessorFunction::new(|s: &S| AccessorValue::Float(s.t)))
            .y(AccessorFunction::new(|s: &S| AccessorValue::Float(s.v)))
            .x_scale(LinearScale::new(0.0, X_MAX, -1.0, 1.0))
            .y_scale(LinearScale::new(-10.0, 10.0, -1.0, 1.0))
            .stroke_color([0.84, 0.15, 0.16, 1.0])
            .stroke_width_px(2.0)
            .width(W)
            .height(H)
            .show_axes(true)
            .build_with_data(pts.clone(), g.clone())
            .expect("line");
        if std::env::var("DUMP").is_ok() {
            for s in line_chart.visualization.data().iter().take(3) {
                println!(
                    "builder seg start={:?} end={:?} width={}",
                    s.start_pos, s.end_pos, s.width
                );
            }
        }
        let line = line_chart;
        let line_w = ChartTexture::default();

        let mut big_data = Vec::with_capacity(200_000);
        let mut r = 7u64;
        for _ in 0..100_000 {
            r = r.wrapping_mul(6364136223846793005).wrapping_add(1);
            let a = (r >> 40) as f32 / (1u64 << 24) as f32;
            r = r.wrapping_mul(6364136223846793005).wrapping_add(1);
            let b = (r >> 40) as f32 / (1u64 << 24) as f32;
            big_data.push(S {
                t: a * X_MAX,
                v: (b - 0.5) * 20.0,
            });
        }
        let mut big_chart = scatter()
            .x(AccessorFunction::new(|s: &S| AccessorValue::Float(s.t)))
            .y(AccessorFunction::new(|s: &S| AccessorValue::Float(s.v)))
            .x_scale(LinearScale::new(0.0, X_MAX, -1.0, 1.0))
            .y_scale(LinearScale::new(-10.0, 10.0, -1.0, 1.0))
            .color(AccessorFunction::new(|_: &S| {
                AccessorValue::Color([0.12, 0.47, 0.71, 0.2])
            }))
            .size(AccessorFunction::new(|_: &S| AccessorValue::Float(0.15)))
            .width(W)
            .height(H)
            .show_axes(true)
            .build_with_data(big_data.clone(), g)
            .expect("scatter");
        big_chart.visualization.attr("stroke_width", |_: &S| 0.0f32);
        let big = big_chart;
        let big_w = ChartTexture::default();
        Self {
            line,
            line_w,
            pts,
            big,
            big_w,
            big_data,
            last: Instant::now(),
            started: Instant::now(),
            rng: 99,
            timings: vec![],
            auto: std::env::var("DOGFOOD_AUTO").is_ok(),
            frame: 0,
        }
    }

    fn tick(&mut self) {
        // --- line: append one point -> one new LineSegment ---------------
        let prev = self.pts.last().unwrap().clone();
        let next = S {
            t: prev.t + 1.0,
            v: (prev.v + 2.0 * self.rand()).clamp(-10.0, 10.0),
        };
        self.pts.push(next.clone());
        let t0 = Instant::now();
        {
            let c = &mut self.line;
            let mut segs = c.visualization.data().to_vec(); // no append API: copy all
            let template = segs.last().cloned().unwrap();
            segs.push(LineSegment {
                data: prev.clone(),
                start_pos: [prev.t, prev.v],
                end_pos: [next.t, next.v],
                ..template
            });
            c.visualization.set_data(segs);
            let ctx = c.visualization.context().cloned().unwrap();
            c.visualization
                .prepare_render_bound(ctx.device(), ctx.queue(), None, None)
                .unwrap();
        }
        let line_cost = t0.elapsed();
        self.line_w.dirty = true;

        // --- big scatter: append 50 points ----------------------------------
        for _ in 0..50 {
            let (a, b) = (self.rand() + 0.5, self.rand());
            self.big_data.push(S {
                t: a * X_MAX,
                v: b * 20.0,
            });
        }
        let t1 = Instant::now();
        {
            let c = &mut self.big;
            c.visualization.set_data(self.big_data.clone());
            let ctx = c.visualization.context().cloned().unwrap();
            c.visualization
                .prepare_render_bound(ctx.device(), ctx.queue(), None, None)
                .unwrap();
        }
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
                self.pts.len(),
                lw,
                self.big_data.len(),
                bw
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
                    "ticks={n} avg line update={:?} avg 100k-scatter update={:?}; last render_to_rgba+upload line={:?} scatter={:?}",
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
