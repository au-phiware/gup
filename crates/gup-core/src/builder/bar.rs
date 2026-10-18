// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bar builder: [`bar`] and [`BarChart`].

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use super::{DRAWN, plot_setters};
use crate::geom::Point;
use crate::prelude::*;
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Key<T> = Arc<dyn for<'a> Fn(&'a T) -> &'a str + Send + Sync>;
type Value<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;
type Op<T> = Box<dyn Fn(&mut Selection<T, Rect>) + Send + Sync>;

/// The padding between bars within a group: a tenth of a slot.
const SLOT_PADDING: f64 = 0.1;

/// Fix a closure's signature to the higher-ranked key form.
fn key_fn<T, F: for<'a> Fn(&'a T) -> &'a str>(f: F) -> F {
    f
}

enum ColorSpec<T> {
    Default,
    Fill(Color),
    Key(Key<T>),
}

enum Arrange<T> {
    Simple,
    Group(Key<T>),
    Stack(Key<T>),
}

/// Start a bar chart over `rows`: one bar per row from 0 to its value, on a
/// band axis of categories. Add [`group`](BarChart::group) for bars side by
/// side, or [`stack`](BarChart::stack) for bars on top of each other.
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Sale { quarter: &'static str, region: &'static str, total: f64 }
/// let rows = ["Q1", "Q2", "Q3"].into_iter().flat_map(|q| {
///     [("North", 3.0), ("South", 5.0)].map(|(region, total)| Sale { quarter: q, region, total })
/// });
///
/// let mut chart = gup_core::bar(rows)
///     .x(|r| r.quarter)
///     .y(|r| r.total)
///     .stack(|r| r.region);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// See the [module docs](crate::builder) for the pattern.
pub fn bar<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> BarChart<T> {
    let mut plot = Plot::new();
    let x = plot.x(Band::new());
    plot.y(Linear::new());
    BarChart {
        plot,
        x,
        rows: Some(rows.into_iter().collect()),
        ops: Vec::new(),
        x_key: None,
        y_value: None,
        arrange: Arrange::Simple,
        color: ColorSpec::Default,
        color_scale: Categorical::okabe_ito(),
        legend_title: None,
    }
}

/// A bar chart under construction, and a [`Chart`].
///
/// # Panics
///
/// The layer setters (`x`, `y`, `padding`, `color*`, `fill`, `group`,
/// `stack`, `opacity`, `tooltip`, `legend_title`) panic once the chart has
/// been drawn, with "set layer encodings before the chart is first drawn;
/// change a drawn chart through layer_mut()". The plot-level setters work at
/// any time.
#[must_use]
pub struct BarChart<T> {
    plot: Plot,
    x: ScaleRef<Band>,
    rows: Option<Vec<T>>,
    ops: Vec<Op<T>>,
    x_key: Option<Key<T>>,
    y_value: Option<Value<T>>,
    arrange: Arrange<T>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    legend_title: Option<String>,
}

impl<T> std::fmt::Debug for BarChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BarChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

/// Per layer and category, the stacked `(base, top)`: positive sums stack
/// upward from 0 and negative sums downward from 0, over the layers in
/// order. Each input table holds one layer's summed values by category.
fn stack_tables(sums: &[HashMap<Arc<str>, f64>]) -> Vec<HashMap<Arc<str>, (f64, f64)>> {
    let mut up: HashMap<Arc<str>, f64> = HashMap::new();
    let mut down: HashMap<Arc<str>, f64> = HashMap::new();
    sums.iter()
        .map(|layer| {
            layer
                .iter()
                .map(|(key, &sum)| {
                    let edge = if sum >= 0.0 { &mut up } else { &mut down };
                    let at = edge.entry(Arc::clone(key)).or_insert(0.0);
                    let base = *at;
                    *at += sum;
                    (Arc::clone(key), (base, *at))
                })
                .collect()
        })
        .collect()
}

impl<T: Send + Sync + 'static> BarChart<T> {
    plot_setters!();

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    fn op(mut self, op: impl Fn(&mut Selection<T, Rect>) + Send + Sync + 'static) -> Self {
        self.check_open();
        self.ops.push(Box::new(op));
        self
    }

    /// The category of each bar, on a band x axis, in first-seen order.
    pub fn x(mut self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self {
        self.check_open();
        self.x_key = Some(Arc::new(key));
        self
    }

    /// The value of each bar, drawn from 0. `None` (NaN) is not drawn.
    pub fn y<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.y_value = Some(Arc::new(move |r| f(r).to_f64()));
        self
    }

    /// The y scale (default [`Linear`]). Bars on a log scale are not
    /// supported.
    pub fn y_scale<S: PositionScale>(mut self, scale: S) -> Self {
        self.plot.y(scale);
        self
    }

    /// The padding of the band axis (default [`Band::new`]'s: 0.2 between
    /// bands, 0.1 outside).
    pub fn padding(mut self, p: f64) -> Self {
        self.check_open();
        self.x = self.plot.x(Band::new().padding(p));
        self
    }

    /// Colour by a category key per row, through the colour scale. The last
    /// of `color` and `fill` wins; either replaces the colouring by group
    /// or stack key.
    pub fn color(mut self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self {
        self.check_open();
        self.color = ColorSpec::Key(Arc::new(key));
        self
    }

    /// The scale for the colours (default [`Categorical::okabe_ito`]).
    pub fn color_scale(mut self, scale: Categorical) -> Self {
        self.check_open();
        self.color_scale = scale;
        self
    }

    /// One fill colour for every bar. The last of `color` and `fill` wins.
    pub fn fill(mut self, color: Color) -> Self {
        self.check_open();
        self.color = ColorSpec::Fill(color);
        self
    }

    /// Bars side by side within each band, one slot per key (first-seen
    /// order), coloured by key. The last of `group` and `stack` wins.
    pub fn group(mut self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self {
        self.check_open();
        self.arrange = Arrange::Group(Arc::new(key));
        self
    }

    /// Bars stacked within each band, bottom to top in first-seen key
    /// order, coloured by key. Positive values stack upward from 0 and
    /// negative values downward from 0. Rows sharing a category and a key
    /// draw as one bar of their summed height. The last of `group` and
    /// `stack` wins.
    pub fn stack(mut self, key: impl for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static) -> Self {
        self.check_open();
        self.arrange = Arrange::Stack(Arc::new(key));
        self
    }

    /// The fill opacity.
    ///
    /// # Panics
    ///
    /// If `opacity` is outside `0..=1`.
    pub fn opacity(self, opacity: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&opacity),
            "opacity must be within 0..=1, not {opacity}"
        );
        self.op(move |s| {
            s.fill_opacity(Unit(opacity));
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
        let mut parts: Vec<Vec<T>> = (0..index.len()).map(|_| Vec::new()).collect();
        for (row, slot) in rows.into_iter().zip(slots) {
            parts[slot].push(row);
        }
        parts
    }

    /// Build the layers from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        // Every layer shares one band domain, so a layer missing a category
        // does not reorder the axis.
        let mut categories: Vec<Arc<str>> = Vec::new();
        if let Some(xk) = &self.x_key {
            let mut seen = std::collections::HashSet::new();
            for r in &rows {
                let k = xk(r);
                if seen.insert(k.to_owned()) {
                    categories.push(k.into());
                }
            }
        }
        let (part_key, stacked) = match &self.arrange {
            Arrange::Simple => (None, false),
            Arrange::Group(k) => (Some(Arc::clone(k)), false),
            Arrange::Stack(k) => (Some(Arc::clone(k)), true),
        };
        let parts = Self::split(rows, part_key.as_ref());
        let count = parts.len().max(1) as u32;

        let tables = if stacked {
            let sums: Vec<HashMap<Arc<str>, f64>> = parts
                .iter()
                .map(|part| {
                    let mut sum: HashMap<Arc<str>, f64> = HashMap::new();
                    if let (Some(xk), Some(y)) = (&self.x_key, &self.y_value) {
                        for r in part {
                            let v = y(r);
                            if v.is_finite() {
                                *sum.entry(xk(r).into()).or_insert(0.0) += v;
                            }
                        }
                    }
                    sum
                })
                .collect();
            Some(stack_tables(&sums))
        } else {
            None
        };

        let colours = ScaleRef::new(self.color_scale.clone());
        for (i, part) in parts.into_iter().enumerate() {
            let sel = self.plot.rects(part);
            if let Some(xk) = &self.x_key {
                let xk = Arc::clone(xk);
                let enc = |s: &mut Selection<T, Rect>| {
                    let xk = Arc::clone(&xk);
                    let key = key_fn(move |r: &T| xk(r));
                    match (&self.arrange, i as u32) {
                        (Arrange::Group(_), i) => {
                            s.x_with(
                                self.x
                                    .sub_band(i, count)
                                    .encode_key(key)
                                    .domain(categories.iter().cloned()),
                            )
                            .width(self.x.sub_band_width_px(count, SLOT_PADDING));
                        }
                        _ => {
                            s.x_with(self.x.encode_key(key).domain(categories.iter().cloned()))
                                .width(self.x.band_width_px());
                        }
                    }
                };
                enc(sel);
            }
            if let Some(y) = &self.y_value {
                if let Some(tables) = &tables {
                    let table = Arc::new(tables[i].clone());
                    let (top, base) = (Arc::clone(&table), table);
                    let (yt, yb) = (Arc::clone(y), Arc::clone(y));
                    let (xt, xb) = (self.x_key.clone(), self.x_key.clone());
                    sel.y(move |r: &T| match &xt {
                        Some(xk) if yt(r).is_finite() => {
                            top.get(xk(r)).map_or(f64::NAN, |&(_, t)| t)
                        }
                        _ => f64::NAN,
                    });
                    sel.y_end(move |r: &T| match &xb {
                        Some(xk) if yb(r).is_finite() => {
                            base.get(xk(r)).map_or(f64::NAN, |&(b, _)| b)
                        }
                        _ => f64::NAN,
                    });
                } else {
                    let y = Arc::clone(y);
                    sel.y(move |r: &T| y(r));
                    sel.y_end(|_: &T| 0.0f64);
                }
            }
            for op in &self.ops {
                op(sel);
            }
            let key = match (&self.color, &part_key) {
                (ColorSpec::Fill(c), _) => {
                    sel.fill(*c);
                    None
                }
                (ColorSpec::Key(k), _) => Some(Arc::clone(k)),
                (ColorSpec::Default, k) => k.clone(),
            };
            if let Some(key) = key {
                sel.fill(colours.encode_key(key_fn(move |r: &T| key(r))));
                if let Some(title) = &self.legend_title {
                    sel.legend_title(Rect::FILL, title.clone());
                }
            }
        }
        self.ops.clear();
    }

    /// The bar layers, one per group or stack key in first-seen order (one
    /// for simple bars), for settings the builder has no method for.
    /// Materialises the chart: layer setters panic after.
    pub fn layers_mut(&mut self) -> Vec<&mut Selection<T, Rect>> {
        self.materialise();
        self.plot.layers_mut::<T, Rect>().collect()
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

    /// The row of the bar under `at` (logical pixels), once the chart has
    /// been resolved.
    pub fn pick(&mut self, at: Point) -> Option<&T> {
        self.materialise();
        self.plot.pick::<T>(at)
    }
}

impl<T: Send + Sync + 'static> Chart for BarChart<T> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(pairs: &[(&str, f64)]) -> HashMap<Arc<str>, f64> {
        pairs.iter().map(|&(k, v)| (k.into(), v)).collect()
    }

    #[test]
    fn positives_stack_up_and_negatives_down_separately() {
        let t = stack_tables(&[
            layer(&[("Q1", 3.0), ("Q2", -2.0)]),
            layer(&[("Q1", -1.0), ("Q2", 4.0)]),
            layer(&[("Q1", 2.0), ("Q2", -1.0)]),
        ]);
        assert_eq!(t[0]["Q1"], (0.0, 3.0));
        assert_eq!(t[1]["Q1"], (0.0, -1.0));
        assert_eq!(t[2]["Q1"], (3.0, 5.0));
        assert_eq!(t[0]["Q2"], (0.0, -2.0));
        assert_eq!(t[1]["Q2"], (0.0, 4.0));
        assert_eq!(t[2]["Q2"], (-2.0, -3.0));
    }

    #[test]
    fn a_missing_category_leaves_the_stack_unchanged() {
        let t = stack_tables(&[
            layer(&[("A", 1.0)]),
            layer(&[("B", 2.0)]),
            layer(&[("A", 1.0)]),
        ]);
        assert!(!t[1].contains_key("A"));
        assert_eq!(t[2]["A"], (1.0, 2.0));
        assert_eq!(t[1]["B"], (0.0, 2.0));
    }
}
