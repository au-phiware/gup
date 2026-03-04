// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`GupChart`] ECS component.

use bevy::prelude::*;
use gup::chart_builder::ComposedChart;
use gup::error::GupResult;
use gup::render::RenderContext;

/// Object-safe trait that allows type-erased chart rendering.
///
/// Implemented automatically for any [`ComposedChart<T, M>`] whose type
/// parameters satisfy the required bounds, so users never need to implement
/// this trait manually.
pub trait DynChart: Send + Sync + 'static {
    /// Prepare GPU resources and record draw commands.
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()>;

    /// Render the chart to PNG bytes at the given pixel dimensions.
    fn render_to_png(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>>;
}

impl<T, M> DynChart for ComposedChart<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    M: gup::mark::Mark,
{
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        ComposedChart::render(self, context)
    }

    fn render_to_png(&mut self, width: u32, height: u32) -> GupResult<Vec<u8>> {
        ComposedChart::render_to_png(self, width, height)
    }
}

/// Bevy [`Component`] that wraps a Gup chart for ECS-driven rendering.
///
/// `GupChart` holds a type-erased chart (any [`ComposedChart<T, M>`]) and an
/// `auto_update` flag that controls whether the render system re-renders it
/// every frame.
///
/// # Examples
///
/// ```rust,ignore
/// use gup_bevy::GupChart;
///
/// // Wrap any chart built with the Gup chart-builder API.
/// let gup_chart = GupChart::new(my_scatter_chart);
///
/// // Spawn as a regular Bevy entity with a sprite.
/// commands.spawn((gup_chart, Sprite::default(), Transform::default()));
/// ```
#[derive(Component)]
pub struct GupChart {
    /// The type-erased chart.
    chart: Box<dyn DynChart>,
    /// When `true` the render system re-renders every frame.
    pub auto_update: bool,
    /// Internal dirty flag — set to `true` when the chart needs re-rendering.
    dirty: bool,
    /// Pixel width for offscreen rendering.
    pub width: u32,
    /// Pixel height for offscreen rendering.
    pub height: u32,
}

impl GupChart {
    /// Create a new `GupChart` with `auto_update` set to `true`.
    pub fn new(chart: impl DynChart) -> Self {
        Self {
            chart: Box::new(chart),
            auto_update: true,
            dirty: true,
            width: 800,
            height: 600,
        }
    }

    /// Create a new `GupChart` with an explicit `auto_update` flag.
    pub fn with_auto_update(chart: impl DynChart, auto_update: bool) -> Self {
        Self {
            chart: Box::new(chart),
            auto_update,
            dirty: true,
            width: 800,
            height: 600,
        }
    }

    /// Set the pixel dimensions for offscreen rendering.
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self.dirty = true;
        self
    }

    /// Mark the chart as needing a re-render on the next frame.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Returns `true` if the chart will be re-rendered on the next frame.
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.auto_update
    }

    /// Clear the dirty flag (called by the render system after rendering).
    pub(crate) fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Borrow the inner chart for direct manipulation.
    pub fn chart(&self) -> &dyn DynChart {
        &*self.chart
    }

    /// Mutably borrow the inner chart.
    pub fn chart_mut(&mut self) -> &mut dyn DynChart {
        &mut *self.chart
    }

    /// Render the chart to PNG bytes at the configured dimensions.
    pub fn render_to_png(&mut self) -> GupResult<Vec<u8>> {
        self.chart.render_to_png(self.width, self.height)
    }
}
