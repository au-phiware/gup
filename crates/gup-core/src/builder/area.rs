// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The area builder: [`area`] and [`AreaChart`].

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::column::DictionaryKey;
use crate::geom::Point;
use crate::prelude::*;
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

/// A row accessor reading a number (NaN for a null).
type Num<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;

/// A row accessor reading tooltip text.
type Text<T> = Arc<dyn Fn(&T) -> String + Send + Sync>;

/// One point of an area layer built by [`AreaChart`]: where the area's top
/// edge and baseline are at `x`.
///
/// A chart's layers hold these rather than the rows you gave, because a
/// stacked area must have a point at every x the stack uses. `row` is the
/// index of the row the point came from (the first at that x, when rows
/// share an x), or `None` for a point that stacking inserted. Tooltips and
/// [`AreaChart::pick`] speak in your rows through it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaPoint {
    /// The x value.
    pub x: f64,
    /// The top edge.
    pub y: f64,
    /// The baseline.
    pub y_end: f64,
    /// The index of the user's row this point came from, if any.
    pub row: Option<usize>,
}

enum ColorSpec<T> {
    Default,
    Fill(Color),
    Key(Key<T>),
    Stack(Key<T>),
}

/// Start an area chart over `rows`: the region between `y` and a baseline
/// (0 unless [`y_end`](AreaChart::y_end) is set), or one area per key with
/// [`color`](AreaChart::color), or stacked with [`stack`](AreaChart::stack).
///
/// An area is drawn through its rows in data order, so sort them by x
/// first (a stack sorts its own). A null in x or y breaks the area.
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// let rain: Vec<(f64, f64)> = (0..12).map(|m| (f64::from(m), 20.0 + f64::from(m % 5) * 6.0)).collect();
///
/// let mut chart = gup_core::area(rain).x(|r| r.0).y(|r| r.1);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// One area per key, overlapping at half opacity:
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Visit { day: f64, source: &'static str, count: f64 }
/// let visits = (0..10).flat_map(|d| {
///     [("web", 3.0), ("app", 2.0)].map(|(source, k)| Visit {
///         day: f64::from(d),
///         source,
///         count: k * (4.0 + f64::from(d % 4)),
///     })
/// });
///
/// let mut chart = gup_core::area(visits)
///     .x(|v| v.day)
///     .y(|v| v.count)
///     .color(|v| v.source);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// Stacked, with an outline along each layer's top:
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Visit { day: f64, source: String, count: f64 }
/// let visits = (0..10).flat_map(|d| {
///     ["web", "app", "api"].map(|source| Visit {
///         day: f64::from(d),
///         source: source.to_string(),
///         count: 2.0 + f64::from(d % 3),
///     })
/// });
///
/// let mut chart = gup_core::area(visits)
///     .x(|v| v.day)
///     .y(|v| v.count)
///     .stack(|v| v.source.clone())
///     .outline(1.0);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// See the [module docs](crate::builder) for the pattern.
pub fn area<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> AreaChart<T> {
    let mut plot = Plot::new();
    plot.x(Linear::new());
    plot.y(Linear::new());
    AreaChart {
        plot,
        rows: Some(rows.into_iter().collect()),
        data: None,
        x: None,
        y: None,
        y_end: None,
        color: ColorSpec::Default,
        color_scale: Categorical::okabe_ito(),
        opacity: None,
        outline: None,
        legend_title: None,
        tooltip: None,
    }
}

/// An area chart under construction, and a [`Chart`].
///
/// Its layers are [`Selection<AreaPoint, Area>`](Selection), not layers over
/// your rows: see [`AreaPoint`]. `stack` ignores
/// [`y_end`](Self::y_end), and the last of `color`, `stack` and `fill` wins.
/// There is no appender; a live area uses a [`Plot`] with [`Plot::area`] and
/// [`Selection::appender`].
///
/// # Panics
///
/// The layer setters (`x`, `y`, `y_end`, `color*`, `stack`, `fill`,
/// `opacity`, `outline`, `tooltip`, `legend_title`) panic once the chart has
/// been drawn, with "set layer encodings before the chart is first drawn;
/// change a drawn chart through layer_mut()". The plot-level setters work at
/// any time.
#[must_use]
pub struct AreaChart<T> {
    plot: Plot,
    rows: Option<Vec<T>>,
    /// The rows, once materialised; [`AreaPoint::row`] indexes them.
    data: Option<Arc<[T]>>,
    x: Option<Num<T>>,
    y: Option<Num<T>>,
    y_end: Option<Num<T>>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    opacity: Option<f32>,
    outline: Option<f32>,
    legend_title: Option<String>,
    tooltip: Option<Text<T>>,
}

impl<T> std::fmt::Debug for AreaChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AreaChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

/// Stack `series` (each `(x, value, row)` triples) into area points.
///
/// Every output series has one point per distinct x in the union of all
/// series' x values, ascending. At an x, positive values stack upward from 0
/// and negative values downward, over the series in order, so a layer's base
/// is the sum below it and the layer above starts exactly at its top. A
/// series missing an x, or with a null value there, contributes 0; rows that
/// share an x within a series sum. A point's `row` is the first row of its
/// series at that x, or `None` where the series has none. Rows with a null x
/// are dropped.
fn stack_areas(series: &[Vec<(f64, f64, usize)>]) -> Vec<Vec<AreaPoint>> {
    // -0.0 and 0.0 are one x.
    fn bits(x: f64) -> u64 {
        if x == 0.0 {
            0.0_f64.to_bits()
        } else {
            x.to_bits()
        }
    }
    let mut xs: Vec<f64> = series
        .iter()
        .flatten()
        .filter(|p| !p.0.is_nan())
        .map(|p| if p.0 == 0.0 { 0.0 } else { p.0 })
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by_key(|x| bits(*x));
    let slot: HashMap<u64, usize> = xs.iter().enumerate().map(|(i, &x)| (bits(x), i)).collect();

    let mut up = vec![0.0_f64; xs.len()];
    let mut down = vec![0.0_f64; xs.len()];
    series
        .iter()
        .map(|rows| {
            let mut sums = vec![0.0_f64; xs.len()];
            let mut first: Vec<Option<usize>> = vec![None; xs.len()];
            for &(x, v, row) in rows {
                if x.is_nan() {
                    continue;
                }
                let i = slot[&bits(x)];
                if v.is_finite() {
                    sums[i] += v;
                }
                first[i].get_or_insert(row);
            }
            xs.iter()
                .enumerate()
                .map(|(i, &x)| {
                    let v = sums[i];
                    let edge = if v >= 0.0 { &mut up[i] } else { &mut down[i] };
                    let base = *edge;
                    *edge += v;
                    AreaPoint {
                        x,
                        y: *edge,
                        y_end: base,
                        row: first[i],
                    }
                })
                .collect()
        })
        .collect()
}

impl<T: Send + Sync + 'static> AreaChart<T> {
    plot_setters!();

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    fn number<D: ColumnValue>(f: impl Fn(&T) -> D + Send + Sync + 'static) -> Num<T> {
        Arc::new(move |r| f(r).to_f64())
    }

    /// The x position of a row, through the x scale. `None` breaks the area.
    pub fn x<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.x = Some(Self::number(f));
        self
    }

    /// The top edge of a row, through the y scale. `None` breaks the area
    /// (counts as 0 in a stack).
    pub fn y<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.y = Some(Self::number(f));
        self
    }

    /// The baseline of a row (default 0), for a band between two values:
    /// `.y(|r| r.high).y_end(|r| r.low)`. Ignored by [`stack`](Self::stack).
    pub fn y_end<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.y_end = Some(Self::number(f));
        self
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

    /// One area per key, in first-seen order, coloured by key and drawn at
    /// half opacity so overlapping areas show each other (see
    /// [`opacity`](Self::opacity)). The last of `color`, `stack` and `fill`
    /// wins.
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

    /// One area per key, in first-seen order from the bottom, stacked: at
    /// every x any series has, a layer's base is the sum of the layers below
    /// it (positive values upward from 0, negative downward). A series
    /// missing an x, or with a null there, contributes 0, and rows of a
    /// series that share an x sum. Rows are sorted by x. Coloured by key; the
    /// last of `color`, `stack` and `fill` wins. Ignores
    /// [`y_end`](Self::y_end).
    pub fn stack<K: DictionaryKey>(
        mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.color = ColorSpec::Stack(erase_key(key));
        self
    }

    /// The scale for [`color`](Self::color) and [`stack`](Self::stack)
    /// (default [`Categorical::okabe_ito`]).
    pub fn color_scale(mut self, scale: Categorical) -> Self {
        self.check_open();
        self.color_scale = scale;
        self
    }

    /// One fill colour for every area. The last of `color`, `stack` and
    /// `fill` wins.
    pub fn fill(mut self, color: Color) -> Self {
        self.check_open();
        self.color = ColorSpec::Fill(color);
        self
    }

    /// The fill opacity (default 1, or 0.5 for [`color`](Self::color)).
    ///
    /// # Panics
    ///
    /// If `opacity` is outside `0..=1`.
    pub fn opacity(mut self, opacity: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&opacity),
            "opacity must be within 0..=1, not {opacity}"
        );
        self.check_open();
        self.opacity = Some(opacity);
        self
    }

    /// Draw a line `px` logical pixels wide along each area's top edge (a
    /// stack's layer top), in the area's colour at full opacity (default no
    /// outline).
    pub fn outline(mut self, px: f32) -> Self {
        self.check_open();
        self.outline = Some(px);
        self
    }

    /// Title the colour legend.
    pub fn legend_title(mut self, title: impl Into<String>) -> Self {
        self.check_open();
        self.legend_title = Some(title.into());
        self
    }

    /// The tooltip text of a row, shown for the row nearest the pointer. A
    /// point stacking inserted has no row and shows nothing.
    pub fn tooltip(mut self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self {
        self.check_open();
        self.tooltip = Some(Arc::new(text));
        self
    }

    /// Row indices by key, in first-seen key order, with each key's label.
    fn split(rows: &[T], key: &Key<T>) -> Vec<(Arc<str>, Vec<usize>)> {
        let mut index: HashMap<Arc<str>, usize> = HashMap::new();
        let mut series: Vec<(Arc<str>, Vec<usize>)> = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let k = key(r);
            let slot = *index.entry(Arc::clone(&k)).or_insert_with(|| {
                series.push((k, Vec::new()));
                series.len() - 1
            });
            series[slot].1.push(i);
        }
        series
    }

    /// Build the layers from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        let rows: Arc<[T]> = Arc::from(rows);
        self.data = Some(Arc::clone(&rows));
        let nan: Num<T> = Arc::new(|_| f64::NAN);
        let xf = self.x.clone().unwrap_or_else(|| Arc::clone(&nan));
        let yf = self.y.clone().unwrap_or(nan);
        let groups: Vec<(Option<Arc<str>>, Vec<usize>)> = match &self.color {
            ColorSpec::Key(key) | ColorSpec::Stack(key) => Self::split(&rows, key)
                .into_iter()
                .map(|(k, v)| (Some(k), v))
                .collect(),
            _ => vec![(None, (0..rows.len()).collect())],
        };
        let points: Vec<Vec<AreaPoint>> = if matches!(self.color, ColorSpec::Stack(_)) {
            let series: Vec<Vec<(f64, f64, usize)>> = groups
                .iter()
                .map(|(_, ix)| {
                    ix.iter()
                        .map(|&i| (xf(&rows[i]), yf(&rows[i]), i))
                        .collect()
                })
                .collect();
            stack_areas(&series)
        } else {
            groups
                .iter()
                .map(|(_, ix)| {
                    ix.iter()
                        .map(|&i| AreaPoint {
                            x: xf(&rows[i]),
                            y: yf(&rows[i]),
                            y_end: self.y_end.as_ref().map_or(0.0, |f| f(&rows[i])),
                            row: Some(i),
                        })
                        .collect()
                })
                .collect()
        };

        let opacity = self
            .opacity
            .or_else(|| matches!(self.color, ColorSpec::Key(_)).then_some(0.5));
        let scale = ScaleRef::new(self.color_scale.clone());
        let (has_x, has_y) = (self.x.is_some(), self.y.is_some());
        let layers: Vec<_> = groups.into_iter().map(|g| g.0).zip(points).collect();
        for (name, pts) in &layers {
            let sel = self.plot.area(pts.clone());
            if has_x {
                sel.x(|p: &AreaPoint| p.x);
            }
            if has_y {
                sel.y(|p: &AreaPoint| p.y);
                sel.y_end(|p: &AreaPoint| p.y_end);
            }
            if let Some(o) = opacity {
                sel.fill_opacity(Unit(o));
            }
            match (&self.color, name) {
                (ColorSpec::Fill(c), _) => {
                    sel.fill(*c);
                }
                (_, Some(name)) => {
                    let name = Arc::clone(name);
                    sel.fill(scale.encode_owned_key(move |_: &AreaPoint| Arc::clone(&name)));
                    if let Some(title) = &self.legend_title {
                        sel.legend_title(Area::FILL, title.clone());
                    }
                }
                _ => {}
            }
            if let Some(text) = &self.tooltip {
                let (text, rows) = (Arc::clone(text), Arc::clone(&rows));
                sel.tooltip(move |p: &AreaPoint| {
                    p.row.map_or_else(String::new, |i| text(&rows[i]))
                });
            }
        }
        if let Some(px) = self.outline {
            for (name, pts) in layers {
                let sel = self.plot.line(pts);
                if has_x {
                    sel.x(|p: &AreaPoint| p.x);
                }
                if has_y {
                    sel.y(|p: &AreaPoint| p.y);
                }
                sel.width(Px(px));
                match (&self.color, name) {
                    (ColorSpec::Fill(c), _) => {
                        sel.color(*c);
                    }
                    (_, Some(name)) => {
                        sel.color(scale.encode_owned_key(move |_: &AreaPoint| Arc::clone(&name)));
                    }
                    _ => {}
                }
                if let Some(text) = &self.tooltip {
                    let (text, rows) = (Arc::clone(text), Arc::clone(&rows));
                    sel.tooltip(move |p: &AreaPoint| {
                        p.row.map_or_else(String::new, |i| text(&rows[i]))
                    });
                }
            }
        }
    }

    /// The area layers, one per key in first-seen order (one without
    /// [`color`](Self::color) or [`stack`](Self::stack)), for settings the
    /// builder has no method for. Their rows are [`AreaPoint`]s. Materialises
    /// the chart: layer setters panic after.
    pub fn layers_mut(&mut self) -> Vec<&mut Selection<AreaPoint, Area>> {
        self.materialise();
        self.plot.layers_mut::<AreaPoint, Area>().collect()
    }

    /// The inner plot. Materialises the chart.
    pub fn plot_mut(&mut self) -> &mut Plot {
        self.materialise();
        &mut self.plot
    }

    /// The finished plot, whose layers hold [`AreaPoint`]s.
    pub fn into_plot(mut self) -> Plot {
        self.materialise();
        self.plot
    }

    /// The row nearest the pointer on the area under `at` (logical pixels),
    /// once the chart has been resolved; `None` over a point stacking
    /// inserted. See [`Area`] for which row an area picks.
    pub fn pick(&mut self, at: Point) -> Option<&T> {
        self.materialise();
        let row = self.plot.pick::<AreaPoint>(at)?.row?;
        self.data.as_ref()?.get(row)
    }
}

impl<T: Send + Sync + 'static> Chart for AreaChart<T> {
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
    fn set_waker(&mut self, waker: Waker) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(p: &[AreaPoint]) -> Vec<(f64, f64, f64, Option<usize>)> {
        p.iter().map(|p| (p.x, p.y, p.y_end, p.row)).collect()
    }

    #[test]
    fn stacks_over_the_union_of_x() {
        // a has x 0, 1, 2; b has x 1, 3.
        let a = vec![(0.0, 1.0, 0), (1.0, 2.0, 1), (2.0, 3.0, 2)];
        let b = vec![(3.0, 4.0, 3), (1.0, 5.0, 4)];
        let s = stack_areas(&[a, b]);
        assert_eq!(
            pts(&s[0]),
            [
                (0.0, 1.0, 0.0, Some(0)),
                (1.0, 2.0, 0.0, Some(1)),
                (2.0, 3.0, 0.0, Some(2)),
                (3.0, 0.0, 0.0, None),
            ]
        );
        assert_eq!(
            pts(&s[1]),
            [
                (0.0, 1.0, 1.0, None),
                (1.0, 7.0, 2.0, Some(4)),
                (2.0, 3.0, 3.0, None),
                (3.0, 4.0, 0.0, Some(3)),
            ]
        );
    }

    #[test]
    fn nulls_count_as_zero_and_duplicates_sum() {
        let a = vec![(0.0, f64::NAN, 0), (1.0, 2.0, 1), (1.0, 3.0, 2)];
        let b = vec![(0.0, 4.0, 3), (1.0, 1.0, 4)];
        let s = stack_areas(&[a, b]);
        assert_eq!(
            pts(&s[0]),
            [(0.0, 0.0, 0.0, Some(0)), (1.0, 5.0, 0.0, Some(1))]
        );
        assert_eq!(
            pts(&s[1]),
            [(0.0, 4.0, 0.0, Some(3)), (1.0, 6.0, 5.0, Some(4))]
        );
    }

    #[test]
    fn negatives_stack_downward_from_zero() {
        let s = stack_areas(&[
            vec![(0.0, 2.0, 0)],
            vec![(0.0, -1.0, 1)],
            vec![(0.0, 3.0, 2)],
            vec![(0.0, -2.0, 3)],
        ]);
        let edges: Vec<(f64, f64)> = s.iter().map(|l| (l[0].y_end, l[0].y)).collect();
        assert_eq!(edges, [(0.0, 2.0), (0.0, -1.0), (2.0, 5.0), (-1.0, -3.0)]);
    }

    #[test]
    fn negative_zero_is_zero_and_null_x_is_dropped() {
        let s = stack_areas(&[
            vec![(-0.0, 1.0, 0), (f64::NAN, 9.0, 1)],
            vec![(0.0, 2.0, 2)],
        ]);
        assert_eq!(s[0].len(), 1);
        assert_eq!(s[1][0].y_end, 1.0);
        assert_eq!(s[1][0].y, 3.0);
    }
}
