// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A scatter with tooltips beside a live line fed from a thread.
//! `GUP_SCREENSHOT_PATH=/tmp/x.png` saves an egui screenshot and exits.

use gup_core::prelude::*;
use gup_core::{LineChart, ScatterChart};
use gup_egui::GupWidget;

/// The scatter's data; `tests/screenshot_parity.rs` rebuilds it.
fn scatter_chart() -> ScatterChart<(f64, f64)> {
    let pts = (0..80).map(|i| (i as f64 * 0.5 % 10.0, ((i * 7) % 20) as f64 / 2.0));
    gup_core::scatter(pts.collect::<Vec<_>>())
        .x(|p| p.0)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y(|p| p.1)
        .y_scale(Linear::new().domain(0.0, 10.0))
        .fill(Color::hex(0x1f77b4))
        .radius(5.0)
        .tooltip(|p| format!("x {:.1}, y {:.1}", p.0, p.1))
        .title("Scatter (hover for tooltips)")
}

struct App {
    scatter: ScatterChart<(f64, f64)>,
    line: LineChart<(f64, f64)>,
    widgets: [GupWidget; 2],
    frame: u32,
    shot: Option<String>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        let size = egui::vec2(420.0, 320.0);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.widgets[0].show(ui, size, &mut self.scatter);
                self.widgets[1].show(ui, size, &mut self.line);
            });
        });
        let Some(path) = self.shot.clone() else {
            return;
        };
        ctx.request_repaint();
        if self.frame == 30 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let [w, h] = img.size;
            image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8).unwrap();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn main() -> eframe::Result {
    let mut line = gup_core::line(vec![(0.0, 0.0)])
        .x(|p| p.0)
        .x_scale(Linear::new().domain(0.0, 100.0))
        .y(|p| p.1)
        .y_scale(Linear::new().domain(-2.0, 2.0))
        .stroke(Color::hex(0xd62728))
        .title("Live line (appended from a thread)");
    let feed = line.appender();
    std::thread::spawn(move || {
        for i in 1..100 {
            let x = i as f64;
            if feed.push((x, (x * 0.2).sin())).is_err() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    let shot = std::env::var("GUP_SCREENSHOT_PATH").ok();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 380.0]),
        ..Default::default()
    };
    eframe::run_native(
        "gup-egui",
        options,
        Box::new(|cc| {
            let rs = cc.wgpu_render_state.as_ref().expect("the wgpu backend");
            let widgets = [GupWidget::new(rs), GupWidget::new(rs)];
            let scatter = scatter_chart();
            Ok(Box::new(App {
                scatter,
                line,
                widgets,
                frame: 0,
                shot,
            }))
        }),
    )
}
