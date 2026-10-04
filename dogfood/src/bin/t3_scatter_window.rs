// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 3b: the 200k scatter in a window with a hover tooltip showing the
//! datum. GupApp has no input hooks, ComposedChart's "hover reveal" only
//! reveals clipped label text, and gup-egui does not compile, so this uses
//! eframe + our own ChartTexture + hand-written hit testing.
//!
//! DOGFOOD_AUTO=1 injects a pointer move, saves a screenshot to
//! /tmp/gup-dogfood/t3_window.png, then exits.

use eframe::egui;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::chart_builder::{ChartBuilder, ComposedChart, ConfigurableBuilder};
use gup::shader_function::math::{LinearScale, LogScale};
use gup_dogfood::*;
use std::sync::Arc;

const W: f32 = 1000.0;
const H: f32 = 700.0;

struct App {
    chart: ComposedChart<Pt, gup::Circle>,
    tex: ChartTexture,
    data: Vec<Pt>,
    /// Unit-space (0..1) positions recomputed by hand: there is no
    /// screen->datum query for builder charts.
    unit: Vec<(f32, f32)>,
    frac: PlotFrac,
    frame: u32,
    auto: bool,
}

fn build(data: Vec<Pt>, ctx: Arc<gup::RenderContext>) -> (ComposedChart<Pt, gup::Circle>, [f32; 4]) {
    let (xmin, xmax) = data.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.income), b.max(p.income)));
    let (ymin, ymax) = data.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.spend), b.max(p.spend)));
    let mut chart = scatter()
        .x(AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.income)))
        .y(AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.spend)))
        .color(AccessorFunction::new(|p: &Pt| AccessorValue::Color(rgba(seg_index(&p.segment), 0.35))))
        .size(AccessorFunction::new(|p: &Pt| AccessorValue::Float(0.04 * p.weight)))
        .x_scale(LogScale::base10(xmin, xmax, -1.0, 1.0))
        .y_scale(LinearScale::new(ymin, ymax, -1.0, 1.0))
        .width(W)
        .height(H)
        .show_axes(true)
        .build_with_data(data, ctx)
        .expect("chart");
    chart.visualization.attr("stroke_width", |_: &Pt| 0.0f32);
    let c = chart.visualization.context().cloned().unwrap();
    chart.visualization.prepare_render_bound(c.device(), c.queue(), None, None).expect("prepare");
    (chart, [xmin, xmax, ymin, ymax])
}

impl App {
    fn new() -> Self {
        let ctx = Arc::new(pollster::block_on(gup::RenderContext::new()).expect("gpu"));
        let data = load_points().expect("points.csv");
        let (chart, [xmin, xmax, ymin, ymax]) = build(data.clone(), ctx);
        let frac = plot_frac(&chart);
        let (lx0, lx1) = (xmin.log10(), xmax.log10());
        let unit = data
            .iter()
            .map(|p| ((p.income.log10() - lx0) / (lx1 - lx0), (p.spend - ymin) / (ymax - ymin)))
            .collect();
        Self { chart, tex: ChartTexture::default(), data, unit, frac, frame: 0, auto: std::env::var("DOGFOOD_AUTO").is_ok() }
    }

    fn nearest(&self, ux: f32, uy: f32, max_d: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = max_d * max_d;
        for (i, &(x, y)) in self.unit.iter().enumerate() {
            let d = (x - ux).powi(2) + (y - uy).powi(2);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.auto && self.frame >= 3 {
            raw.events.push(egui::Event::PointerMoved(egui::pos2(520.0, 380.0)));
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            // Title drawn by egui: gup's texture path renders no text.
            ui.heading("Spend vs income (200k customers), log x");
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::hover());
            let _ = &resp;
            self.tex.show(ui, rect, &mut self.chart);
            if let Some(pos) = resp.hover_pos() {
                let local = pos - rect.min;
                if let Some((ux, uy)) = self.frac.unit(local.x, local.y, W, H) {
                    let t0 = std::time::Instant::now();
                    if let Some(i) = self.nearest(ux, uy, 0.01) {
                        let p = &self.data[i];
                        let took = t0.elapsed();
                        let text = format!(
                            "id {}\nincome ${:.0}\nspend {:.2}\nsegment {}\nweight {:.2}\n(lookup {:?})",
                            p.id, p.income, p.spend, p.segment, p.weight, took
                        );
                        egui::Area::new(egui::Id::new("tip"))
                            .fixed_pos(pos + egui::vec2(14.0, 14.0))
                            .order(egui::Order::Tooltip)
                            .show(ctx, |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(text));
                            });
                        // Highlight ring around the hovered datum (egui, not gup).
                        let (x, y) = self.unit[i];
                        let f = self.frac;
                        let c = rect.min
                            + egui::vec2(
                                (f.left + x * (f.right - f.left)) * W,
                                (f.bottom - y * (f.bottom - f.top)) * H,
                            );
                        ui.painter().circle_stroke(c, 6.0, egui::Stroke::new(2.0, egui::Color32::BLACK));
                    }
                }
            }
        });
        if self.auto {
            if self.frame == 10 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if save_screenshot(ctx, "/tmp/gup-dogfood/t3_window.png") {
                println!("chart render+upload {:?}", self.tex.last_render);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([W, H + 40.0]),
        ..Default::default()
    };
    eframe::run_native("gup dogfood t3", options, Box::new(|_cc| Ok(Box::new(App::new()))))
}
