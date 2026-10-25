// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data transforms that run before drawing: [`bin`] counts values into
//! [`Bin`]s, the rows of a histogram.
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
}
