// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gup charts as Bevy sprite textures.
//!
//! [`GupPlugin`] makes a [`GupContext`] on Bevy's own wgpu device, renders
//! every [`GupChart`] into a texture on that device each frame, and copies
//! the texture into the Bevy image the chart names (GPU to GPU, no
//! readback). Show the image with a [`Sprite`](bevy::sprite::Sprite).
//!
//! # Quick start
//!
//! ```no_run
//! use bevy::prelude::*;
//! use gup_bevy::prelude::*;
//! use gup_core::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, GupPlugin))
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
//!     commands.spawn(Camera2d);
//!     let size = UVec2::new(480, 320);
//!     let image = images.add(chart_image(size, 1.0));
//!     let chart = gup_core::scatter(vec![(1.0, 2.0), (2.0, 3.0)])
//!         .x(|p| p.0)
//!         .y(|p| p.1)
//!         .title("Hello Bevy");
//!     commands.spawn((GupChart::new(chart, size, image.clone()), Sprite::from_image(image)));
//! }
//! ```
//!
//! To change a chart's data, keep an `Appender` from the chart before
//! spawning it, or reach the chart with
//! `chart.as_any_mut().downcast_mut::<ScatterChart<_>>()`.

mod chart;
mod copy;
mod plugin;
pub(crate) mod render;

pub use chart::{GupChart, GupContext, chart_image};
pub use plugin::GupPlugin;

/// Everything a Bevy app needs.
pub mod prelude {
    pub use crate::{GupChart, GupContext, GupPlugin, chart_image};
}
