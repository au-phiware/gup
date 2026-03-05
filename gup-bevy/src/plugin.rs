// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`GupPlugin`] that wires everything together.

use crate::context::GupRenderContext;
use crate::render_node::{
    cleanup_extracted_gup_charts, copy_gup_textures_to_bevy, extract_gup_charts,
};
use crate::render_system::{ensure_chart_texture_targets, gup_render_system};
use bevy::prelude::*;
use bevy::render::renderer::{RenderAdapter, RenderDevice, RenderInstance, RenderQueue};
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};

/// Bevy [`Plugin`] that integrates Gup's chart rendering into a Bevy app.
///
/// Adding this plugin to a Bevy [`App`] will:
///
/// 1. Extract Bevy's `RenderDevice` / `RenderQueue` (the wgpu handles Bevy
///    already owns).
/// 2. Construct a [`GupRenderContext`] that shares those same GPU resources —
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
        app.add_systems(
            PostUpdate,
            (ensure_chart_texture_targets, gup_render_system).chain(),
        );
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
        let render_adapter: &RenderAdapter = render_app.world().resource();
        let render_instance: &RenderInstance = render_app.world().resource();

        // Clone the underlying wgpu objects.  In wgpu 27 these types are
        // internally reference-counted, so Clone is a cheap Arc bump.
        let device: wgpu::Device = render_device.wgpu_device().clone();
        let queue: wgpu::Queue = render_queue.0.as_ref().clone().into_inner();
        let adapter: wgpu::Adapter = render_adapter.0.as_ref().clone().into_inner();
        let instance: wgpu::Instance = render_instance.0.as_ref().clone().into_inner();

        let gup_render_context = GupRenderContext::from_wgpu(instance, adapter, device, queue);

        // Insert the shared context as a Resource in the **main** world so
        // that normal Bevy systems can access it.
        app.insert_resource(gup_render_context);

        // Render-world systems:
        // 1. Extract chart textures + image handles from the main world.
        // 2. Copy chart textures into GpuImage textures (GPU → GPU).
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    ExtractSchedule,
                    (cleanup_extracted_gup_charts, extract_gup_charts).chain(),
                )
                .add_systems(
                    Render,
                    copy_gup_textures_to_bevy.in_set(RenderSystems::Queue),
                );
        }
    }
}
