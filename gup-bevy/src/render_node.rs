// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Render-world systems for GPU-to-GPU texture copy.
//!
//! After the main-world render system draws a chart into its
//! [`ChartTextureTarget`], these systems copy the result directly into the
//! Bevy `GpuImage` backing the entity's sprite — zero CPU readback.

use crate::texture_target::ChartTextureTarget;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;

/// Render-world component produced by the extract system.
///
/// Holds the source chart texture (which lives on the shared wgpu device)
/// and the [`AssetId<Image>`] of the Bevy sprite image to copy into.
#[derive(Component)]
pub struct ExtractedGupChart {
    /// The chart's offscreen texture (wgpu::Texture is reference-counted).
    pub source_texture: wgpu::Texture,
    /// Asset id of the Bevy Image whose GPU texture we copy into.
    pub target_image_id: AssetId<Image>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Cleanup system: despawns all previously extracted chart entities in the
/// render world before fresh ones are spawned.
///
/// In Bevy 0.18 the render world is persistent across frames, so we must
/// remove stale entities ourselves.
pub fn cleanup_extracted_gup_charts(
    mut commands: Commands,
    extracted: Query<Entity, With<ExtractedGupChart>>,
) {
    for entity in extracted.iter() {
        commands.entity(entity).despawn();
    }
}

/// Extraction system: runs in the [`ExtractSchedule`] and reads from the
/// main world.
///
/// For every entity that has both a [`ChartTextureTarget`] and a [`Sprite`],
/// we spawn a render-world entity carrying an [`ExtractedGupChart`] so the
/// copy system can run later.
pub fn extract_gup_charts(
    mut commands: Commands,
    charts: Extract<Query<(&ChartTextureTarget, &Sprite)>>,
) {
    for (target, sprite) in charts.iter() {
        commands.spawn(ExtractedGupChart {
            source_texture: target.texture.clone(),
            target_image_id: sprite.image.id(),
            width: target.width,
            height: target.height,
        });
    }
}

/// Render-world system: copies the chart texture into the Bevy sprite's
/// `GpuImage` texture.
///
/// Both textures live on the same wgpu device (guaranteed by
/// [`GupPlugin`](crate::GupPlugin)), so `copy_texture_to_texture` is a
/// pure GPU operation with no CPU involvement.
pub fn copy_gup_textures_to_bevy(
    extracted: Query<&ExtractedGupChart>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for chart in extracted.iter() {
        let Some(gpu_image) = gpu_images.get(chart.target_image_id) else {
            // GpuImage not yet prepared (first frame) — skip.
            continue;
        };

        let mut encoder = render_device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gup_chart_texture_copy"),
        });

        let copy_size = wgpu::Extent3d {
            width: chart.width.min(gpu_image.size.width),
            height: chart.height.min(gpu_image.size.height),
            depth_or_array_layers: 1,
        };

        encoder.copy_texture_to_texture(
            chart.source_texture.as_image_copy(),
            gpu_image.texture.as_image_copy(),
            copy_size,
        );

        render_queue.submit(std::iter::once(encoder.finish()));
    }
}
