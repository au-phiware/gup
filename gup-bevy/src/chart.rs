// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`GupChart`] component and the [`GupContext`] resource.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

/// The [`gup_core::Context`] over Bevy's render device. A main-world
/// resource, made in [`GupPlugin::finish`](crate::GupPlugin).
#[derive(Resource, Clone)]
pub struct GupContext(pub gup_core::Context);

/// A chart drawn into `image` at `size` logical pixels and `dpr`.
#[derive(Component)]
pub struct GupChart {
    /// The chart. Reach the concrete type with `as_any_mut`.
    pub chart: Box<dyn gup_core::Chart + Send + Sync>,
    /// Logical size in pixels.
    pub size: UVec2,
    /// Device pixels per logical pixel.
    pub dpr: f32,
    /// The image the chart is copied into; make it with [`chart_image`].
    pub image: Handle<Image>,
}

impl GupChart {
    /// A chart at device pixel ratio 1.
    pub fn new(
        chart: impl gup_core::Chart + Send + Sync,
        size: UVec2,
        image: Handle<Image>,
    ) -> Self {
        Self {
            chart: Box::new(chart),
            size,
            dpr: 1.0,
            image,
        }
    }

    /// The size in device pixels, which the image must have.
    pub fn pixel_size(&self) -> UVec2 {
        (self.size.as_vec2() * self.dpr)
            .round()
            .as_uvec2()
            .max(UVec2::ONE)
    }
}

/// A blank `Rgba8UnormSrgb` image of `size × dpr` pixels for a
/// [`GupChart`] to be copied into. It has no CPU data.
pub fn chart_image(size: UVec2, dpr: f32) -> Image {
    let px = (size.as_vec2() * dpr).round().as_uvec2().max(UVec2::ONE);
    let mut image = Image::new_uninit(
        Extent3d {
            width: px.x,
            height: px.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}
