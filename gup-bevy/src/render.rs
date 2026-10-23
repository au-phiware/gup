// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The main-world render path, free of Bevy types so tests can drive it.

use gup_core::{Chart, Context, Renderer, Result, TextureTarget};

/// Resolve `chart` at `size` (logical pixels) and draw it into `target`,
/// whose dpr is the chart's (Gup's own pass and submit).
///
/// A clean chart re-resolves to the same scene, so calling this on every
/// frame costs one draw and no uploads.
pub(crate) fn render_into(
    cx: &Context,
    renderer: &mut Renderer,
    chart: &mut dyn Chart,
    size: (f32, f32),
    target: &mut TextureTarget,
) -> Result<()> {
    let resolved = chart.resolve(cx, size.0, size.1)?;
    renderer.render(cx, &resolved.scene, target)
}

/// The `Rgba8UnormSrgb` texture a chart is drawn into, with the non-sRGB
/// view Gup draws through listed.
pub(crate) fn chart_texture(device: &wgpu::Device, px: (u32, u32)) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("gup bevy chart"),
        size: wgpu::Extent3d {
            width: px.0,
            height: px.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
    })
}
