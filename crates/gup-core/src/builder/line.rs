// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The line builder: [`line`] and [`LineChart`].

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use super::{DRAWN, plot_setters};
use crate::geom::Point;
use crate::prelude::*;
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Key<T> = Arc<dyn for<'a> Fn(&'a T) -> &'a str + Send + Sync>;
type Op<T> = Box<dyn Fn(&mut Selection<T, Line>) + Send + Sync>;

/// Fix a closure's signature to the higher-ranked key form.
fn key_fn<T, F: for<'a> Fn(&'a T) -> &'a str>(f: F) -> F {
    f
}

enum ColorSpec<T> {
    Default,
    Stroke(Color),
    Key(Key<T>),
}

/// Start a line chart over `rows`: a polyline through `(x, y)`, or one per
/// key with [`color`](LineChart::color).
///
/// Rows are drawn in data order within a line, so sort them by x first. A
/// null (`None`, NaN) in x or y breaks the line. Many series make many
/// layers; that is fine up to a few hundred.
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Price { day: f64, ticker: &'static str, price: Option<f64> }
/// let rows = (0..20).flat_map(|d| {
///     ["AAA", "BBB"].map(|t| Price {
///         day: f64::from(d),
///         ticker: t,
///         price: (d != 8).then(|| f64::from(d) + if t == "AAA" { 0.0 } else { 5.0 }),
///     })
/// });
///
/// let mut chart = gup_core::line(rows)
///     .x(|r| r.day)
///     .y(|r| r.price)
///     .color(|r| r.ticker);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// See the [module docs](crate::builder) for the pattern.
pub fn line<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> LineChart<T> {
    let mut plot = Plot::new();
    plot.x(Linear::new());
    plot.y(Linear::new());
    LineChart {
        plot,
        rows: Some(rows.into_iter().collect()),
        ops: Vec::new(),
        color: ColorSpec::Default,
        color_scale: Categorical::okabe_ito(),
        legend_title: None,
    }
}

/// A line chart under construction, and a [`Chart`].
///
/// # Panics
///
/// The layer setters (`x`, `y`, `color*`, `stroke`, `width`, `tooltip`,
/// `legend_title`) panic once the chart has been drawn, with "set layer
/// encodings before the chart is first drawn; change a drawn chart through
/// layer_mut()". The plot-level setters work at any time.
#[must_use]
pub struct LineChart<T> {
    plot: Plot,
    rows: Option<Vec<T>>,
    ops: Vec<Op<T>>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    legend_title: Option<String>,
}

impl<T> std::fmt::Debug for LineChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static> LineChart<T> {
    plot_setters!();

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    fn op(mut self, op: impl Fn(&mut Selection<T, Line>) + Send + Sync + 'static) -> Self {
        self.check_open();
        self.ops.push(Box::new(op));
        self
    }

    /// The x position of a row, through the x scale.
    pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        let f = Arc::new(f);
        self.op(move |s| {
            let f = Arc::clone(&f);
            s.x(move |r: &T| f(r));
        })
    }

    /// The y position of a row, through the y scale. `None` breaks the line.
    pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        let f = Arc::new(f);
        self.op(move |s| {
            let f = Arc::clone(&f);
            s.y(move |r: &T| f(r));
        })
    }

    /// The x scale (default [`Linear`]).
    pub fn x_scale<S: PositionScale>(mut self, scale: S) -> Self {
        self.plot.x(scale);
        self
    }

    /// The y scale (default [`Linear`]).
    pub fn y_scale<S: PositionScale>(mut self, scale: S) -> Self {
        self.plot.y(scale);
        self
    }

    /// One line per key, in first-seen order, coloured by key. The last of
    /// `color` and `stroke` wins.
    pub fn color(mut self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self {
        self.check_open();
        self.color = ColorSpec::Key(Arc::new(key));
        self
    }

    /// The scale for [`color`](Self::color) (default
    /// [`Categorical::okabe_ito`]).
    pub fn color_scale(mut self, scale: Categorical) -> Self {
        self.check_open();
        self.color_scale = scale;
        self
    }

    /// One colour for every line. The last of `color` and `stroke` wins.
    pub fn stroke(mut self, color: Color) -> Self {
        self.check_open();
        self.color = ColorSpec::Stroke(color);
        self
    }

    /// The line width in logical pixels (default 1.5).
    pub fn width(self, px: f32) -> Self {
        self.op(move |s| {
            s.width(Px(px));
        })
    }

    /// Title the colour legend.
    pub fn legend_title(mut self, title: impl Into<String>) -> Self {
        self.check_open();
        self.legend_title = Some(title.into());
        self
    }

    /// The tooltip text of a row.
    pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self {
        let text = Arc::new(text);
        self.op(move |s| {
            let text = Arc::clone(&text);
            s.tooltip(move |r: &T| text(r));
        })
    }

    /// Partition the rows by key, moving them, in first-seen key order.
    fn split(rows: Vec<T>, key: Option<&Key<T>>) -> Vec<Vec<T>> {
        let Some(key) = key else {
            return vec![rows];
        };
        let mut index: HashMap<String, usize> = HashMap::new();
        let slots: Vec<usize> = rows
            .iter()
            .map(|r| {
                let k = key(r);
                if let Some(&i) = index.get(k) {
                    i
                } else {
                    let i = index.len();
                    index.insert(k.to_owned(), i);
                    i
                }
            })
            .collect();
        let mut series: Vec<Vec<T>> = (0..index.len()).map(|_| Vec::new()).collect();
        for (row, slot) in rows.into_iter().zip(slots) {
            series[slot].push(row);
        }
        series
    }

    /// Build the layers from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        let key = match &self.color {
            ColorSpec::Key(key) => Some(key),
            _ => None,
        };
        let scale = ScaleRef::new(self.color_scale.clone());
        for series in Self::split(rows, key) {
            let sel = self.plot.line(series);
            for op in &self.ops {
                op(sel);
            }
            match &self.color {
                ColorSpec::Default => {}
                ColorSpec::Stroke(c) => {
                    sel.color(*c);
                }
                ColorSpec::Key(key) => {
                    let key = Arc::clone(key);
                    sel.color(scale.encode_key(key_fn(move |r: &T| key(r))));
                }
            }
            if let Some(title) = &self.legend_title {
                sel.legend_title(Line::COLOR, title.clone());
            }
        }
        self.ops.clear();
    }

    /// The line layers, one per key in first-seen order (one without
    /// [`color`](Self::color)), for settings the builder has no method for.
    /// Materialises the chart: layer setters panic after.
    pub fn layers_mut(&mut self) -> Vec<&mut Selection<T, Line>> {
        self.materialise();
        self.plot.layers_mut::<T, Line>().collect()
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

    /// The row at the nearer end of the segment under `at` (logical
    /// pixels), once the chart has been resolved.
    pub fn pick(&mut self, at: Point) -> Option<&T> {
        self.materialise();
        self.plot.pick::<T>(at)
    }
}

impl<T: Send + Sync + 'static> Chart for LineChart<T> {
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
