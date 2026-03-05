// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offscreen texture target for chart rendering.
//!
//! [`ChartTextureTarget`] manages a reusable `wgpu::Texture` that Gup charts
//! render into.  After rendering, the texture is GPU-copied into the Bevy
//! `GpuImage` backing the sprite — no CPU readback required.

use bevy::prelude::*;

/// The texture format used for chart offscreen rendering.
///
/// `Bgra8UnormSrgb` matches wgpu's default surface format and the format
/// used by Gup's existing render pipelines.
pub const CHART_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// Component that manages an offscreen `wgpu::Texture` for a single chart.
///
/// Added alongside [`GupChart`](crate::GupChart) by the render system.
/// Holds the GPU texture that the chart is rendered into before being copied
/// to the Bevy sprite's `GpuImage`.
#[derive(Component)]
pub struct ChartTextureTarget {
    /// The offscreen texture the chart renders into.
    pub texture: wgpu::Texture,
    /// Cached view for the texture (avoids re-creating every frame).
    pub view: wgpu::TextureView,
    /// Current width in pixels.
    pub width: u32,
    /// Current height in pixels.
    pub height: u32,
}

impl ChartTextureTarget {
    /// Create a new offscreen texture target at the given pixel dimensions.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (texture, view) = Self::create_texture(device, width, height);
        Self {
            texture,
            view,
            width,
            height,
        }
    }

    /// Recreate the texture if dimensions have changed.
    ///
    /// Returns `true` if the texture was recreated.
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        if self.width == width && self.height == height {
            return false;
        }
        let (texture, view) = Self::create_texture(device, width, height);
        self.texture = texture;
        self.view = view;
        self.width = width;
        self.height = height;
        true
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gup_chart_offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CHART_TEXTURE_FORMAT,
            // RENDER_ATTACHMENT: so we can render into it.
            // COPY_SRC: so we can GPU-copy into the Bevy GpuImage texture.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}
