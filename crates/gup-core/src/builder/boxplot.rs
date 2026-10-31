// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The box plot builder: [`boxplot`], [`BoxPlotChart`] and [`BoxSummary`].

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::column::DictionaryKey;
use crate::geom::Point;
use crate::prelude::*;
use crate::transform::{BoxStats, box_stats};
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Value<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;
type Text = Arc<dyn Fn(&BoxSummary) -> String + Send + Sync>;

/// The box's width as a fraction of its band.
const BOX_WIDTH: f64 = 0.6;
/// The box's fill opacity; its stroke is opaque.
const BOX_OPACITY: f32 = 0.3;
/// The radius of an outlier's disc (logical pixels).
const OUTLIER_RADIUS: f32 = 2.5;

enum ColorSpec<T> {
    Default,
    Fill(Color),
    Key(Key<T>),
}

/// The summary of one category's values: the row of a box, and what a
/// [`tooltip`](BoxPlotChart::tooltip) describes.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxSummary {
    /// The category (its label).
    pub key: Arc<str>,
    /// The statistics of the category's values.
    pub stats: BoxStats,
}

/// A box, with the key that colours it.
#[derive(Clone)]
struct Boxed {
    summary: BoxSummary,
    color: Arc<str>,
}

/// A whisker, a cap, a median or an outlier: a category and one or two
/// data values on the value axis.
#[derive(Clone)]
struct Part {
    key: Arc<str>,
    color: Arc<str>,
    y: f64,
    y_end: f64,
}

/// Start a box plot over `rows`: for each category [`x`](BoxPlotChart::x)
/// reads (in first-seen order, on a band axis), a box from the first to the
/// third quartile of the values [`y`](BoxPlotChart::y) reads, a line at the
/// median, whiskers to the most extreme values within 1.5 IQR of the box
/// ([`box_stats`]) and a disc for each value beyond them.
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Request { service: &'static str, ms: f64 }
/// let requests = (0..60).map(|i| Request {
///     service: ["auth", "search", "cart"][i % 3],
///     ms: 20.0 + (i * 7 % 23) as f64 * (1 + i % 3) as f64,
/// });
///
/// let mut chart = gup_core::boxplot(requests)
///     .x(|r| r.service)
///     .y(|r| r.ms);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// See the [module docs](crate::builder) for the pattern.
pub fn boxplot<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> BoxPlotChart<T> {
    let mut plot = Plot::new();
    plot.y(Linear::new());
    let band = plot.x(Band::new());
    BoxPlotChart {
        plot,
        band,
        rows: Some(rows.into_iter().collect()),
        cat_key: None,
        value: None,
        color: ColorSpec::Default,
        color_scale: Categorical::okabe_ito(),
        legend_title: None,
        tooltip: None,
    }
}

/// A box plot under construction, and a [`Chart`].
///
/// A chart without [`x`](Self::x) and [`y`](Self::y) draws nothing. The
/// layers, in order: boxes, whiskers, caps, medians and, if any category
/// has outliers, the outliers.
///
/// # Panics
///
/// The layer setters (`x`, `y`, `padding`, `color*`, `fill`, `tooltip`,
/// `legend_title`) panic once the chart has been drawn, with "set layer
/// encodings before the chart is first drawn; change a drawn chart through
/// layer_mut()". The plot-level setters work at any time.
#[must_use]
pub struct BoxPlotChart<T> {
    plot: Plot,
    band: ScaleRef<Band>,
    rows: Option<Vec<T>>,
    cat_key: Option<Key<T>>,
    value: Option<Value<T>>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    legend_title: Option<String>,
    tooltip: Option<Text>,
}

impl<T> std::fmt::Debug for BoxPlotChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxPlotChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static> BoxPlotChart<T> {
    plot_setters!();

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    /// The category of each row, on a band x axis, in first-seen order.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn x<K: DictionaryKey>(mut self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self {
        self.check_open();
        self.cat_key = Some(erase_key(key));
        self
    }

    /// The value summarised. `None` (NaN) and infinite values are left out.
    pub fn y<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.value = Some(Arc::new(move |r| f(r).to_f64()));
        self
    }

    /// The y scale (default [`Linear`]).
    pub fn y_scale<S: PositionScale>(mut self, scale: S) -> Self {
        self.plot.y(scale);
        self
    }

    /// The padding of the band axis (default [`Band::new`]'s: 0.2 between
    /// bands, 0.1 outside).
    pub fn padding(mut self, p: f64) -> Self {
        self.check_open();
        self.band = self.plot.x(Band::new().padding(p));
        self
    }

    /// Colour each category's marks by a key, through the colour scale, with
    /// a legend. A category takes the key of its first row. The last of
    /// `color` and `fill` wins.
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

    /// The scale for the colours (default [`Categorical::okabe_ito`]).
    pub fn color_scale(mut self, scale: Categorical) -> Self {
        self.check_open();
        self.color_scale = scale;
        self
    }

    /// One colour for every box (default `#0072B2`). The last of `color`
    /// and `fill` wins.
    pub fn fill(mut self, color: Color) -> Self {
        self.check_open();
        self.color = ColorSpec::Fill(color);
        self
    }

    /// Title the colour legend.
    pub fn legend_title(mut self, title: impl Into<String>) -> Self {
        self.check_open();
        self.legend_title = Some(title.into());
        self
    }

    /// The tooltip text of a box, shown on hover.
    pub fn tooltip(mut self, text: impl Fn(&BoxSummary) -> String + Send + Sync + 'static) -> Self {
        self.check_open();
        self.tooltip = Some(Arc::new(text));
        self
    }

    /// The category summaries in first-seen order, each with the key that
    /// colours it.
    fn summaries(&self, rows: &[T]) -> Vec<Boxed> {
        let (Some(xk), Some(y)) = (&self.cat_key, &self.value) else {
            return Vec::new();
        };
        let color_of = match &self.color {
            ColorSpec::Key(k) => Some(k),
            _ => None,
        };
        let mut index: HashMap<Arc<str>, usize> = HashMap::new();
        let mut groups: Vec<(Arc<str>, Arc<str>, Vec<f64>)> = Vec::new();
        for r in rows {
            let key = xk(r);
            let i = *index.entry(Arc::clone(&key)).or_insert_with(|| {
                let color = color_of.map_or_else(|| Arc::clone(&key), |c| c(r));
                groups.push((Arc::clone(&key), color, Vec::new()));
                groups.len() - 1
            });
            groups[i].2.push(y(r));
        }
        groups
            .into_iter()
            .filter_map(|(key, color, values)| {
                let stats = box_stats(values)?;
                Some(Boxed {
                    summary: BoxSummary { key, stats },
                    color,
                })
            })
            .collect()
    }

    /// Build the layers from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        let boxes = self.summaries(&rows);
        drop(rows);
        if boxes.is_empty() {
            return;
        }
        let categories: Vec<Arc<str>> = boxes.iter().map(|b| Arc::clone(&b.summary.key)).collect();
        let colours = ScaleRef::new(self.color_scale.clone());
        let band = self.band.clone();
        // The centre of a band moved by a fraction of its width.
        let at = |fraction: f64| {
            let band = band.clone();
            band.clone().then(Offset::by(Derived::new(move || {
                Px((fraction * band.read().band_width().abs()) as f32)
            })))
        };
        let box_width = {
            let band = band.clone();
            Derived::new(move || Px((BOX_WIDTH * band.read().band_width().abs()) as f32))
        };
        let (half, cap) = (BOX_WIDTH / 2.0, BOX_WIDTH / 4.0);

        let part = |b: &Boxed, y: f64, y_end: f64| Part {
            key: Arc::clone(&b.summary.key),
            color: Arc::clone(&b.color),
            y,
            y_end,
        };
        let whiskers: Vec<Part> = boxes
            .iter()
            .flat_map(|b| {
                let s = &b.summary.stats;
                [part(b, s.q3, s.whisker_hi), part(b, s.q1, s.whisker_lo)]
            })
            .collect();
        let caps: Vec<Part> = boxes
            .iter()
            .flat_map(|b| {
                let s = &b.summary.stats;
                [
                    part(b, s.whisker_hi, s.whisker_hi),
                    part(b, s.whisker_lo, s.whisker_lo),
                ]
            })
            .collect();
        let medians: Vec<Part> = boxes
            .iter()
            .map(|b| part(b, b.summary.stats.median, b.summary.stats.median))
            .collect();
        let outliers: Vec<Part> = boxes
            .iter()
            .flat_map(|b| b.summary.stats.outliers.iter().map(|&o| part(b, o, o)))
            .collect();

        // Boxes.
        {
            let layer = self.plot.rects(boxes);
            layer
                .x_with(
                    band.encode_owned_key(|b: &Boxed| Arc::clone(&b.summary.key))
                        .domain(categories.iter().cloned()),
                )
                .width(box_width)
                .y(|b: &Boxed| b.summary.stats.q3)
                .y_end(|b: &Boxed| b.summary.stats.q1)
                .fill_opacity(Unit(BOX_OPACITY))
                .stroke_width(Px(1.0));
            if let Some(text) = self.tooltip.clone() {
                layer.tooltip(move |b: &Boxed| text(&b.summary));
            }
            match &self.color {
                ColorSpec::Default => {
                    layer.fill(Rect::DEFAULT_FILL).stroke(Rect::DEFAULT_FILL);
                }
                ColorSpec::Fill(c) => {
                    layer.fill(*c).stroke(*c);
                }
                ColorSpec::Key(_) => {
                    layer.fill(colours.encode_owned_key(|b: &Boxed| Arc::clone(&b.color)));
                    layer.stroke(colours.encode_owned_key(|b: &Boxed| Arc::clone(&b.color)));
                    if let Some(title) = &self.legend_title {
                        layer.legend_title(Rect::FILL, title.clone());
                    }
                }
            }
        }

        // Whiskers, caps and medians: segments from the band's centre plus
        // one offset to its centre plus another.
        let spans: [(Vec<Part>, f64, f64, f32); 3] = [
            (whiskers, 0.0, 0.0, 1.0),
            (caps, -cap, cap, 1.0),
            (medians, -half, half, 2.0),
        ];
        for (rows, from, to, width) in spans {
            let layer = self.plot.segments(rows);
            layer
                .x_with(
                    at(from)
                        .encode_owned_key(|p: &Part| Arc::clone(&p.key))
                        .domain(categories.iter().cloned()),
                )
                .x_end_with(
                    at(to)
                        .encode_owned_key(|p: &Part| Arc::clone(&p.key))
                        .domain(categories.iter().cloned()),
                )
                .y(|p: &Part| p.y)
                .y_end(|p: &Part| p.y_end)
                .width(Px(width));
            match &self.color {
                ColorSpec::Default => {
                    layer.color(Segment::DEFAULT_COLOR);
                }
                ColorSpec::Fill(c) => {
                    layer.color(*c);
                }
                ColorSpec::Key(_) => {
                    layer.color(colours.encode_owned_key(|p: &Part| Arc::clone(&p.color)));
                }
            }
        }

        // Outliers.
        if !outliers.is_empty() {
            let layer = self.plot.circles(outliers);
            layer
                .x_with(
                    band.encode_owned_key(|p: &Part| Arc::clone(&p.key))
                        .domain(categories.iter().cloned()),
                )
                .y(|p: &Part| p.y)
                .radius(Px(OUTLIER_RADIUS));
            match &self.color {
                ColorSpec::Default => {
                    layer.fill(Circle::DEFAULT_FILL);
                }
                ColorSpec::Fill(c) => {
                    layer.fill(*c);
                }
                ColorSpec::Key(_) => {
                    layer.fill(colours.encode_owned_key(|p: &Part| Arc::clone(&p.color)));
                }
            }
        }
    }

    /// The inner plot, whose layers are the chart's. Materialises the
    /// chart.
    pub fn plot_mut(&mut self) -> &mut Plot {
        self.materialise();
        &mut self.plot
    }

    /// The finished plot.
    pub fn into_plot(mut self) -> Plot {
        self.materialise();
        self.plot
    }

    /// The summary of the box under `at` (logical pixels), once the chart
    /// has been resolved.
    pub fn pick(&mut self, at: impl Into<Point>) -> Option<&BoxSummary> {
        self.materialise();
        self.plot.pick::<Boxed>(at).map(|b| &b.summary)
    }
}

impl<T: Send + Sync + 'static> Chart for BoxPlotChart<T> {
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
