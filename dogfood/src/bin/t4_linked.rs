// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 4: two linked views in egui. Brushing in the left scatter
//! (income vs spend) highlights the same customers in the right scatter
//! (weight vs spend). Uses gup's SharedSelectionState for the link.
//!
//! DOGFOOD_AUTO=1 performs a scripted brush drag and saves a screenshot.

use eframe::egui;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::chart_builder::{ChartBuilder, ComposedChart, ConfigurableBuilder};
use gup::linked_selection::SharedSelectionState;
use gup::shader_function::math::{LinearScale, LogScale};
use gup_dogfood::*;
use std::sync::Arc;
use std::time::Instant;

const N: usize = 30_000;
const W: f32 = 560.0;
const H: f32 = 480.0;

type Chart = ComposedChart<Pt, gup::Circle>;

struct View {
    tex: ChartTexture,
    chart: Chart,
    frac: PlotFrac,
    unit: Vec<(f32, f32)>,
}

struct App {
    left: View,
    right: View,
    shared: SharedSelectionState<u32>,
    brush: Option<(egui::Pos2, egui::Pos2)>,
    last_gen: u64,
    frame: u32,
    auto: bool,
    status: String,
}

fn ext(it: impl Iterator<Item = f32>) -> (f32, f32) {
    it.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)))
}

fn make_view(
    data: &[Pt],
    ctx: Arc<gup::RenderContext>,
    shared: SharedSelectionState<u32>,
    log_x: bool,
    fx: fn(&Pt) -> f32,
) -> View {
    let (x0, x1) = ext(data.iter().map(fx));
    let (y0, y1) = ext(data.iter().map(|p| p.spend));
    let b = scatter()
        .x(AccessorFunction::new(move |p: &Pt| AccessorValue::Float(fx(p))))
        .y(AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.spend)))
        .size(AccessorFunction::new(|p: &Pt| AccessorValue::Float(0.06 * p.weight)));
    let b = if log_x { b.x_scale(LogScale::base10(x0, x1, -1.0, 1.0)) } else { b.x_scale(LinearScale::new(x0, x1, -1.0, 1.0)) };
    let mut chart = b
        .y_scale(LinearScale::new(y0, y1, -1.0, 1.0))
        .width(W)
        .height(H)
        .show_axes(true)
        .build_with_data(data.to_vec(), ctx)
        .expect("chart");
    // Highlight = re-bind colour against the shared selection state. The
    // closure is re-evaluated on every prepare_render_bound.
    let any = shared.clone();
    chart.visualization.attr("color", move |p: &Pt| {
        if any.is_empty() || any.is_selected(&p.id) {
            rgba(seg_index(&p.segment), 0.6)
        } else {
            [0.75, 0.75, 0.75, 0.15]
        }
    });
    chart.visualization.attr("stroke_width", |_: &Pt| 0.0f32);
    prepare(&mut chart);
    let frac = plot_frac(&chart);
    let unit = data
        .iter()
        .map(|p| {
            let ux = if log_x {
                (fx(p).log10() - x0.log10()) / (x1.log10() - x0.log10())
            } else {
                (fx(p) - x0) / (x1 - x0)
            };
            (ux, (p.spend - y0) / (y1 - y0))
        })
        .collect();
    View { tex: ChartTexture::default(), chart, frac, unit }
}

fn prepare(chart: &mut Chart) {
    let c = chart.visualization.context().cloned().unwrap();
    chart.visualization.prepare_render_bound(c.device(), c.queue(), None, None).expect("prepare");
}

impl App {
    fn new() -> Self {
        let gctx = Arc::new(pollster::block_on(gup::RenderContext::new()).expect("gpu"));
        let mut data = load_points().expect("points.csv");
        data.truncate(N);
        let shared = SharedSelectionState::<u32>::new();
        let left = make_view(&data, gctx.clone(), shared.clone(), true, |p| p.income);
        let right = make_view(&data, gctx.clone(), shared.clone(), false, |p| p.weight);
        Self {
            left,
            right,
            shared,
            brush: None,
            last_gen: 0,
            frame: 0,
            auto: std::env::var("DOGFOOD_AUTO").is_ok(),
            status: String::new(),
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if !self.auto {
            return;
        }
        let ev = |p: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let (a, b) = (egui::pos2(260.0, 120.0), egui::pos2(420.0, 330.0));
        match self.frame {
            3 => raw.events.push(egui::Event::PointerMoved(a)),
            4 => raw.events.push(ev(a, true)),
            5..=12 => {
                let t = (self.frame - 4) as f32 / 8.0;
                raw.events.push(egui::Event::PointerMoved(a + (b - a) * t));
            }
            13 => raw.events.push(ev(b, false)),
            _ => {}
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| ui.label(&self.status));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left view with brush.
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::drag());
                self.left.tex.show(ui, rect, &mut self.left.chart);
                if resp.drag_started() {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.brush = Some((p, p));
                    }
                }
                if resp.dragged() {
                    if let (Some((a, _)), Some(p)) = (self.brush, resp.interact_pointer_pos()) {
                        self.brush = Some((a, p));
                        let r = egui::Rect::from_two_pos(a, p);
                        let (ux0, uy1) = self.left.frac.unit_unclamped(r.min.x - rect.min.x, r.min.y - rect.min.y, W, H);
                        let (ux1, uy0) = self.left.frac.unit_unclamped(r.max.x - rect.min.x, r.max.y - rect.min.y, W, H);
                        let t0 = Instant::now();
                        let ids: Vec<u32> = self
                            .left
                            .unit
                            .iter()
                            .enumerate()
                            .filter(|(_, (x, y))| (ux0..=ux1).contains(x) && (uy0..=uy1).contains(y))
                            .map(|(i, _)| i as u32)
                            .collect();
                        let n = ids.len();
                        self.shared.set(ids);
                        self.status = format!("brushed {n} of {N} (query {:?})", t0.elapsed());
                    }
                }
                if let Some((a, b)) = self.brush {
                    ui.painter().rect_stroke(
                        egui::Rect::from_two_pos(a, b),
                        0.0,
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(40, 40, 40)),
                        egui::StrokeKind::Middle,
                    );
                }
                let r2 = egui::Rect::from_min_size(rect.right_top() + egui::vec2(10.0, 0.0), egui::vec2(W, H));
                self.right.tex.show(ui, r2, &mut self.right.chart);
            });
        });
        // Propagate selection changes: re-evaluate both charts' bindings.
        let g = self.shared.generation();
        if g != self.last_gen {
            self.last_gen = g;
            let t0 = Instant::now();
            prepare(&mut self.left.chart);
            prepare(&mut self.right.chart);
            self.left.tex.dirty = true;
            self.right.tex.dirty = true;
            self.status.push_str(&format!(" | re-prepare {:?}", t0.elapsed()));
        }
        if self.auto {
            if self.frame == 20 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if save_screenshot(ctx, "/tmp/gup-dogfood/t4_linked.png") {
                println!("{}", self.status);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([2.0 * W + 40.0, H + 60.0]),
        ..Default::default()
    };
    eframe::run_native("gup dogfood t4", options, Box::new(|_cc| Ok(Box::new(App::new()))))
}
