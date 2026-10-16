// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The chart interface: anything that resolves to a [`Scene`](crate::Scene).

use crate::context::Context;
use crate::error::Result;
#[cfg(not(target_arch = "wasm32"))]
use crate::plot::Layout;
use crate::plot::Resolved;
use std::any::Any;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// Something that resolves to a scene of `width × height` logical pixels.
///
/// Hold any chart as a `Box<dyn Chart>` and render it through
/// [`ChartExt`]. There is no `Send` or `Sync` supertrait (a plot holds
/// wgpu handles that are not `Send` on wasm32); write
/// `Box<dyn Chart + Send>` where that is needed.
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// plot.add(Selection::<(f64, f64), Circle>::new(vec![(1.0, 2.0), (3.0, 1.0)]))
///     .attr(Circle::X, x.encode(|p: &(f64, f64)| p.0))
///     .attr(Circle::Y, y.encode(|p: &(f64, f64)| p.1));
///
/// let mut chart: Box<dyn Chart> = Box::new(plot);
/// let image = chart.render(&cx, 320, 200)?;
/// assert_eq!((image.width(), image.height()), (320, 200));
/// assert!(chart.as_any_mut().downcast_mut::<Plot>().is_some());
/// # Ok(())
/// # }
/// ```
pub trait Chart: 'static {
    /// Resolve domains, layout and GPU state for a `width × height`
    /// logical-pixel chart.
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<Resolved>;
    /// The chart as [`Any`], to downcast to its concrete type.
    fn as_any(&self) -> &dyn Any;
    /// The chart as mutable [`Any`], to downcast to its concrete type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Rendering and saving for every [`Chart`], including `dyn Chart`.
pub trait ChartExt: Chart {
    /// Resolve and render to an image of `width × height` pixels (dpr 1).
    #[cfg(not(target_arch = "wasm32"))]
    fn render(&mut self, cx: &Context, width: u32, height: u32) -> Result<image::RgbaImage> {
        Ok(self.render_resolved(cx, width, height)?.0)
    }

    /// Like [`render`](Self::render), also returning the layout.
    #[cfg(not(target_arch = "wasm32"))]
    fn render_resolved(
        &mut self,
        cx: &Context,
        width: u32,
        height: u32,
    ) -> Result<(image::RgbaImage, Layout)> {
        let resolved = self.resolve(cx, width as f32, height as f32)?;
        let image = crate::target::ImageTarget::new(cx, width, height)?
            .render_blocking(cx, &resolved.scene)?;
        Ok((image, resolved.layout))
    }

    /// Resolve, render and write a PNG.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_png(
        &mut self,
        cx: &Context,
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        crate::target::save_png(&self.render(cx, width, height)?, path)
    }
}

impl<C: Chart + ?Sized> ChartExt for C {}
