// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`GupPlugin`] that wires everything together.

use crate::chart::GupContext;
use crate::copy::{copy_charts, extract_charts, render_charts};
use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};

/// Bevy [`Plugin`] that integrates Gup's chart rendering into a Bevy app.
///
/// Adding this plugin to a Bevy [`App`] will:
///
/// 1. Extract Bevy's `RenderDevice` / `RenderQueue` (the wgpu handles Bevy
///    already owns).
/// 2. Construct a [`GupContext`] that shares those same GPU resources —
///    no second adapter or device is created.
/// 3. Register the main-world systems that render charts into offscreen
///    textures.
/// 4. Register render-world systems that copy those textures directly into
///    the Bevy sprite's `GpuImage` — zero CPU readback.
///
/// # Examples
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use gup_bevy::prelude::*;
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(GupPlugin)
///     .run();
/// ```
pub struct GupPlugin;

impl Plugin for GupPlugin {
    fn build(&self, app: &mut App) {
        // Main-world systems:
        // 1. Ensure every GupChart has a ChartTextureTarget.
        // 2. Render dirty charts into their offscreen textures.
        app.add_systems(PostUpdate, render_charts);
    }

    fn finish(&self, app: &mut App) {
        // At this point the RenderPlugin has finished, so the render
        // resources are available in the main world.
        let render_app = match app.get_sub_app(bevy::render::RenderApp) {
            Some(ra) => ra,
            None => {
                log::warn!("GupPlugin: RenderApp not found; skipping context creation");
                return;
            }
        };

        // Extract the raw wgpu handles from Bevy's render resources.
        let render_device: &RenderDevice = render_app.world().resource();
        let render_queue: &RenderQueue = render_app.world().resource();
        let device: wgpu::Device = render_device.wgpu_device().clone();
        let queue: wgpu::Queue = render_queue.0.as_ref().clone().into_inner();
        let context = GupContext(gup_core::Context::from_wgpu(device, queue));

        // Insert the shared context as a Resource in the **main** world so
        // that normal Bevy systems can access it.
        app.insert_resource(context);

        // Render-world systems:
        // 1. Extract chart textures + image handles from the main world.
        // 2. Copy chart textures into GpuImage textures (GPU → GPU).
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(ExtractSchedule, extract_charts)
                .add_systems(Render, copy_charts.in_set(RenderSystems::Queue));
        }
    }
}
