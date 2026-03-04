// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shared GPU render context resource.

use bevy::prelude::*;
use gup::context::GupContext;
use gup::render::RenderContext;
use std::sync::Arc;

/// Bevy [`Resource`] wrapping Gup's [`GupContext`].
///
/// Constructed automatically by [`GupPlugin`](crate::GupPlugin) from
/// Bevy's `RenderDevice` / `RenderQueue` — no second GPU adapter is created.
///
/// The inner [`GupContext`] is `Arc`-wrapped and can be cheaply cloned when
/// a chart builder needs its own reference.
#[derive(Resource)]
pub struct GupRenderContext {
    /// The shared GupContext backed by Bevy's wgpu device/queue.
    pub gup_context: Arc<GupContext>,
    /// A RenderContext that shares the same underlying GPU resources.
    ///
    /// Chart builders and `ComposedChart::render` consume `&mut RenderContext`,
    /// so we keep one ready for use.
    pub render_context: Arc<RenderContext>,
}

impl GupRenderContext {
    /// Create a [`GupRenderContext`] from externally-owned wgpu handles.
    ///
    /// Both the [`GupContext`] and the [`RenderContext`] are constructed from
    /// the same `Device`/`Queue`, so all GPU work ends up on the same adapter
    /// with no resource duplication.
    pub fn from_wgpu(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        let device_arc = Arc::new(device);
        let queue_arc = Arc::new(queue);

        // GupContext takes Arc<Device> + Arc<Queue>.
        let gup_context = GupContext::from_wgpu(
            // Instance and Adapter are internally reference-counted in wgpu 26,
            // so cloning is a cheap Arc bump.
            instance.clone(),
            adapter.clone(),
            Arc::clone(&device_arc),
            Arc::clone(&queue_arc),
        );

        // RenderContext takes owned Device + Queue.  wgpu::Device and
        // wgpu::Queue are internally reference-counted, so cloning them
        // does *not* create a new GPU device — it merely increments a
        // reference count.
        let device_clone: wgpu::Device = (*device_arc).clone();
        let queue_clone: wgpu::Queue = (*queue_arc).clone();
        let render_context = Arc::new(RenderContext::from_wgpu(
            instance,
            adapter,
            device_clone,
            queue_clone,
        ));

        Self {
            gup_context,
            render_context,
        }
    }

    /// Borrow the underlying [`GupContext`].
    pub fn gup_context(&self) -> &Arc<GupContext> {
        &self.gup_context
    }

    /// Borrow the underlying [`RenderContext`] (for chart builder APIs).
    pub fn render_context(&self) -> &Arc<RenderContext> {
        &self.render_context
    }
}
