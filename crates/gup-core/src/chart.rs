// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The chart interface: anything that resolves to a [`Scene`](crate::Scene).

use crate::context::Context;
use crate::error::Result;
use crate::geom::Point;
use crate::input::{Input, Response};
use crate::pick::RowId;
use crate::plot::{Layout, Resolved};
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
/// use gup_core::{Input, Response};
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
/// let resolved = chart.resolve(&cx, 320.0, 200.0)?;
/// let rect = resolved.layout.plot;
/// let at = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
/// let layout = chart.layout().expect("resolved");
/// assert_eq!((layout.width, layout.height), (320.0, 200.0));
/// assert_eq!(
///     chart.handle(Input::wheel(at, 1.0)),
///     Response { redraw: true }
/// );
/// let image = chart.render(&cx, 320, 200)?;
/// assert_eq!((image.width(), image.height()), (320, 200));
/// assert!(chart.as_any_mut().downcast_mut::<Plot>().is_some());
/// # Ok(())
/// # }
/// ```
pub trait Chart: 'static {
    /// Resolve for a `width × height` logical-pixel chart. Cached: returns
    /// the previous result, doing no work and no GPU writes, when nothing
    /// changed.
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved>;
    /// The layout of the last successful resolve.
    fn layout(&self) -> Option<&Layout>;
    /// React to input; never fails.
    fn handle(&mut self, input: Input) -> Response;
    /// The topmost row drawn under `at` (logical pixels), if the chart can
    /// say; see [`Plot::pick_row`](crate::Plot::pick_row).
    fn pick_row(&mut self, at: Point) -> Option<RowId>;
    /// The chart is waiting on the GPU (a pick in flight) and wants another
    /// [`resolve`](Self::resolve) soon even without input. Hosts redraw while
    /// it is true; it is `false` unless a plot has a layer the CPU cannot
    /// read (see [`Plot::on_row`](crate::Plot::on_row)).
    ///
    /// ```
    /// use gup_core::prelude::*;
    ///
    /// struct Row { x: f64, y: f64 }
    /// let mut plot = Plot::new();
    /// plot.circles(vec![Row { x: 1.0, y: 2.0 }]).x(|r| r.x).y(|r| r.y);
    /// assert!(!Chart::needs_frame(&plot));
    /// ```
    fn needs_frame(&self) -> bool {
        false
    }
    /// Called by hosts with a waker that schedules a frame. Charts with
    /// appenders wake it when rows arrive.
    fn set_waker(&mut self, waker: std::task::Waker) {
        let _ = waker;
    }
    /// The chart's title, if any (a window title).
    fn title_text(&self) -> Option<&str>;
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
        let layout = resolved.layout.clone();
        let image = crate::target::ImageTarget::new(cx, width, height)?
            .render_blocking(cx, &resolved.scene)?;
        Ok((image, layout))
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
