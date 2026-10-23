// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The systems that draw each chart and copy it into its Bevy image.

use crate::chart::{GupChart, GupContext};
use crate::render::{chart_texture, render_into};
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use gup_core::{Renderer, TargetOptions, TextureTarget};

/// A chart's texture, target and renderer (main world).
#[derive(Component)]
pub(crate) struct ChartTexture {
    texture: wgpu::Texture,
    target: TextureTarget,
    renderer: Renderer,
    px: UVec2,
    dpr: f32,
}

/// Draw every chart into its texture, making or resizing the texture as
/// needed.
pub(crate) fn render_charts(
    mut commands: Commands,
    cx: Option<Res<GupContext>>,
    mut charts: Query<(Entity, &mut GupChart, Option<&mut ChartTexture>)>,
) {
    let Some(cx) = cx else { return };
    let cx = &cx.0;
    for (entity, mut chart, texture) in &mut charts {
        let px = chart.pixel_size();
        let dpr = chart.dpr;
        let fresh = |cx: &gup_core::Context| -> Option<ChartTexture> {
            let texture = chart_texture(cx.device(), (px.x, px.y));
            let target =
                TextureTarget::new(cx, texture.clone(), TargetOptions::default().with_dpr(dpr))
                    .inspect_err(|e| log::warn!("GupChart target failed: {e}"))
                    .ok()?;
            Some(ChartTexture {
                texture,
                target,
                renderer: Renderer::new(),
                px,
                dpr,
            })
        };
        let size = (chart.size.x as f32, chart.size.y as f32);
        match texture {
            Some(mut t) => {
                if t.px != px || t.dpr != dpr {
                    let Some(new) = fresh(cx) else { continue };
                    *t = new;
                }
                let t = &mut *t;
                if let Err(e) =
                    render_into(cx, &mut t.renderer, &mut *chart.chart, size, &mut t.target)
                {
                    log::warn!("GupChart render failed: {e}");
                }
            }
            None => {
                let Some(mut t) = fresh(cx) else { continue };
                if let Err(e) =
                    render_into(cx, &mut t.renderer, &mut *chart.chart, size, &mut t.target)
                {
                    log::warn!("GupChart render failed: {e}");
                }
                commands.entity(entity).insert(t);
            }
        }
    }
}

/// A chart's texture and destination, in the render world.
#[derive(Component)]
pub(crate) struct ExtractedChart {
    source: wgpu::Texture,
    image: AssetId<Image>,
    px: UVec2,
}

/// Replace last frame's extracted charts with this frame's.
pub(crate) fn extract_charts(
    mut commands: Commands,
    old: Query<Entity, With<ExtractedChart>>,
    charts: Extract<Query<(&ChartTexture, &GupChart)>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    for (t, chart) in &charts {
        commands.spawn(ExtractedChart {
            source: t.texture.clone(),
            image: chart.image.id(),
            px: t.px,
        });
    }
}

/// Copy each chart texture into its `GpuImage` (GPU to GPU).
pub(crate) fn copy_charts(
    extracted: Query<&ExtractedChart>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    for chart in &extracted {
        let Some(gpu) = gpu_images.get(chart.image) else {
            continue; // not prepared yet
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gup bevy chart copy"),
        });
        encoder.copy_texture_to_texture(
            chart.source.as_image_copy(),
            gpu.texture.as_image_copy(),
            wgpu::Extent3d {
                width: chart.px.x.min(gpu.size.width),
                height: chart.px.y.min(gpu.size.height),
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
    }
}
