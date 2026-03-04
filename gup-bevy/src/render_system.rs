// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Bevy system that drives per-frame chart rendering.

use crate::chart::GupChart;
use crate::context::GupRenderContext;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

/// Bevy system that re-renders all dirty [`GupChart`] entities.
///
/// For each entity with a `GupChart` component and a [`Sprite`], the system
/// checks the dirty / `auto_update` flags.  If the chart needs re-rendering
/// it renders to an offscreen PNG and replaces the Bevy [`Image`] asset so
/// the sprite updates on screen.
///
/// The system is added automatically by [`GupPlugin`](crate::GupPlugin) in
/// the `PostUpdate` schedule.
pub fn gup_render_system(
    _context: Option<Res<GupRenderContext>>,
    mut charts: Query<(&mut GupChart, &mut Sprite)>,
    mut images: ResMut<Assets<Image>>,
) {
    for (mut chart, mut sprite) in &mut charts {
        if !chart.is_dirty() {
            continue;
        }

        // Render the chart to PNG bytes using the chart's own context.
        let png_bytes = match chart.render_to_png() {
            Ok(bytes) => bytes,
            Err(e) => {
                log::warn!("GupChart render failed: {e}");
                continue;
            }
        };

        // Decode the PNG into a Bevy Image.
        let bevy_image = match Image::from_buffer(
            &png_bytes,
            bevy::image::ImageType::Format(bevy::image::ImageFormat::Png),
            bevy::image::CompressedImageFormats::NONE,
            true,
            bevy::image::ImageSampler::default(),
            bevy::asset::RenderAssetUsages::all(),
        ) {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to decode rendered chart PNG: {e}");
                continue;
            }
        };

        // Replace the Image asset backing this sprite.
        let handle = images.add(bevy_image);
        sprite.image = handle;

        // Sync the sprite's custom_size to match the chart dimensions.
        sprite.custom_size = Some(Vec2::new(chart.width as f32, chart.height as f32));

        chart.clear_dirty();
    }
}

/// Create a blank placeholder [`Image`] with the given dimensions.
///
/// This is used when spawning a new `GupChart` entity: the blank image gives
/// Bevy something to display while the first render completes.
pub fn blank_chart_image(width: u32, height: u32) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}
