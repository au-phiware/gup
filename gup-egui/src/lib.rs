// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! egui integration for the Gup GPU-accelerated data visualization library.
//!
//! This crate provides [`GupWidget`], a stateful widget that renders a Gup
//! chart inside any [`egui`] panel. Charts are rendered off-screen to a GPU
//! texture via Gup's existing rendering pipeline, then uploaded to egui's
//! texture manager as a [`ColorImage`](egui::ColorImage). Pointer events
//! emitted by egui are translated into Gup's [`InteractionEvent`] type and
//! exposed for the host application to dispatch.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use gup_egui::GupWidget;
//!
//! // Wrap any chart that implements DynChart.
//! let mut widget = GupWidget::new(my_chart);
//!
//! // Inside your egui frame:
//! ui.add(&mut widget);
//!
//! // When data changes:
//! widget.mark_dirty();
//! ```
//!
//! See the `egui_chart` example for a full live-updating scatter plot demo.

mod bridge;
mod widget;

/// Convenience re-exports for common usage.
pub mod prelude {
    pub use crate::bridge::translate_response;
    pub use crate::widget::{DynChart, GupWidget};
}

pub use bridge::translate_response;
pub use widget::{DynChart, GupWidget};
