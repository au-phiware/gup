// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Heatmap builder with Observable Plot compatibility.

use super::{
    AccessorFunction, ConfigurableBuilder, apply_accessors_to_selection,
    validate_required_accessors,
};
use crate::RenderContext;
use crate::chart_builder::{ChartBuilder, ChartBuilderError, ChartConfig};
use crate::error::GupResult;
use crate::selection::Circle; // TODO: Replace with Rectangle mark when available
use crate::selection::Selection;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct HeatmapBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) fill_accessor: Option<AccessorFunction<T>>,
    pub(crate) config: ChartConfig,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> HeatmapBuilder<T> {
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            fill_accessor: None,
            config: ChartConfig::default(),
            _phantom: PhantomData,
        }
    }

    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    pub fn fill<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.fill_accessor = Some(accessor.into());
        self
    }

    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.fill_accessor = Some(accessor.into());
        self
    }
}

impl<T> Default for HeatmapBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ConfigurableBuilder for HeatmapBuilder<T> {
    fn title(mut self, title: impl Into<String>) -> Self {
        self.config.title = Some(title.into());
        self
    }

    fn width(mut self, width: f32) -> Self {
        self.config.width = width;
        self
    }

    fn height(mut self, height: f32) -> Self {
        self.config.height = height;
        self
    }

    fn background(mut self, color: [f32; 4]) -> Self {
        self.config.background_color = Some(color);
        self
    }

    fn show_axes(mut self, show: bool) -> Self {
        self.config.show_axes = show;
        self
    }

    fn show_grid(mut self, show: bool) -> Self {
        self.config.show_grid = show;
        self
    }
}

impl<T> ChartBuilder<T> for HeatmapBuilder<T>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    type Output = Selection<T, Circle>; // TODO: Replace with Rectangle mark

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;

        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        let mut selection = Selection::<T, Circle>::new(data, context)?;

        apply_accessors_to_selection(
            &mut selection,
            &self.x_accessor,
            &self.y_accessor,
            &self.fill_accessor,
            &None,
        )?;

        Ok(selection)
    }
}

pub fn heatmap<T>() -> HeatmapBuilder<T> {
    HeatmapBuilder::new()
}
