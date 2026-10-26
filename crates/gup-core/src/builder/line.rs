// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The line builder: [`line`] and [`LineChart`].

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::append::{Appender, FeedRx};
use crate::column::DictionaryKey;
use crate::geom::Point;
use crate::prelude::*;
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Op<T> = Box<dyn Fn(&mut Selection<T, Line>) + Send + Sync>;

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
        scale: None,
        keys: HashMap::new(),
        feed: None,
        waker: None,
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
    /// The colour scale shared by every key's layer, once materialised.
    scale: Option<ScaleRef<Categorical>>,
    /// Each key's index among the line layers.
    keys: HashMap<Arc<str>, usize>,
    /// Rows pushed through [`appender`](LineChart::appender) of a keyed
    /// chart, routed to the key's layer at resolve.
    feed: Option<FeedRx<T>>,
    waker: Option<Waker>,
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
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn color<K: DictionaryKey>(
        mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.color = ColorSpec::Key(erase_key(key));
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

    /// How each line runs from one row to the next (default
    /// [`Curve::Linear`]): [`Curve::StepAfter`] for a value that holds
    /// until the next row changes it, [`Curve::MonotoneX`] for a smooth
    /// curve through every row that never overshoots it. See
    /// [`Selection::curve`].
    ///
    /// ```
    /// use gup_core::prelude::*;
    /// # fn main() -> gup_core::Result<()> {
    /// struct Price { t: f64, price: f64 }
    /// let prices = vec![
    ///     Price { t: 0.0, price: 10.0 },
    ///     Price { t: 2.0, price: 12.5 },
    ///     Price { t: 5.0, price: 11.0 },
    /// ];
    /// let mut chart = gup_core::line(prices).x(|p| p.t).y(|p| p.price).curve(Curve::StepAfter);
    /// # let cx = Context::new_blocking()?;
    /// let image = chart.render(&cx, 320, 200)?;
    /// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
    /// # Ok(())
    /// # }
    /// ```
    pub fn curve(self, curve: Curve) -> Self {
        self.op(move |s| {
            s.curve(curve);
        })
    }

    /// How many rows each line keeps as rows are appended (default
    /// [`Window::All`]), per line: with [`color`](Self::color), each key's
    /// layer keeps its own newest `n` rows. A line starts at its first kept
    /// row. See [`Selection::window`].
    ///
    /// ```
    /// use gup_core::prelude::*;
    /// # fn main() -> gup_core::Result<()> {
    /// let cx = Context::new_blocking()?;
    /// let mut chart = gup_core::line(Vec::<(f64, f64)>::new())
    ///     .x(|p| p.0)
    ///     .y(|p| p.1)
    ///     .window(Window::LastRows(3_000));
    /// let feed = chart.appender();
    /// for k in 0..10 {
    ///     feed.extend((0..1_000).map(|i| (f64::from(k * 1_000 + i), 0.0))).unwrap();
    ///     chart.resolve(&cx, 320.0, 200.0)?;
    /// }
    /// let held = chart.layers_mut()[0].len();
    /// assert!((3_000..3_000 + 2 * 1_024).contains(&held), "{held}");
    /// # Ok(())
    /// # }
    /// ```
    pub fn window(self, window: Window) -> Self {
        self.op(move |s| {
            s.window(window);
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
        let mut index: HashMap<Arc<str>, usize> = HashMap::new();
        let slots: Vec<usize> = rows
            .iter()
            .map(|r| {
                let k = key(r);
                if let Some(&i) = index.get(&k) {
                    i
                } else {
                    let i = index.len();
                    index.insert(k, i);
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

    /// Add one line layer over `series`, with the recorded settings.
    fn add_series(&mut self, series: Vec<T>) {
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
                let scale = self.scale.as_ref().expect("the colour scale is made first");
                sel.color(scale.encode_owned_key(move |r: &T| key(r)));
            }
        }
        if let Some(title) = &self.legend_title {
            sel.legend_title(Line::COLOR, title.clone());
        }
    }

    /// Build the layers from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        self.scale = Some(ScaleRef::new(self.color_scale.clone()));
        let key = match &self.color {
            ColorSpec::Key(key) => Some(Arc::clone(key)),
            _ => None,
        };
        for series in Self::split(rows, key.as_ref()) {
            if let Some(key) = &key {
                let name = key(&series[0]);
                let index = self.keys.len();
                self.keys.insert(name, index);
            }
            self.add_series(series);
        }
    }

    /// Move rows pushed through a keyed chart's appender to their lines.
    fn drain_feed(&mut self) -> Result<()> {
        let Some(feed) = &self.feed else {
            return Ok(());
        };
        if !feed.pending() {
            return Ok(());
        }
        let rows = feed.drain();
        let Some(key) = (match &self.color {
            ColorSpec::Key(key) => Some(Arc::clone(key)),
            _ => None,
        }) else {
            return Ok(());
        };
        for series in Self::split(rows, Some(&key)) {
            let name = key(&series[0]);
            if let Some(&i) = self.keys.get(&name) {
                let layer = self
                    .plot
                    .layers_mut::<T, Line>()
                    .nth(i)
                    .expect("every key has a layer");
                layer.append(series)?;
            } else {
                self.keys.insert(name, self.keys.len());
                self.add_series(series);
            }
        }
        Ok(())
    }

    /// A handle that appends rows to the chart from any thread; rows
    /// pushed between two frames are drawn together by the next resolve.
    ///
    /// With [`color`](Self::color), a row of an existing key extends that
    /// key's line, and a row of a new key adds a line, in the next palette
    /// colour and with a legend entry. Materialises the chart: layer
    /// setters panic after.
    ///
    /// ```
    /// use gup_core::{ChartExt, Context};
    /// # fn main() -> gup_core::Result<()> {
    /// let mut chart = gup_core::line(vec![(0.0, 1.0)]).x(|p| p.0).y(|p| p.1);
    /// let feed = chart.appender();
    /// std::thread::spawn(move || {
    ///     for i in 1..10 {
    ///         feed.push((f64::from(i), f64::from(i * i))).unwrap();
    ///     }
    /// })
    /// .join()
    /// .unwrap();
    /// # let cx = Context::new_blocking()?;
    /// let image = chart.render(&cx, 320, 200)?;
    /// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
    /// # Ok(())
    /// # }
    /// ```
    pub fn appender(&mut self) -> Appender<T> {
        self.materialise();
        if matches!(self.color, ColorSpec::Key(_)) {
            let feed = self.feed.get_or_insert_with(FeedRx::new);
            if let Some(w) = &self.waker {
                feed.set_waker(Some(w));
            }
            feed.appender()
        } else {
            self.plot
                .layers_mut::<T, Line>()
                .next()
                .expect("an unkeyed line chart has one layer")
                .appender()
        }
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
    /// pixels), once the chart has been resolved. On a step line, the row
    /// that holds the level under `at` (see [`Curve`]).
    pub fn pick(&mut self, at: Point) -> Option<&T> {
        self.materialise();
        self.plot.pick::<T>(at)
    }
}

impl<T: Send + Sync + 'static> Chart for LineChart<T> {
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved> {
        self.materialise();
        self.drain_feed()?;
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
        Chart::needs_frame(&self.plot) || self.feed.as_ref().is_some_and(FeedRx::pending)
    }
    fn set_waker(&mut self, waker: Waker) {
        if let Some(feed) = &self.feed {
            feed.set_waker(Some(&waker));
        }
        self.plot.set_waker(waker.clone());
        self.waker = Some(waker);
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
