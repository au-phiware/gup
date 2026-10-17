// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The surface configuration shared by the window and canvas targets.

use crate::error::{Error, Result};

/// An 8-bit RGBA/BGRA format (preferring one whose non-sRGB view format can
/// be used on it), opaque alpha where offered, `COPY_SRC` where the surface
/// allows it, and `Fifo` presentation. Also returns the present modes the
/// surface supports.
pub(crate) fn surface_config(
    adapter: &wgpu::Adapter,
    surface: &wgpu::Surface<'_>,
    width: u32,
    height: u32,
) -> Result<(wgpu::SurfaceConfiguration, Vec<wgpu::PresentMode>)> {
    let caps = surface.get_capabilities(adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| {
            matches!(
                f.remove_srgb_suffix(),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            )
        })
        .ok_or_else(|| {
            Error::config(
                "surface",
                format!(
                    "{} cannot present 8-bit RGBA or BGRA to this surface (formats: {:?})",
                    adapter.get_info().name,
                    caps.formats
                ),
            )
        })?;
    let view_format = format.remove_srgb_suffix();
    let usage = if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
    } else {
        wgpu::TextureUsages::RENDER_ATTACHMENT
    };
    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else {
        caps.alpha_modes[0]
    };
    let config = wgpu::SurfaceConfiguration {
        usage,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: if view_format == format {
            vec![]
        } else {
            vec![view_format]
        },
    };
    Ok((config, caps.present_modes))
}
