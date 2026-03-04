// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bevy integration for the Gup GPU-accelerated data visualization library.
//!
//! This crate provides a [`GupPlugin`] for [Bevy](https://bevyengine.org/)
//! that shares a single wgpu `Device`/`Queue` between Bevy's renderer and
//! Gup's chart rendering pipeline.  No second GPU adapter is created.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use gup_bevy::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(GupPlugin)
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn(Camera2d);
//! }
//! ```
//!
//! See the `bevy_scatter` example for a full animated scatter-plot demo.

mod chart;
mod context;
mod plugin;
mod render_system;

/// Convenience re-exports for common usage.
pub mod prelude {
    pub use crate::chart::{DynChart, GupChart};
    pub use crate::context::GupRenderContext;
    pub use crate::plugin::GupPlugin;
    pub use crate::render_system::blank_chart_image;
}

pub use chart::{DynChart, GupChart};
pub use context::GupRenderContext;
pub use plugin::GupPlugin;
pub use render_system::{blank_chart_image, gup_render_system};
