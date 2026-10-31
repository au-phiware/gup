// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bar builder: [`bar`] and [`BarChart`].

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::column::DictionaryKey;
use crate::encoding::{KeyEncoded, LabelKey};
use crate::geom::Point;
use crate::prelude::*;
use crate::scene::{HAlign, VAlign};
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Value<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;
type Op<T> = Box<dyn Fn(&mut Selection<T, Rect>) + Send + Sync>;

mod sealed {
    pub trait Sealed {}
}

/// Which way a [`BarChart`]'s bars run: [`Vertical`] (from [`bar`]) or
/// [`Horizontal`] (from [`barh`]). Sealed: only those two exist.
pub trait Orientation: sealed::Sealed + Send + Sync + 'static {
    /// Whether bars run rightward from a band y axis.
    #[doc(hidden)]
    const HORIZONTAL: bool;
}

/// Bars standing on a band x axis: the orientation of [`bar`].
#[derive(Debug, Clone, Copy)]
pub struct Vertical;

/// Bars running rightward from a band y axis: the orientation of [`barh`].
#[derive(Debug, Clone, Copy)]
pub struct Horizontal;

impl sealed::Sealed for Vertical {}
impl sealed::Sealed for Horizontal {}
impl Orientation for Vertical {
    const HORIZONTAL: bool = false;
}
impl Orientation for Horizontal {
    const HORIZONTAL: bool = true;
}

/// The default gap between the bars of a group: a tenth of a slot.
const DEFAULT_GROUP_PADDING: f64 = 0.1;

enum ColorSpec<T> {
    Default,
    Fill(Color),
    Key(Key<T>),
}

/// Formats a bar's value as label text.
type Format = Arc<dyn Fn(f64) -> String + Send + Sync>;

/// Whether, and how, bars print their values (GUP-544).
enum LabelSpec {
    Off,
    /// The value axis's tick format, else [`default_label`].
    Default,
    With(Format),
}

/// One bar's (or stacked segment's) label: its category, its summed
/// value, the data span it covers and, for a stack, its stack key.
struct BarLabel {
    key: Arc<str>,
    value: f64,
    base: f64,
    top: f64,
    stack: Option<Arc<str>>,
}

/// The default value label: two decimals at most, trailing zeros and
/// point trimmed, the minus written as U+2212; no thousands separators.
fn default_label(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    if s == "-0" {
        return "0".into();
    }
    s.replace('-', &crate::scale::MINUS.to_string())
}

/// The relative luminance of a colour (WCAG 2).
fn luminance(c: Color) -> f64 {
    let lin = |v: f32| {
        let v = f64::from(v);
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// White or `#111111`, whichever contrasts more (WCAG 2 ratio) with `fill`.
fn contrast_text(fill: Color) -> Color {
    let dark = Color::hex(0x11_11_11);
    let ratio = |a: Color, b: Color| {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    };
    if ratio(fill, Color::WHITE) >= ratio(fill, dark) {
        Color::WHITE
    } else {
        dark
    }
}

/// What fills a stack's segments, which decides its label colour.
enum Fill {
    /// The colour scale, by stack key.
    Scale(ScaleRef<Categorical>),
    Const(Color),
    /// Overridden by `.color(key)`: the theme's label colour.
    Theme,
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
    BarChart::new(rows)
}

/// Start a horizontal bar chart over `rows`: one bar per row from 0 to its
/// value, with the categories down a band y axis (the first row's category
/// at the top) and the value along x. The setters are [`bar`]'s, with the
/// axes swapped: [`y`](BarChart::y) is the category and
/// [`x`](BarChart::x) the value.
///
/// ```
/// use gup_core::{ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Answer { text: &'static str, share: f64 }
/// let rows = [
///     Answer { text: "Strongly agree", share: 41.0 },
///     Answer { text: "Agree", share: 33.0 },
///     Answer { text: "Disagree", share: 12.0 },
/// ];
///
/// let mut chart = gup_core::barh(rows)
///     .y(|r| r.text)
///     .x(|r| r.share);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// On a horizontal chart `x` is the value, so a category accessor does not
/// compile there:
///
/// ```compile_fail
/// struct Answer { text: String, share: f64 }
/// let rows: Vec<Answer> = Vec::new();
/// let _ = gup_core::barh(rows).x(|r| r.text.as_str());
/// ```
#[doc(alias = "horizontal")]
pub fn barh<T: Send + Sync + 'static>(
    rows: impl IntoIterator<Item = T>,
) -> BarChart<T, Horizontal> {
    BarChart::new(rows)
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
pub struct BarChart<T, O: Orientation = Vertical> {
    plot: Plot,
    band: ScaleRef<Band>,
    rows: Option<Vec<T>>,
    ops: Vec<Op<T>>,
    cat_key: Option<Key<T>>,
    value: Option<Value<T>>,
    arrange: Arrange<T>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    legend_title: Option<String>,
    group_padding: f64,
    labels: LabelSpec,
    orientation: PhantomData<O>,
}

impl<T, O: Orientation> std::fmt::Debug for BarChart<T, O> {
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

impl<T: Send + Sync + 'static> BarChart<T, Vertical> {
    /// The category of each bar, on a band x axis, in first-seen order.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn x<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self {
        self.set_category(key)
    }

    /// The value of each bar, drawn from 0. `None` (NaN) is not drawn.
    pub fn y<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.set_value(f)
    }

    /// The y scale (default [`Linear`]). Bars on a log scale are not
    /// supported.
    pub fn y_scale<S: PositionScale>(self, scale: S) -> Self {
        self.set_value_scale(scale)
    }

    /// Label the value axis ticks with `f` instead of the default labels;
    /// `f` receives the tick value and returns its label. The category axis
    /// is labelled by its keys.
    pub fn y_tick_format(mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        self.plot.y_tick_format(f);
        self
    }
}

impl<T: Send + Sync + 'static> BarChart<T, Horizontal> {
    /// The category of each bar, on a band y axis, in first-seen order from
    /// the top.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn y<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self {
        self.set_category(key)
    }

    /// The value of each bar, drawn from 0. `None` (NaN) is not drawn.
    pub fn x<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.set_value(f)
    }

    /// The x scale (default [`Linear`]). Bars on a log scale are not
    /// supported.
    pub fn x_scale<S: PositionScale>(self, scale: S) -> Self {
        self.set_value_scale(scale)
    }

    /// Label the value axis ticks with `f` instead of the default labels;
    /// `f` receives the tick value and returns its label. The category axis
    /// is labelled by its keys.
    pub fn x_tick_format(mut self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        self.plot.x_tick_format(f);
        self
    }
}

impl<T: Send + Sync + 'static, O: Orientation> BarChart<T, O> {
    fn new(rows: impl IntoIterator<Item = T>) -> Self {
        let mut plot = Plot::new();
        let band = if O::HORIZONTAL {
            plot.x(Linear::new());
            plot.round_to_ticks(0);
            plot.y(Band::new())
        } else {
            plot.y(Linear::new());
            plot.round_to_ticks(1);
            plot.x(Band::new())
        };
        BarChart {
            plot,
            band,
            rows: Some(rows.into_iter().collect()),
            ops: Vec::new(),
            cat_key: None,
            value: None,
            arrange: Arrange::Simple,
            color: ColorSpec::Default,
            color_scale: Categorical::okabe_ito(),
            legend_title: None,
            group_padding: DEFAULT_GROUP_PADDING,
            labels: LabelSpec::Off,
            orientation: PhantomData,
        }
    }

    plot_setters!(titles);

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    fn op(mut self, op: impl Fn(&mut Selection<T, Rect>) + Send + Sync + 'static) -> Self {
        self.check_open();
        self.ops.push(Box::new(op));
        self
    }

    fn set_category<K: DictionaryKey>(
        mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.cat_key = Some(erase_key(key));
        self
    }

    fn set_value<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.value = Some(Arc::new(move |r| f(r).to_f64()));
        self
    }

    fn set_value_scale<S: PositionScale>(mut self, scale: S) -> Self {
        if O::HORIZONTAL {
            self.plot.x(scale);
        } else {
            self.plot.y(scale);
        }
        self
    }

    /// The padding of the band axis (default [`Band::new`]'s: 0.2 between
    /// bands, 0.1 outside).
    pub fn padding(mut self, p: f64) -> Self {
        self.check_open();
        let band = Band::new().padding(p);
        self.band = if O::HORIZONTAL {
            self.plot.y(band)
        } else {
            self.plot.x(band)
        };
        self
    }

    /// The gap between the bars of a [`group`](Self::group), as a fraction of
    /// a slot (default 0.1). Zero makes neighbouring bars touch.
    ///
    /// ```
    /// let chart = gup_core::bar([("Q1", "North", 3.0), ("Q1", "South", 5.0)])
    ///     .x(|r| r.0)
    ///     .y(|r| r.2)
    ///     .group(|r| r.1)
    ///     .group_padding(0.0);
    /// # let _ = chart;
    /// ```
    ///
    /// # Panics
    ///
    /// If `p` is not in `0..1`.
    pub fn group_padding(mut self, p: f64) -> Self {
        assert!(
            (0.0..1.0).contains(&p),
            "group_padding must be within 0..1, not {p}"
        );
        self.check_open();
        self.group_padding = p;
        self
    }

    /// Colour by a category key per row, through the colour scale. The last
    /// of `color` and `fill` wins; either replaces the colouring by group
    /// or stack key.
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

    /// One fill colour for every bar. The last of `color` and `fill` wins.
    pub fn fill(mut self, color: Color) -> Self {
        self.check_open();
        self.color = ColorSpec::Fill(color);
        self
    }

    /// Bars side by side within each band, one slot per key (first-seen
    /// order), coloured by key. The last of `group` and `stack` wins.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn group<K: DictionaryKey>(
        mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.arrange = Arrange::Group(erase_key(key));
        self
    }

    /// Bars stacked within each band, bottom to top in first-seen key
    /// order, coloured by key. Positive values stack upward from 0 and
    /// negative values downward from 0. Rows sharing a category and a key
    /// draw as one bar of their summed height. The last of `group` and
    /// `stack` wins.
    ///
    /// The accessor returns an owned key (an integer, an enum with `Display`,
    /// `r.name.clone()`, a `&'static str`); see
    /// [category keys](crate::builder#category-keys).
    pub fn stack<K: DictionaryKey>(
        mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.arrange = Arrange::Stack(erase_key(key));
        self
    }

    /// Print each bar's value at its end: above a non-negative vertical bar,
    /// below a negative one, right of (left of) a horizontal one, and
    /// centred in each segment of a [`stack`](Self::stack) (a segment too
    /// short to hold its label has none). The text is the value axis's tick
    /// format, however it was set (on the builder or through `plot_mut()`),
    /// if one is set, else the value to two decimals at most. A format set
    /// only with `ScaleRef::tick_format` on a scale handle is not seen. An
    /// automatic
    /// value axis makes room for the labels. The last of this and
    /// [`value_labels_with`](Self::value_labels_with) wins.
    ///
    /// ```
    /// use gup_core::scene::TextRole;
    /// use gup_core::{ChartExt, Context};
    /// # fn main() -> gup_core::Result<()> {
    /// let mut chart = gup_core::bar([("Q1", 3.0), ("Q2", 5.25), ("Q3", -1.5)])
    ///     .x(|r| r.0)
    ///     .y(|r| r.1)
    ///     .value_labels();
    /// # let cx = Context::new_blocking()?;
    /// let (_, layout) = chart.render_resolved(&cx, 320, 200)?;
    /// let mut labels: Vec<&str> = layout
    ///     .texts
    ///     .iter()
    ///     .filter(|t| t.run.role == TextRole::Annotation)
    ///     .map(|t| &*t.run.text)
    ///     .collect();
    /// labels.sort();
    /// assert_eq!(labels, ["3", "5.25", "\u{2212}1.5"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn value_labels(mut self) -> Self {
        self.check_open();
        self.labels = LabelSpec::Default;
        self
    }

    /// Like [`value_labels`](Self::value_labels), with the text from `format`
    /// (given the bar's value, or a segment's summed value).
    ///
    /// ```
    /// let chart = gup_core::bar([("Q1", 3.0), ("Q2", 5.25)])
    ///     .x(|r| r.0)
    ///     .y(|r| r.1)
    ///     .value_labels_with(|v| format!("${v:.1}M"));
    /// # let _ = chart;
    /// ```
    pub fn value_labels_with(
        mut self,
        format: impl Fn(f64) -> String + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.labels = LabelSpec::With(Arc::new(format));
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
        let mut index: HashMap<Arc<str>, usize> = HashMap::new();
        let slots: Vec<usize> = rows
            .iter()
            .map(|r| {
                let k = key.label(r);
                if let Some(&i) = index.get(&k) {
                    i
                } else {
                    let i = index.len();
                    index.insert(k, i);
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
        if let Some(xk) = &self.cat_key {
            let mut seen = std::collections::HashSet::new();
            for r in &rows {
                let k = xk.label(r);
                if seen.insert(Arc::clone(&k)) {
                    categories.push(k);
                }
            }
        }
        if O::HORIZONTAL {
            // A y band's range runs bottom to top; the first category goes
            // on top.
            categories.reverse();
        }
        let (part_key, stacked) = match &self.arrange {
            Arrange::Simple => (None, false),
            Arrange::Group(k) => (Some(k.clone()), false),
            Arrange::Stack(k) => (Some(k.clone()), true),
        };
        let parts = Self::split(rows, part_key.as_ref());
        let count = parts.len().max(1) as u32;

        let tables = if stacked {
            let sums: Vec<HashMap<Arc<str>, f64>> = parts
                .iter()
                .map(|part| {
                    let mut sum: HashMap<Arc<str>, f64> = HashMap::new();
                    if let (Some(xk), Some(y)) = (&self.cat_key, &self.value) {
                        for r in part {
                            let v = y(r);
                            if v.is_finite() {
                                *sum.entry(xk.label(r)).or_insert(0.0) += v;
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
            let label_rows = self.label_rows(&part, tables.as_ref().map(|t| &t[i]), &categories);
            let sel = self.plot.rects(part);
            if let Some(xk) = &self.cat_key {
                let xk = xk.clone();
                let enc = |s: &mut Selection<T, Rect>| {
                    let key = LabelKey(xk.clone());
                    let grouped = matches!(self.arrange, Arrange::Group(_));
                    // The first key takes the topmost bar of a horizontal band.
                    let slot = if O::HORIZONTAL {
                        count - 1 - i as u32
                    } else {
                        i as u32
                    };
                    let domain = categories.iter().cloned();
                    match (O::HORIZONTAL, grouped) {
                        (false, true) => {
                            s.x_with(
                                KeyEncoded::new(key.clone(), self.band.sub_band(slot, count))
                                    .domain(domain),
                            )
                            .width(self.band.sub_band_width_px(count, self.group_padding));
                        }
                        (true, true) => {
                            s.y_with(
                                KeyEncoded::new(key.clone(), self.band.sub_band(slot, count))
                                    .domain(domain),
                            )
                            .height(self.band.sub_band_width_px(count, self.group_padding));
                        }
                        (false, false) => {
                            s.x_with(
                                KeyEncoded::new(key.clone(), self.band.clone()).domain(domain),
                            )
                            .width(self.band.band_width_px());
                        }
                        (true, false) => {
                            s.y_with(
                                KeyEncoded::new(key.clone(), self.band.clone()).domain(domain),
                            )
                            .height(self.band.band_width_px());
                        }
                    }
                };
                enc(sel);
            }
            if let Some(y) = &self.value {
                let (end, base): (Value<T>, Value<T>) = if let Some(tables) = &tables {
                    let table = Arc::new(tables[i].clone());
                    let (top, base) = (Arc::clone(&table), table);
                    let (yt, yb) = (Arc::clone(y), Arc::clone(y));
                    let (xt, xb) = (self.cat_key.clone(), self.cat_key.clone());
                    (
                        Arc::new(move |r: &T| match &xt {
                            Some(xk) if yt(r).is_finite() => {
                                top.get(&xk.label(r)).map_or(f64::NAN, |&(_, t)| t)
                            }
                            _ => f64::NAN,
                        }),
                        Arc::new(move |r: &T| match &xb {
                            Some(xk) if yb(r).is_finite() => {
                                base.get(&xk.label(r)).map_or(f64::NAN, |&(b, _)| b)
                            }
                            _ => f64::NAN,
                        }),
                    )
                } else {
                    (Arc::clone(y), Arc::new(|_: &T| 0.0f64))
                };
                if O::HORIZONTAL {
                    sel.x(move |r: &T| end(r));
                    sel.x_end(move |r: &T| base(r));
                } else {
                    sel.y(move |r: &T| end(r));
                    sel.y_end(move |r: &T| base(r));
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
                (ColorSpec::Key(k), _) => Some(k.clone()),
                (ColorSpec::Default, k) => k.clone(),
            };
            if let Some(key) = key {
                sel.fill(KeyEncoded::new(LabelKey(key), colours.clone()));
                if let Some(title) = &self.legend_title {
                    sel.legend_title(Rect::FILL, title.clone());
                }
            }
            if !label_rows.is_empty() {
                let slot = if O::HORIZONTAL {
                    count - 1 - i as u32
                } else {
                    i as u32
                };
                let stack = stacked.then(|| match &self.color {
                    ColorSpec::Default => Fill::Scale(colours.clone()),
                    ColorSpec::Fill(c) => Fill::Const(*c),
                    ColorSpec::Key(_) => Fill::Theme,
                });
                self.add_value_labels(label_rows, slot, count, stack);
            }
        }
        self.ops.clear();
    }

    /// The label rows of one bar layer: per row for simple and grouped
    /// bars, per category (summed) for a stack. Empty if labels are off.
    fn label_rows(
        &self,
        part: &[T],
        table: Option<&HashMap<Arc<str>, (f64, f64)>>,
        categories: &[Arc<str>],
    ) -> Vec<BarLabel> {
        let (Some(xk), Some(y), true) = (
            &self.cat_key,
            &self.value,
            !matches!(self.labels, LabelSpec::Off),
        ) else {
            return Vec::new();
        };
        if let Some(table) = table {
            let stack = match &self.arrange {
                Arrange::Stack(k) => part.first().map(|r| k.label(r)),
                _ => None,
            };
            return categories
                .iter()
                .filter_map(|c| {
                    let &(base, top) = table.get(c)?;
                    Some(BarLabel {
                        key: Arc::clone(c),
                        value: top - base,
                        base,
                        top,
                        stack: stack.clone(),
                    })
                })
                .collect();
        }
        part.iter()
            .filter_map(|r| {
                let v = y(r);
                v.is_finite().then(|| BarLabel {
                    key: xk.label(r),
                    value: v,
                    base: 0.0,
                    top: v,
                    stack: None,
                })
            })
            .collect()
    }

    /// Add the text layers that print `rows`; `stack` is `Some` for a
    /// stack layer, with what fills its segments.
    fn add_value_labels(
        &mut self,
        rows: Vec<BarLabel>,
        slot: u32,
        count: u32,
        stack: Option<Fill>,
    ) {
        let format: Format = match &self.labels {
            LabelSpec::Off => return,
            LabelSpec::With(f) => Arc::clone(f),
            LabelSpec::Default => {
                // Read at evaluation, so a format set later still applies.
                let slot = self.plot.axis_format(usize::from(!O::HORIZONTAL));
                Arc::new(move |v| slot.get().map_or_else(|| default_label(v), |f| f.apply(v)))
            }
        };
        let text_of = move |l: &BarLabel| format(l.value);
        if let Some(fill) = stack {
            let layer = self.plot.text(rows);
            let mid = |l: &BarLabel| (l.base + l.top) / 2.0;
            if O::HORIZONTAL {
                layer
                    .y_key(|l| Arc::clone(&l.key))
                    .x(mid)
                    .within_x(|l| (l.base, l.top));
            } else {
                layer
                    .x_key(|l| Arc::clone(&l.key))
                    .y(mid)
                    .within_y(|l| (l.base, l.top));
            }
            layer.text(text_of).anchor(Anchor::CENTER);
            match fill {
                Fill::Theme => {}
                Fill::Const(c) => {
                    layer.color(contrast_text(c));
                }
                Fill::Scale(colours) => {
                    layer.color_by(move |l| {
                        let scale = colours.read();
                        let code = l
                            .stack
                            .as_ref()
                            .and_then(|k| scale.keys().iter().position(|s| s == k));
                        contrast_text(scale.eval(code.map_or(f64::NAN, |c| c as f64)))
                    });
                }
            }
            return;
        }
        let offset = matches!(self.arrange, Arrange::Group(_)).then(|| {
            let band = self.band.clone();
            // The offset of `sub_band(slot, count)`.
            Derived::new(move || {
                let w = band.read().band_width();
                Px((((f64::from(slot) + 0.5) / f64::from(count) - 0.5) * w) as f32)
            })
        });
        let (positive, negative): (Vec<_>, Vec<_>) = rows.into_iter().partition(|l| l.value >= 0.0);
        let text_of = Arc::new(text_of);
        for (rows, after) in [(positive, true), (negative, false)] {
            if rows.is_empty() {
                continue;
            }
            let text_of = Arc::clone(&text_of);
            let layer = self.plot.text(rows);
            layer.text(move |l| text_of(l));
            if O::HORIZONTAL {
                layer.y_key(|l| Arc::clone(&l.key)).x(|l| l.top);
                if after {
                    layer.anchor(Anchor::LEFT).dx(4.0);
                } else {
                    layer.anchor(Anchor::RIGHT).dx(-4.0);
                }
                if let Some(o) = &offset {
                    layer.dy_with(o.clone());
                }
            } else {
                layer.x_key(|l| Arc::clone(&l.key)).y(|l| l.top);
                if after {
                    // The baseline, not the descender line (`Anchor::BOTTOM`): the
                    // digits' ink (which sits a little above or below it, snapped to whole
                    // pixels) then ends
                    // 4 px above the bar.
                    layer
                        .anchor(Anchor::new(HAlign::Middle, VAlign::Baseline))
                        .dy(-4.5);
                } else {
                    layer.anchor(Anchor::TOP).dy(4.0);
                }
                if let Some(o) = &offset {
                    layer.dx_with(o.clone());
                }
            }
        }
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
    pub fn pick(&mut self, at: impl Into<Point>) -> Option<&T> {
        self.materialise();
        self.plot.pick::<T>(at)
    }
}

impl<T: Send + Sync + 'static, O: Orientation> Chart for BarChart<T, O> {
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

    #[test]
    fn the_default_label_trims_and_uses_the_minus_sign() {
        for (v, want) in [
            (3.0, "3"),
            (3.25, "3.25"),
            (3.256, "3.26"),
            (-1.5, "\u{2212}1.5"),
            (1_234_567.0, "1234567"),
            (-0.001, "0"),
            (10.0, "10"),
        ] {
            assert_eq!(default_label(v), want, "{v}");
        }
    }

    #[test]
    fn stacked_label_text_contrasts_with_its_fill() {
        // Okabe-Ito yellow takes dark text, blue takes white.
        assert_eq!(
            contrast_text(Color::hex(0xF0_E4_42)),
            Color::hex(0x11_11_11)
        );
        assert_eq!(contrast_text(Color::hex(0x00_72_B2)), Color::WHITE);
    }
}
