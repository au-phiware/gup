// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`GupPlugin`] that wires everything together.

use crate::context::GupRenderContext;
use crate::render_system::gup_render_system;
use bevy::prelude::*;
use bevy::render::renderer::{RenderAdapter, RenderDevice, RenderInstance, RenderQueue};

/// Bevy [`Plugin`] that integrates Gup's chart rendering into a Bevy app.
///
/// Adding this plugin to a Bevy [`App`] will:
///
/// 1. Extract Bevy's `RenderDevice` / `RenderQueue` (the wgpu handles Bevy
///    already owns).
/// 2. Construct a [`GupRenderContext`] that shares those same GPU resources —
///    no second adapter or device is created.
/// 3. Register the [`gup_render_system`] in the `PostUpdate` schedule so that
///    all [`GupChart`](crate::GupChart) entities are rendered each frame.
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
        // Register the per-frame chart render system.
        app.add_systems(PostUpdate, gup_render_system);
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

        // Clone the underlying wgpu objects.  In wgpu 26 these types are
        // internally reference-counted, so Clone is a cheap Arc bump.
        let device: wgpu::Device = render_device.wgpu_device().clone();
        let queue: wgpu::Queue = render_queue.0.as_ref().clone().into_inner();
        let adapter: wgpu::Adapter = render_adapter.0.as_ref().clone().into_inner();
        let instance: wgpu::Instance = render_instance.0.as_ref().clone().into_inner();

        let gup_render_context = GupRenderContext::from_wgpu(instance, adapter, device, queue);

        // Insert the shared context as a Resource in the **main** world so
        // that normal Bevy systems can access it.
        app.insert_resource(gup_render_context);
    }
}
