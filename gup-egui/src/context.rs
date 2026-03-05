// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared GPU render context for egui integration.
//!
//! [`GupEguiContext`] wraps the wgpu resources from eframe's
//! [`RenderState`](eframe::egui_wgpu::RenderState) so that Gup charts render
//! on the same GPU device as egui — no second adapter is created and textures
//! can be shared without CPU readback.

use gup::render::RenderContext;
use std::sync::Arc;

/// Shared GPU context created from eframe's render state.
///
/// Construct this once inside the [`eframe::App`] creation callback using
/// [`from_render_state`](Self::from_render_state), then pass the returned
/// [`RenderContext`] to chart builders so they use egui's device/queue.
///
/// # Example
///
/// ```rust,ignore
/// eframe::run_native("my app", options, Box::new(|cc| {
///     let render_state = cc.wgpu_render_state.as_ref().unwrap();
///     let egui_ctx = GupEguiContext::from_render_state(render_state);
///     let chart = scatter()
///         .build_with_data(data, egui_ctx.render_context().clone())?;
///     Ok(Box::new(MyApp { chart, egui_ctx }))
/// }));
/// ```
pub struct GupEguiContext {
    /// A Gup [`RenderContext`] backed by egui's wgpu device/queue.
    render_context: Arc<RenderContext>,
}

impl GupEguiContext {
    /// Create a [`GupEguiContext`] from eframe's render state.
    ///
    /// Extracts the `device` and `queue` from the
    /// [`RenderState`](eframe::egui_wgpu::RenderState) and wraps them in a
    /// Gup [`RenderContext`]. No second GPU adapter is created.
    ///
    /// The render context's surface format is set to `Rgba8UnormSrgb` so
    /// that chart pipelines produce output compatible with egui's texture
    /// registration API.
    pub fn from_render_state(render_state: &eframe::egui_wgpu::RenderState) -> Self {
        // wgpu 27: Device, Queue, Adapter are internally reference-counted,
        // so cloning is a cheap Arc bump — not a new GPU resource.
        let device = render_state.device.clone();
        let queue = render_state.queue.clone();
        let adapter = render_state.adapter.clone();

        // We need an Instance for RenderContext::from_wgpu. Create a
        // lightweight one — it is only used for surface creation (which we
        // never do in the egui integration).
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let mut render_context = RenderContext::from_wgpu(instance, adapter, device, queue);

        // Configure the render context to compile pipelines for
        // Rgba8UnormSrgb — the format of the offscreen texture the chart
        // renders into.  This ensures mark pipelines, axis pipelines, etc.
        // all match the render target.
        render_context.set_headless_format(wgpu::TextureFormat::Rgba8UnormSrgb);

        let render_context = Arc::new(render_context);

        Self { render_context }
    }

    /// Borrow the shared [`RenderContext`].
    ///
    /// Pass a clone of this `Arc` to chart builders so charts render on the
    /// same device as egui.
    pub fn render_context(&self) -> &Arc<RenderContext> {
        &self.render_context
    }
}
