// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Depth buffer management for 3D rendering.
//!
//! [`DepthBuffer`] owns a `Depth32Float` texture and provides the
//! `wgpu::TextureView` needed by render pass depth-stencil attachments.

use wgpu::{Device, Extent3d, TextureDescriptor, TextureDimension, TextureUsages, TextureView};

/// The texture format used for all 3D depth attachments.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Owns a depth/stencil texture that matches the current surface dimensions.
///
/// Call [`DepthBuffer::resize`] whenever the surface size changes.
#[derive(Debug)]
pub struct DepthBuffer {
    view: TextureView,
    width: u32,
    height: u32,
}

impl DepthBuffer {
    /// Create a new depth buffer matching the given dimensions.
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let view = Self::create_view(device, width, height);
        Self {
            view,
            width,
            height,
        }
    }

    /// Recreate the depth texture if the surface size changed.
    ///
    /// Returns `true` if the buffer was actually recreated.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) -> bool {
        if self.width == width && self.height == height {
            return false;
        }
        self.view = Self::create_view(device, width, height);
        self.width = width;
        self.height = height;
        true
    }

    /// Borrow the [`TextureView`] for use in a render pass.
    pub fn view(&self) -> &TextureView {
        &self.view
    }

    /// Current width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Current height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    // -- internal --

    fn create_view(device: &Device, width: u32, height: u32) -> TextureView {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("depth_buffer"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_format_is_depth32float() {
        assert_eq!(DEPTH_FORMAT, wgpu::TextureFormat::Depth32Float);
    }
}
