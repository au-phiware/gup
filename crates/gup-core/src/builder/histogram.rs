// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The histogram builder: [`histogram`], [`HistogramChart`] and
//! [`HistogramBin`].

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use super::{DRAWN, Key, erase_key, plot_setters};
use crate::column::DictionaryKey;
use crate::geom::Point;
use crate::prelude::*;
use crate::transform::{edges, index_of};
use crate::{ColumnValue, Context, Input, Layout, Resolved, Response, Result, RowId};

type Value<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;
type Op = Box<dyn Fn(&mut Selection<HistogramBin, Rect>) + Send + Sync>;

/// A bin narrower than this on screen (logical pixels) touches its
/// neighbours instead of leaving a gap.
const MIN_GAPPED_PX: f64 = 4.0;
/// Half the gap between bars (logical pixels), taken from each side.
const HALF_GAP_PX: f32 = 0.5;

enum ColorSpec<T> {
    Default,
    Fill(Color),
    Key(Key<T>),
}

/// One drawn bar of a histogram: a bin, and for a coloured histogram one
/// colour's share of it.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramBin {
    /// The lower edge of the bin, in the x values' units.
    pub lo: f64,
    /// The upper edge of the bin.
    pub hi: f64,
    /// The number of values in this bar.
    pub count: usize,
    /// The colour key of this bar's values, if the chart is coloured.
    pub key: Option<Arc<str>>,
    /// Where the bar starts: 0, or the top of the bars below it in the bin.
    pub base: f64,
    /// Where the bar ends: `base + count`.
    pub top: f64,
}

/// Start a histogram over `rows`: the distribution of the value
/// [`x`](HistogramChart::x) reads from each row, counted into bins whose
/// edges fall on round numbers. [`bins`](HistogramChart::bins) chooses
/// them; the default is [`Bins::Auto`]. Add
/// [`color`](HistogramChart::color) to stack each bin's count by key.
///
/// ```
/// use gup_core::{Bins, ChartExt, Context};
/// # fn main() -> gup_core::Result<()> {
/// struct Trip { minutes: f64 }
/// let trips = (0..200).map(|i| Trip { minutes: f64::from(i % 50) });
///
/// let mut chart = gup_core::histogram(trips)
///     .x(|t| t.minutes)
///     .bins(Bins::Width(5.0));
/// // Ten bins of five minutes: 0-5, 5-10, ... 45-50.
/// assert_eq!(chart.layers_mut()[0].len(), 10);
///
/// # let cx = Context::new_blocking()?;
/// let image = chart.render(&cx, 320, 200)?;
/// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
/// # Ok(())
/// # }
/// ```
///
/// See the [module docs](crate::builder) for the pattern.
pub fn histogram<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> HistogramChart<T> {
    let mut plot = Plot::new();
    plot.y(Linear::new());
    plot.round_to_ticks(1);
    plot.y_title("Count");
    HistogramChart {
        plot,
        rows: Some(rows.into_iter().collect()),
        ops: Vec::new(),
        x_value: None,
        bins: Bins::Auto,
        x_domain: None,
        color: ColorSpec::Default,
        color_scale: Categorical::okabe_ito(),
        legend_title: None,
    }
}

/// A histogram under construction, and a [`Chart`].
///
/// # Panics
///
/// The layer setters (`x`, `bins`, `x_domain`, `color*`, `fill`, `opacity`,
/// `tooltip`, `legend_title`) panic once the chart has been drawn, with
/// "set layer encodings before the chart is first drawn; change a drawn
/// chart through layer_mut()". The plot-level setters work at any time.
#[must_use]
pub struct HistogramChart<T> {
    plot: Plot,
    rows: Option<Vec<T>>,
    ops: Vec<Op>,
    x_value: Option<Value<T>>,
    bins: Bins,
    x_domain: Option<(f64, f64)>,
    color: ColorSpec<T>,
    color_scale: Categorical,
    legend_title: Option<String>,
}

impl<T> std::fmt::Debug for HistogramChart<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistogramChart")
            .field("drawn", &self.rows.is_none())
            .finish_non_exhaustive()
    }
}

/// The bin width, if the edges are all multiples of it (so that ticks at
/// its multiples fall on edges): not for [`Bins::Edges`], nor for a single
/// bin centred on its value.
fn regular_unit(edges: &[f64], bins: &Bins) -> Option<f64> {
    if matches!(bins, Bins::Edges(_)) || edges.len() < 3 {
        return None;
    }
    let unit = edges[1] - edges[0];
    edges
        .iter()
        .all(|e| ((e / unit) - (e / unit).round()).abs() < 1e-9)
        .then_some(unit)
}

/// The edges and bars of a histogram.
struct Tallied {
    edges: Vec<f64>,
    /// The bars of each layer (colour key, first-seen order; one layer if
    /// there is no key), left to right, empty bins left out.
    layers: Vec<Vec<HistogramBin>>,
}

/// Bin the value of each row, and split the counts by its key.
fn tally<T>(
    rows: &[T],
    value: Option<&Value<T>>,
    key: Option<&Key<T>>,
    bins: &Bins,
    domain: Option<(f64, f64)>,
) -> Tallied {
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    // Each counted value, and the layer (key) it belongs to.
    let mut keys: Vec<Arc<str>> = Vec::new();
    let mut index: HashMap<Arc<str>, usize> = HashMap::new();
    let mut values: Vec<(f64, usize)> = Vec::new();
    if let Some(value) = value {
        for r in rows {
            let v = value(r);
            let inside = domain.is_none_or(|(lo, hi)| (lo..=hi).contains(&v));
            if !v.is_finite() || !inside {
                continue;
            }
            let layer = key.map_or(0, |key| {
                let k = key.label(r);
                *index.entry(Arc::clone(&k)).or_insert_with(|| {
                    keys.push(k);
                    keys.len() - 1
                })
            });
            min = min.min(v);
            max = max.max(v);
            values.push((v, layer));
        }
    }
    let edges = edges(min, max, values.len(), bins).unwrap_or_default();
    let bin_count = edges.len().saturating_sub(1);
    let layers = keys.len().max(1);
    let mut counts = vec![vec![0usize; bin_count]; layers];
    for &(v, layer) in &values {
        if let Some(i) = index_of(&edges, v) {
            counts[layer][i] += 1;
        }
    }
    let mut below = vec![0usize; bin_count];
    let layers = counts
        .iter()
        .enumerate()
        .map(|(layer, counts)| {
            let bars = (0..bin_count)
                .filter(|&i| counts[i] > 0)
                .map(|i| {
                    let base = below[i] as f64;
                    HistogramBin {
                        lo: edges[i],
                        hi: edges[i + 1],
                        count: counts[i],
                        key: keys.get(layer).cloned(),
                        base,
                        top: base + counts[i] as f64,
                    }
                })
                .collect();
            for (b, c) in below.iter_mut().zip(counts) {
                *b += c;
            }
            bars
        })
        .collect();
    Tallied { edges, layers }
}

impl<T: Send + Sync + 'static> HistogramChart<T> {
    plot_setters!();

    fn check_open(&self) {
        assert!(self.rows.is_some(), "{DRAWN}");
    }

    fn op(
        mut self,
        op: impl Fn(&mut Selection<HistogramBin, Rect>) + Send + Sync + 'static,
    ) -> Self {
        self.check_open();
        self.ops.push(Box::new(op));
        self
    }

    /// The value to count from each row. `None`, NaN and infinite values
    /// are skipped.
    pub fn x<D: ColumnValue>(mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self {
        self.check_open();
        self.x_value = Some(Arc::new(move |r| f(r).to_f64()));
        self
    }

    /// How to choose the bins (default [`Bins::Auto`]). The axis runs from
    /// the first edge to the last.
    ///
    /// # Panics
    ///
    /// If `bins` is [`Bins::Width`] with a width that is not positive and
    /// finite, or [`Bins::Edges`] with fewer than two edges or edges that
    /// are not finite and strictly ascending.
    pub fn bins(mut self, bins: Bins) -> Self {
        self.check_open();
        bins.validate();
        self.bins = bins;
        self
    }

    /// Count only the values in `lo..=hi`; the others are dropped before
    /// the edges are chosen. The axis still runs from the first edge to
    /// the last, which may lie beyond `lo` and `hi` (round numbers).
    pub fn x_domain(mut self, lo: f64, hi: f64) -> Self {
        self.check_open();
        self.x_domain = Some((lo, hi));
        self
    }

    /// Stack each bin's count by a category key per row, in first-seen
    /// key order, coloured through the colour scale, with a legend. The
    /// last of `color` and `fill` wins.
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

    /// The tooltip text of a bar.
    pub fn tooltip(self, text: impl Fn(&HistogramBin) -> String + Send + Sync + 'static) -> Self {
        let text = Arc::new(text);
        self.op(move |s| {
            let text = Arc::clone(&text);
            s.tooltip(move |b: &HistogramBin| text(b));
        })
    }

    /// Build the layers from the recorded settings, once.
    fn materialise(&mut self) {
        let Some(rows) = self.rows.take() else {
            return;
        };
        let key_of = match &self.color {
            ColorSpec::Key(k) => Some(k.clone()),
            _ => None,
        };
        let tallied = tally(
            &rows,
            self.x_value.as_ref(),
            key_of.as_ref(),
            &self.bins,
            self.x_domain,
        );
        drop(rows);
        let Tallied {
            edges,
            layers: tables,
        } = tallied;
        let x = match (edges.first(), edges.last()) {
            (Some(&lo), Some(&hi)) => {
                let mut scale = Linear::new().domain(lo, hi);
                if let Some(unit) = regular_unit(&edges, &self.bins) {
                    scale = scale.tick_unit(unit);
                }
                self.plot.x(scale)
            }
            _ => self.plot.x(Linear::new()),
        };
        // The gap is decided by the narrowest bin, so that every bar of the
        // chart keeps or loses its gap together.
        let narrowest = edges
            .windows(2)
            .min_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
            .map_or((0.0, 1.0), |w| (w[0], w[1]));
        let half_gap = {
            let x = x.clone();
            move |sign: f32| {
                let x = x.clone();
                Derived::new(move || {
                    let scale = x.read();
                    let width = (scale.eval(narrowest.1) - scale.eval(narrowest.0)).abs();
                    Px(if width >= MIN_GAPPED_PX {
                        sign * HALF_GAP_PX
                    } else {
                        0.0
                    })
                })
            }
        };

        let colours = ScaleRef::new(self.color_scale.clone());
        for bars in tables {
            let sel = self.plot.rects(bars);
            sel.x_with(
                x.then(Offset::by(half_gap(1.0)))
                    .encode(|b: &HistogramBin| b.lo),
            );
            sel.x_end_with(
                x.then(Offset::by(half_gap(-1.0)))
                    .encode(|b: &HistogramBin| b.hi),
            );
            sel.y(|b: &HistogramBin| b.top);
            sel.y_end(|b: &HistogramBin| b.base);
            for op in &self.ops {
                op(sel);
            }
            match &self.color {
                ColorSpec::Default => {}
                ColorSpec::Fill(c) => {
                    sel.fill(*c);
                }
                ColorSpec::Key(_) => {
                    sel.fill(colours.encode_owned_key(|b: &HistogramBin| {
                        b.key.clone().unwrap_or_else(|| Arc::from(""))
                    }));
                    if let Some(title) = &self.legend_title {
                        sel.legend_title(Rect::FILL, title.clone());
                    }
                }
            }
        }
        self.ops.clear();
    }

    /// The bar layers, one per colour key in first-seen order (one if the
    /// chart is not coloured), for settings the builder has no method for.
    /// Materialises the chart: layer setters panic after.
    pub fn layers_mut(&mut self) -> Vec<&mut Selection<HistogramBin, Rect>> {
        self.materialise();
        self.plot.layers_mut::<HistogramBin, Rect>().collect()
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

    /// The bar under `at` (logical pixels), once the chart has been
    /// resolved.
    pub fn pick(&mut self, at: impl Into<Point>) -> Option<&HistogramBin> {
        self.materialise();
        self.plot.pick::<HistogramBin>(at)
    }
}

impl<T: Send + Sync + 'static> Chart for HistogramChart<T> {
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

    fn value(r: &(&'static str, f64)) -> f64 {
        r.1
    }

    fn rows() -> Vec<(&'static str, f64)> {
        let mut rows = Vec::new();
        for i in 0..100 {
            rows.push(("a", f64::from(i) * 0.1 + 0.05));
            if i % 3 == 0 {
                rows.push(("b", f64::from(i) * 0.1 + 0.05));
            }
        }
        rows
    }

    #[test]
    fn stacking_by_key_keeps_the_bin_totals() {
        let rows = rows();
        let v: Value<(&str, f64)> = Arc::new(value);
        let k = erase_key(|r: &(&str, f64)| Arc::<str>::from(r.0));
        let plain = tally(&rows, Some(&v), None, &Bins::Auto, None);
        let keyed = tally(&rows, Some(&v), Some(&k), &Bins::Auto, None);
        assert_eq!(plain.edges, keyed.edges);
        assert_eq!(plain.layers.len(), 1);
        assert_eq!(keyed.layers.len(), 2);
        assert_eq!(keyed.layers[0][0].key.as_deref(), Some("a"));
        assert_eq!(keyed.layers[1][0].key.as_deref(), Some("b"));
        for p in &plain.layers[0] {
            let parts: Vec<&HistogramBin> = keyed
                .layers
                .iter()
                .flatten()
                .filter(|b| (b.lo, b.hi) == (p.lo, p.hi))
                .collect();
            assert_eq!(parts.iter().map(|b| b.count).sum::<usize>(), p.count);
            assert_eq!((parts[0].base, parts[0].top), (0.0, parts[0].count as f64));
            assert_eq!(parts[1].base, parts[0].top);
            assert_eq!(parts[1].top, p.count as f64);
        }
        assert_eq!(
            (plain.layers[0][0].base, plain.layers[0][0].top),
            (0.0, plain.layers[0][0].count as f64)
        );
    }

    #[test]
    fn the_builder_counts_as_the_transform_does() {
        let rows = rows();
        let v: Value<(&str, f64)> = Arc::new(value);
        let got = tally(&rows, Some(&v), None, &Bins::Auto, None);
        let want = crate::transform::bin(rows.iter().map(|r| r.1), &Bins::Auto);
        let got: Vec<_> = got.layers[0]
            .iter()
            .map(|b| (b.lo, b.hi, b.count))
            .collect();
        let want: Vec<_> = want.iter().map(|b| (b.lo, b.hi, b.count)).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn x_domain_drops_values_outside_and_missing_rows_are_skipped() {
        let v: Value<f64> = Arc::new(|r| *r);
        let rows = [-50.0, 1.0, f64::NAN, 2.0, 3.0, 4.0, 90.0, f64::INFINITY];
        let t = tally(&rows, Some(&v), None, &Bins::Width(2.0), Some((0.0, 10.0)));
        assert_eq!(t.layers[0].iter().map(|b| b.count).sum::<usize>(), 4);
        assert_eq!((t.edges[0], t.edges[t.edges.len() - 1]), (0.0, 4.0));
        let t = tally(&rows, Some(&v), None, &Bins::Width(2.0), None);
        assert_eq!(t.layers[0].iter().map(|b| b.count).sum::<usize>(), 6);
    }

    #[test]
    fn no_values_make_one_empty_layer() {
        let t = tally::<f64>(&[], None, None, &Bins::Auto, None);
        assert!(t.edges.is_empty());
        assert_eq!(t.layers.len(), 1);
        assert!(t.layers[0].is_empty());
    }
}
