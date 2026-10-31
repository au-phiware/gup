// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data transforms that run before drawing: [`bin`] counts values into
//! [`Bin`]s, the rows of a histogram, and [`box_stats`] summarises values as
//! the [`BoxStats`] of a box plot.
//!
//! ```
//! use gup_core::transform::{Bins, bin};
//!
//! let bins = bin([0.5, 1.5, 1.7, 9.5], &Bins::Width(5.0));
//! let counts: Vec<_> = bins.iter().map(|b| (b.lo, b.hi, b.count)).collect();
//! assert_eq!(counts, [(0.0, 5.0, 3), (5.0, 10.0, 1)]);
//! ```

use crate::scale::{step_value, tick_step};

/// How [`bin`] chooses its edges.
#[derive(Clone, Debug, PartialEq)]
pub enum Bins {
    /// About ⌈log2 n⌉ + 1 bins (Sturges), on a 1–2–5 step. Sturges
    /// under-bins very large inputs (a million values give 21 bins): use
    /// [`Count`](Self::Count) or [`Width`](Self::Width) for those.
    Auto,
    /// About this many bins, on a 1–2–5 step.
    Count(usize),
    /// Bins of exactly this width, with edges at its multiples.
    Width(f64),
    /// These edges, ascending (n edges make n − 1 bins).
    Edges(Vec<f64>),
}

impl Bins {
    /// Panics unless the setting can produce bins.
    pub(crate) fn validate(&self) {
        match self {
            Bins::Width(w) => assert!(
                w.is_finite() && *w > 0.0,
                "Bins::Width must be positive and finite, not {w}"
            ),
            Bins::Edges(e) => assert!(
                e.len() >= 2
                    && e.iter().all(|v| v.is_finite())
                    && e.windows(2).all(|w| w[0] < w[1]),
                "Bins::Edges needs at least two finite, strictly ascending edges, not {e:?}"
            ),
            Bins::Auto | Bins::Count(_) => {}
        }
    }
}

/// One bin: the values in `lo..hi` (`lo..=hi` for the last bin).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bin {
    /// The lower edge, included.
    pub lo: f64,
    /// The upper edge, excluded unless this is the last bin.
    pub hi: f64,
    /// How many values fell in the bin.
    pub count: usize,
}

/// The edges `bins` gives for `n` values spanning `min..=max`. `None` if
/// there are no edges to give.
pub(crate) fn edges(min: f64, max: f64, n: usize, bins: &Bins) -> Option<Vec<f64>> {
    match bins {
        Bins::Edges(e) => Some(e.clone()),
        _ if n == 0 => None,
        _ if min == max => Some(vec![min - 0.5, min + 0.5]),
        Bins::Auto => {
            let k = (n as f64).log2().ceil() as usize + 1;
            Some(round_edges(min, max, tick_step(min, max, k)))
        }
        Bins::Count(k) => Some(round_edges(min, max, tick_step(min, max, *k))),
        Bins::Width(w) => Some(round_edges(min, max, *w)),
    }
}

/// Edges at multiples of `step` from the one at or below `min` to the one
/// at or above `max`. Edge `i` is computed from its index, never
/// accumulated.
fn round_edges(min: f64, max: f64, step: f64) -> Vec<f64> {
    let k0 = (min / step).floor();
    let mut k1 = (max / step).ceil().max(k0 + 1.0);
    while step_value(k1, step) < max {
        k1 += 1.0;
    }
    (0..=(k1 - k0) as usize)
        .map(|i| step_value(k0 + i as f64, step))
        .collect()
}

/// Count `values` into bins. NaN and infinite values are dropped.
///
/// Every bin is `lo..hi` except the last, which includes `hi`. A value on
/// an inner edge belongs to the bin above it. With [`Bins::Edges`], values
/// outside the edges are dropped and every bin is returned, empty or not;
/// otherwise the edges run from the multiple of the step at or below the
/// smallest value to the one at or above the largest, so they fall on round
/// numbers, and no values gives no bins. All-equal values give one bin of
/// width 1 centred on the value.
///
/// ```
/// use gup_core::transform::{Bins, bin};
///
/// let values = (0..1000).map(|i| 0.3 + 9.4 * f64::from(i) / 999.0);
/// let bins = bin(values, &Bins::Auto);
/// assert_eq!(bins.len(), 10);
/// assert_eq!((bins[0].lo, bins[9].hi), (0.0, 10.0));
/// assert_eq!(bins.iter().map(|b| b.count).sum::<usize>(), 1000);
/// ```
///
/// # Panics
///
/// If `bins` is [`Bins::Width`] with a width that is not positive and
/// finite, or [`Bins::Edges`] with fewer than two edges or edges that are
/// not finite and strictly ascending.
pub fn bin(values: impl IntoIterator<Item = f64>, bins: &Bins) -> Vec<Bin> {
    bins.validate();
    let values: Vec<f64> = values.into_iter().filter(|v| v.is_finite()).collect();
    let (min, max) = values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
    let Some(edges) = edges(min, max, values.len(), bins) else {
        return Vec::new();
    };
    let mut out: Vec<Bin> = edges
        .windows(2)
        .map(|w| Bin {
            lo: w[0],
            hi: w[1],
            count: 0,
        })
        .collect();
    for v in values {
        if let Some(i) = index_of(&edges, v) {
            out[i].count += 1;
        }
    }
    out
}

/// The bin of `edges` holding `v`, if any.
pub(crate) fn index_of(edges: &[f64], v: f64) -> Option<usize> {
    let (first, last) = (edges[0], edges[edges.len() - 1]);
    if !(first..=last).contains(&v) {
        return None;
    }
    let above = edges.partition_point(|&e| e <= v);
    Some(above.saturating_sub(1).min(edges.len() - 2))
}

/// The summary a box plot draws (Tukey's definition).
///
/// Quartiles are linear interpolations between order statistics (the
/// default "type 7" of R and NumPy's `percentile`); the whiskers reach the
/// most extreme values within 1.5 × IQR of the box.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxStats {
    /// How many values were summarised.
    pub n: usize,
    /// The first quartile.
    pub q1: f64,
    /// The median.
    pub median: f64,
    /// The third quartile.
    pub q3: f64,
    /// The smallest value at or above q1 − 1.5 IQR, and the largest at or
    /// below q3 + 1.5 IQR.
    pub whisker_lo: f64,
    /// See `whisker_lo`: the largest value at or below q3 + 1.5 IQR.
    pub whisker_hi: f64,
    /// Values outside the whiskers, ascending.
    pub outliers: Vec<f64>,
}

/// The quantile `p` (in `0..=1`) of `sorted`, interpolating linearly
/// between order statistics: `h = (n − 1) p`.
fn quantile(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let i = h.floor() as usize;
    match sorted.get(i + 1) {
        Some(next) => sorted[i] + (h - i as f64) * (next - sorted[i]),
        None => sorted[i],
    }
}

/// Summarise `values` as a box plot does. NaN and infinite values are
/// dropped; no values give `None`.
///
/// ```
/// use gup_core::transform::box_stats;
///
/// let s = box_stats([1.0, 2.0, 3.0, 4.0]).unwrap();
/// assert_eq!((s.q1, s.median, s.q3), (1.75, 2.5, 3.25));
///
/// // 100 is more than 1.5 IQR above the box: an outlier, not a whisker.
/// let s = box_stats((1..=9).map(f64::from).chain([100.0])).unwrap();
/// assert_eq!((s.whisker_hi, s.outliers), (9.0, vec![100.0]));
/// ```
pub fn box_stats(values: impl IntoIterator<Item = f64>) -> Option<BoxStats> {
    let mut v: Vec<f64> = values.into_iter().filter(|v| v.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let (q1, median, q3) = (quantile(&v, 0.25), quantile(&v, 0.5), quantile(&v, 0.75));
    let iqr = q3 - q1;
    let (fence_lo, fence_hi) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    let first = v.partition_point(|&x| x < fence_lo);
    let last = v.partition_point(|&x| x <= fence_hi);
    // The fences lie outside the quartiles, so at least the middle values
    // are inside them: `first < last`.
    let outliers = v[..first].iter().chain(&v[last..]).copied().collect();
    Some(BoxStats {
        n: v.len(),
        q1,
        median,
        q3,
        whisker_lo: v[first],
        whisker_hi: v[last - 1],
        outliers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(bins: &[Bin]) -> (f64, f64) {
        (bins[0].lo, bins[bins.len() - 1].hi)
    }

    #[test]
    fn auto_gives_unit_bins_over_a_round_range() {
        let values = (0..1000).map(|i| 0.3 + 9.4 * f64::from(i) / 999.0);
        let bins = bin(values, &Bins::Auto);
        assert_eq!(bins.len(), 10);
        assert_eq!(span(&bins), (0.0, 10.0));
        assert!(bins.iter().all(|b| b.hi - b.lo == 1.0));
        assert_eq!(bins.iter().map(|b| b.count).sum::<usize>(), 1000);
    }

    #[test]
    fn inner_edges_go_up_and_the_maximum_to_the_last_bin() {
        let bins = bin([0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &Bins::Width(1.0));
        let counts: Vec<usize> = bins.iter().map(|b| b.count).collect();
        assert_eq!(counts, [1, 1, 1, 1, 2]);
        assert_eq!(span(&bins), (0.0, 5.0));
    }

    #[test]
    fn non_finite_values_are_dropped() {
        let bins = bin(
            [1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 2.0],
            &Bins::Width(1.0),
        );
        assert_eq!(bins.iter().map(|b| b.count).sum::<usize>(), 2);
        assert_eq!(span(&bins), (1.0, 2.0));
    }

    #[test]
    fn width_puts_edges_at_multiples() {
        let bins = bin([1.0, 6.0, 12.0], &Bins::Width(2.5));
        let lows: Vec<f64> = bins.iter().map(|b| b.lo).collect();
        assert_eq!(lows, [0.0, 2.5, 5.0, 7.5, 10.0]);
        assert_eq!(bins.last().unwrap().hi, 12.5);
    }

    #[test]
    fn explicit_edges_drop_values_outside() {
        let bins = bin(
            [-1.0, 0.0, 0.5, 1.0, 9.0, 10.0, 11.0],
            &Bins::Edges(vec![0.0, 1.0, 10.0]),
        );
        assert_eq!(bins.len(), 2);
        assert_eq!((bins[0].count, bins[1].count), (2, 3));
    }

    #[test]
    fn equal_values_give_one_bin_centred_on_them() {
        for bins in [Bins::Auto, Bins::Count(5), Bins::Width(3.0)] {
            let b = bin([4.0, 4.0, 4.0], &bins);
            assert_eq!(
                b,
                [Bin {
                    lo: 3.5,
                    hi: 4.5,
                    count: 3
                }]
            );
        }
    }

    #[test]
    fn no_values_give_no_bins() {
        assert!(bin(std::iter::empty(), &Bins::Auto).is_empty());
        assert!(bin([f64::NAN], &Bins::Width(1.0)).is_empty());
    }

    #[test]
    fn count_asks_for_about_that_many_bins_on_a_round_step() {
        let bins = bin((0..100).map(f64::from), &Bins::Count(4)); // step 20
        assert_eq!(span(&bins), (0.0, 100.0));
        assert_eq!(bins.len(), 5);
    }

    #[test]
    fn fractional_steps_do_not_drift_and_cover_every_value() {
        for lo in [-3.7, 0.01, 0.1, 12.3, 1e-3] {
            for width in [0.003, 0.7, 1.0, 9.9, 137.0] {
                let values: Vec<f64> = (0..=200)
                    .map(|i| lo + width * f64::from(i) / 200.0)
                    .collect();
                for bins in [Bins::Auto, Bins::Count(7), Bins::Width(width / 9.0)] {
                    let out = bin(values.iter().copied(), &bins);
                    let total: usize = out.iter().map(|b| b.count).sum();
                    assert_eq!(total, values.len(), "{lo} {width} {bins:?}");
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "Bins::Width")]
    fn a_zero_width_panics() {
        bin([1.0], &Bins::Width(0.0));
    }

    #[test]
    fn quartiles_interpolate_as_numpy_does() {
        let s = box_stats([4.0, 1.0, 3.0, 2.0]).unwrap();
        assert_eq!((s.n, s.q1, s.median, s.q3), (4, 1.75, 2.5, 3.25));
        assert_eq!((s.whisker_lo, s.whisker_hi), (1.0, 4.0));
        assert!(s.outliers.is_empty());
        // np.percentile([1, 2, 3, 4, 5], [25, 50, 75]) = 2, 3, 4
        let s = box_stats((1..=5).map(f64::from)).unwrap();
        assert_eq!((s.q1, s.median, s.q3), (2.0, 3.0, 4.0));
        // np.percentile([1, 2, 4, 8, 16, 32], 25) = 2.5, 75 = 14
        let s = box_stats([1.0, 2.0, 4.0, 8.0, 16.0, 32.0]).unwrap();
        assert_eq!((s.q1, s.median, s.q3), (2.5, 6.0, 14.0));
    }

    #[test]
    fn a_far_value_is_an_outlier_and_the_whisker_stops_at_the_data() {
        let s = box_stats((1..=9).map(f64::from).chain([100.0])).unwrap();
        assert_eq!(s.outliers, [100.0]);
        assert_eq!((s.whisker_lo, s.whisker_hi), (1.0, 9.0));
        let s = box_stats([-50.0, 1.0, 2.0, 3.0, 4.0, 5.0, 60.0]).unwrap();
        assert_eq!(s.outliers, [-50.0, 60.0]);
        assert_eq!((s.whisker_lo, s.whisker_hi), (1.0, 5.0));
    }

    #[test]
    fn a_value_on_the_fence_is_not_an_outlier() {
        // q1 = 2, q3 = 4 (IQR 2): the upper fence is 7.
        let s = box_stats([0.0, 2.0, 2.0, 2.0, 4.0, 4.0, 4.0, 7.0]).unwrap();
        assert_eq!((s.q1, s.q3), (2.0, 4.0));
        assert!(s.outliers.is_empty());
        assert_eq!(s.whisker_hi, 7.0);
    }

    #[test]
    fn one_value_gives_it_for_every_statistic() {
        let s = box_stats([3.5]).unwrap();
        assert_eq!(s.n, 1);
        for v in [s.q1, s.median, s.q3, s.whisker_lo, s.whisker_hi] {
            assert_eq!(v, 3.5);
        }
        assert!(s.outliers.is_empty());
    }

    #[test]
    fn non_finite_values_are_dropped_and_nothing_gives_none() {
        let s = box_stats([1.0, f64::NAN, f64::INFINITY, 3.0]).unwrap();
        assert_eq!((s.n, s.median), (2, 2.0));
        assert!(box_stats(std::iter::empty()).is_none());
        assert!(box_stats([f64::NAN]).is_none());
    }
}
