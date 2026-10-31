// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The heatmap builder: [`heatmap`] and [`HeatmapChart`].

use std::any::Any;
use std::collections::HashSet;
use std::sync::Arc;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::column::DictionaryKey;
use crate::encoding::Derived;
use crate::encoding::{KeyEncoded, LabelKey};
use crate::geom::Point;
use crate::prelude::*;
use crate::scale::ColorValueScale;
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Num<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;
type Op<T> = Box<dyn FnOnce(&mut Selection<T, Rect>) + Send + Sync>;

/// A band at least this wide (logical pixels) leaves a 1 px gap on each
/// side of a cell's axis; a narrower one is filled whole.
const GAP_FROM_PX: f32 = 8.0;

/// Start a heatmap over `rows`: one coloured cell per row, at its `x` and
/// `y` categories, coloured by a number through a continuous colour scale
/// with a colour legend. Categories run left to right (x) and top to bottom
/// (y) in first-seen order; a pair with no row draws no cell.
///
/// There is no aggregation: duplicate (x, y) pairs draw on top of each
/// other in row order, so aggregate first.
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Visits { weekday: &'static str, hour: u8, count: f64 }
/// let visits = ["Mon", "Tue", "Wed"].into_iter().flat_map(|weekday| {
///     (0..4u8).map(move |hour| Visits { weekday, hour, count: f64::from(hour) * 10.0 })
/// });
///
/// let mut chart = gup_core::heatmap(visits)
///     .x(|v| v.hour)
///     .y(|v| v.weekday)
///     .color_value(|v| v.count)
///     .legend_title("Visits");
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// See the [module docs](crate::builder) for the pattern.
pub fn heatmap<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> HeatmapChart<T> {
    let mut plot = Plot::new();
    let x = plot.x(Band::new().padding(0.0));
    let y = plot.y(Band::new().padding(0.0));
    HeatmapChart {
        plot,
        x,
        y,
        rows: Some(rows.into_iter().collect()),
        ops: Vec::new(),
        x_key: None,
        y_key: None,
        value: None,
        color_value_scale: Sequential::viridis().into(),
        legend_title: None,
    }
}

/// A heatmap under construction, and a [`Chart`].
///
/// # Panics
///
/// The layer setters (`x`, `y`, `color_value*`, `legend_title`, `tooltip`)
/// panic once the chart has been drawn, with "set layer encodings before the
/// chart is first drawn; change a drawn chart through layer_mut()". The
/// plot-level setters work at any time.
#[must_use]
pub struct HeatmapChart<T> {
    plot: Plot,
    x: ScaleRef<Band>,
    y: ScaleRef<Band>,
    rows: Option<Vec<T>>,
    ops: Vec<Op<T>>,
    x_key: Option<Key<T>>,
    y_key: Option<Key<T>>,
    value: Option<Num<T>>,
    color_value_scale: ColorValueScale,
    legend_title: Option<String>,
}

impl<T> std::fmt::Debug for HeatmapChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeatmapChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

/// A cell's size across a band axis: the band, less 1 px when the band is
/// wide enough.
fn cell_size(band: &ScaleRef<Band>) -> Derived<Px> {
    let band = band.clone();
    Derived::new(move || {
        let w = band.read().band_width().abs() as f32;
        Px(if w >= GAP_FROM_PX { w - 1.0 } else { w })
    })
}

/// The distinct keys of `rows`, first-seen.
fn domain<T>(rows: &[T], key: &Option<Key<T>>) -> Vec<Arc<str>> {
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    if let Some(key) = key {
        for r in rows {
            let k = key.label(r);
            if seen.insert(Arc::clone(&k)) {
                keys.push(k);
            }
        }
    }
    keys
}

impl<T: Send + Sync + 'static> HeatmapChart<T> {
    plot_setters!(titles);

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    /// The category of each cell's column, on a band x axis, left to right
    /// in first-seen order.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn x<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self {
        self.check_open();
        self.x_key = Some(erase_key(key));
        self
    }

    /// The category of each cell's row, on a band y axis, top to bottom in
    /// first-seen order.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn y<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self {
        self.check_open();
        self.y_key = Some(erase_key(key));
        self
    }

    /// Colour each cell by a number, through the value scale. `None` (NaN)
    /// draws the theme's null colour.
    pub fn color_value<D: ColumnValue>(
        mut self,
        f: impl Fn(&T) -> D + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.value = Some(Arc::new(move |r| f(r).to_f64()));
        self
    }

    /// The scale for [`color_value`](Self::color_value) (default
    /// [`Sequential::viridis`]): a [`Sequential`] or a [`Diverging`].
    pub fn color_value_scale(mut self, scale: impl Into<ColorValueScale>) -> Self {
        self.check_open();
        self.color_value_scale = scale.into();
        self
    }

    /// Title the colour legend.
    pub fn legend_title(mut self, title: impl Into<String>) -> Self {
        self.check_open();
        self.legend_title = Some(title.into());
        self
    }

    /// The tooltip text of a row.
    pub fn tooltip(mut self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self {
        self.check_open();
        self.ops.push(Box::new(move |s| {
            s.tooltip(text);
        }));
        self
    }

    /// Build the layer from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        let xs = domain(&rows, &self.x_key);
        // A y band's range runs bottom to top; the first category goes on
        // top.
        let mut ys = domain(&rows, &self.y_key);
        ys.reverse();
        let ops = std::mem::take(&mut self.ops);
        let sel = self.plot.rects(rows);
        if let Some(key) = &self.x_key {
            let key = key.clone();
            sel.x_with(KeyEncoded::new(LabelKey(key), self.x.clone()).domain(xs))
                .width(cell_size(&self.x));
        }
        if let Some(key) = &self.y_key {
            let key = key.clone();
            sel.y_with(KeyEncoded::new(LabelKey(key), self.y.clone()).domain(ys))
                .height(cell_size(&self.y));
        }
        for op in ops {
            op(sel);
        }
        if let Some(f) = &self.value {
            let f = Arc::clone(f);
            match self.color_value_scale.clone() {
                ColorValueScale::Sequential(s) => sel.fill(s.encode(move |r: &T| f(r))),
                ColorValueScale::Diverging(s) => sel.fill(s.encode(move |r: &T| f(r))),
            };
        }
        if let Some(title) = self.legend_title.take() {
            sel.legend_title(Rect::FILL, title);
        }
    }

    /// The cell layer, for settings the builder has no method for (a
    /// stroke, say). Materialises the chart: layer setters panic after.
    pub fn layer_mut(&mut self) -> &mut Selection<T, Rect> {
        self.materialise();
        self.plot
            .layer_mut::<T, Rect>()
            .expect("the heatmap layer was added by materialise")
    }

    /// The inner plot. Materialises the chart.
    pub fn plot_mut(&mut self) -> &mut Plot {
        self.materialise();
        &mut self.plot
    }

    /// The finished plot.
    pub fn into_plot(mut self) -> Plot {
        self.materialise();
        self.plot
    }

    /// The row of the cell under `at` (logical pixels), once the chart has
    /// been resolved.
    pub fn pick(&mut self, at: impl Into<Point>) -> Option<&T> {
        self.materialise();
        self.plot.pick::<T>(at)
    }
}

impl<T: Send + Sync + 'static> Chart for HeatmapChart<T> {
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved> {
        self.materialise();
        self.plot.resolve(cx, width, height)
    }
    fn layout(&self) -> Option<&Layout> {
        self.plot.layout()
    }
    fn handle(&mut self, input: Input) -> Response {
        self.materialise();
        self.plot.handle(input)
    }
    fn pick_row(&mut self, at: Point) -> Option<RowId> {
        self.materialise();
        self.plot.pick_row(at)
    }
    fn needs_frame(&self) -> bool {
        Chart::needs_frame(&self.plot)
    }
    fn set_waker(&mut self, waker: std::task::Waker) {
        self.plot.set_waker(waker);
    }
    fn title_text(&self) -> Option<&str> {
        self.plot.title_text()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
