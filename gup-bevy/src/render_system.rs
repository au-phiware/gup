// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Bevy system that drives per-frame chart rendering.

use crate::chart::GupChart;
use crate::context::GupRenderContext;
use crate::texture_target::{CHART_TEXTURE_FORMAT, ChartTextureTarget};
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureUsages};

/// Bevy system that re-renders all dirty [`GupChart`] entities.
///
/// For each entity with a `GupChart` component, the system renders the chart
/// into a [`ChartTextureTarget`] texture on the GPU.  A separate render-world
/// system ([`copy_gup_textures_to_bevy`](crate::render_node::copy_gup_textures_to_bevy))
/// then copies this texture into the Bevy sprite's `GpuImage` — no CPU
/// readback or PNG encoding takes place.
pub fn gup_render_system(
    context: Option<Res<GupRenderContext>>,
    mut charts: Query<(&mut GupChart, &mut ChartTextureTarget)>,
) {
    let Some(context) = context else {
        return;
    };

    let device = context.gup_context().device.as_ref();

    for (mut chart, mut target) in &mut charts {
        if !chart.is_dirty() {
            continue;
        }

        // Resize the offscreen texture if the chart dimensions changed.
        let (w, h) = (chart.width, chart.height);
        target.ensure_size(device, w, h);

        // Render the chart directly to the offscreen GPU texture.
        match chart
            .chart_mut()
            .render_to_texture_view(&target.view, CHART_TEXTURE_FORMAT, w, h)
        {
            Ok(()) => {}
            Err(e) => {
                log::warn!("GupChart render failed: {e}");
                continue;
            }
        }

        chart.clear_dirty();
    }
}

/// System that ensures every [`GupChart`] entity also has a
/// [`ChartTextureTarget`] component.
///
/// Run before [`gup_render_system`] in the same schedule.
pub fn ensure_chart_texture_targets(
    mut commands: Commands,
    context: Option<Res<GupRenderContext>>,
    charts: Query<(Entity, &GupChart), Without<ChartTextureTarget>>,
) {
    let Some(context) = context else {
        return;
    };
    let device = context.gup_context().device.as_ref();

    for (entity, chart) in charts.iter() {
        let target = ChartTextureTarget::new(device, chart.width, chart.height);
        commands.entity(entity).insert(target);
    }
}

/// Create a blank placeholder [`Image`] with the given dimensions.
///
/// The image is created **without CPU data** (`data: None`) so Bevy allocates
/// only a GPU texture.  The texture has `COPY_DST` usage so the render-world
/// copy system can write chart pixels into it.
///
/// The format is [`CHART_TEXTURE_FORMAT`] (`Bgra8UnormSrgb`) to match the
/// offscreen rendering pipeline.
pub fn blank_chart_image(width: u32, height: u32) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        CHART_TEXTURE_FORMAT,
        bevy::asset::RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}
