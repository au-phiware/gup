// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! egui integration for the Gup GPU-accelerated data visualization library.
//!
//! [`GupWidget`] draws any [`gup_core::Chart`] in an egui panel. It makes a
//! [`gup_core::Context`] over egui's own device, renders the chart into a
//! texture that egui samples directly (no CPU readback), and forwards
//! pointer input to [`Chart::handle`](gup_core::Chart::handle). egui
//! repaints when the chart is interacted with or fed from another thread.
//!
//! # Quick start
//!
//! ```no_run
//! use gup_egui::GupWidget;
//!
//! struct App {
//!     widget: GupWidget,
//!     chart: gup_core::ScatterChart<(f64, f64)>,
//! }
//!
//! impl App {
//!     fn new(cc: &eframe::CreationContext<'_>) -> Self {
//!         let render_state = cc.wgpu_render_state.as_ref().expect("the wgpu backend");
//!         Self {
//!             widget: GupWidget::new(render_state),
//!             chart: gup_core::scatter(vec![(1.0, 2.0), (2.0, 3.0), (3.0, 5.0)])
//!                 .x(|p| p.0)
//!                 .y(|p| p.1),
//!         }
//!     }
//! }
//!
//! impl eframe::App for App {
//!     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//!         egui::CentralPanel::default().show(ctx, |ui| {
//!             self.widget.show(ui, egui::vec2(480.0, 320.0), &mut self.chart);
//!         });
//!     }
//! }
//!
//! fn main() -> eframe::Result {
//!     let options = eframe::NativeOptions {
//!         renderer: eframe::Renderer::Wgpu,
//!         ..Default::default()
//!     };
//!     eframe::run_native("chart", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
//! }
//! ```

mod input;
mod render;
mod widget;

pub use input::to_input;
pub use widget::{GupWidget, context};
