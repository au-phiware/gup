// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 4: two linked views in egui. Brushing in the left scatter
//! (income vs spend) highlights the same customers in the right scatter
//! (weight vs spend).
//!
//! DOGFOOD_AUTO=1 performs a scripted brush drag and saves a screenshot.

use eframe::egui;
use gup_core::ScatterChart;
use gup_core::prelude::*;
use gup_dogfood::*;
use gup_egui::GupWidget;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const N: usize = 30_000;
const W: f32 = 560.0;
const H: f32 = 480.0;
const GREY: u32 = 0xbfbfbf;

/// The brushed row indices and a counter that changes with them.
#[derive(Default)]
struct Shared {
    rows: Mutex<Vec<usize>>,
    generation: AtomicU64,
}

/// A customer and whether it is highlighted in the right view.
#[derive(Clone)]
struct R {
    pt: Pt,
    hot: bool,
}

fn palette() -> impl Iterator<Item = Color> {
    PALETTE_HEX.into_iter().map(Color::hex)
}

fn left_chart(data: &[Pt], shared: Arc<Shared>) -> ScatterChart<Pt> {
    let mut chart = gup_core::scatter(data.to_vec())
        .x(|p| p.income)
        .x_scale(Log::new())
        .y(|p| p.spend)
        .color(|p| p.segment.as_str())
        .color_scale(Categorical::from_palette(palette()))
        .size(|p| p.weight)
        .opacity(0.6)
        .theme(Theme {
            unbrushed: Unbrushed::Grey(Color::hex(GREY)),
            ..Theme::default()
        });
    let plot = chart.plot_mut();
    plot.drag(Drag::Brush);
    plot.on_brush(move |b| {
        let rows = b
            .map(|b| b.rows.iter().map(|r| r.row).collect())
            .unwrap_or_default();
        *shared.rows.lock().unwrap() = rows;
        shared.generation.fetch_add(1, Ordering::SeqCst);
    });
    chart
}

/// Rebuilt from the rows whenever the selection changes: unselected
/// customers first (so the grey key takes the palette's first colour), then
/// the selected ones in colour. With no selection everything is in colour.
fn right_chart(data: &[Pt], selected: &[usize]) -> ScatterChart<R> {
    let mut hot = vec![selected.is_empty(); data.len()];
    for &i in selected {
        hot[i] = true;
    }
    let mut rows: Vec<R> = data
        .iter()
        .zip(hot)
        .map(|(pt, hot)| R {
            pt: pt.clone(),
            hot,
        })
        .collect();
    rows.sort_by_key(|r| r.hot);
    let colors: Vec<Color> = if selected.is_empty() {
        palette().collect()
    } else {
        std::iter::once(Color::hex(GREY)).chain(palette()).collect()
    };
    gup_core::scatter(rows)
        .x(|r| r.pt.weight)
        .y(|r| r.pt.spend)
        .color(|r| {
            if r.hot {
                r.pt.segment.as_str()
            } else {
                "unselected"
            }
        })
        .color_scale(Categorical::from_palette(colors))
        .size(|r| r.pt.weight)
        .opacity(0.6)
}

struct App {
    widget_left: GupWidget,
    widget_right: GupWidget,
    data: Vec<Pt>,
    left: ScatterChart<Pt>,
    right: ScatterChart<R>,
    shared: Arc<Shared>,
    last_gen: u64,
    frame: u32,
    auto: bool,
    status: String,
}

impl App {
    fn new(render_state: &eframe::egui_wgpu::RenderState) -> Self {
        let mut data = load_points().expect("points.csv");
        data.truncate(N);
        let shared = Arc::new(Shared::default());
        Self {
            widget_left: GupWidget::new(render_state),
            widget_right: GupWidget::new(render_state),
            left: left_chart(&data, shared.clone()),
            right: right_chart(&data, &[]),
            data,
            shared,
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
                let size = egui::vec2(W, H);
                self.widget_left.show(ui, size, &mut self.left);
                self.widget_right.show(ui, size, &mut self.right);
            });
        });
        // Propagate selection changes: rebuild the right chart.
        let g = self.shared.generation.load(Ordering::SeqCst);
        if g != self.last_gen {
            self.last_gen = g;
            let t0 = Instant::now();
            let rows = self.shared.rows.lock().unwrap().clone();
            self.right = right_chart(&self.data, &rows);
            self.status = format!("brushed {} of {N} (rebuild {:?})", rows.len(), t0.elapsed());
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
    eframe::run_native(
        "gup dogfood t4",
        options,
        Box::new(|cc| {
            let rs = cc.wgpu_render_state.as_ref().expect("the wgpu backend");
            Ok(Box::new(App::new(rs)))
        }),
    )
}
