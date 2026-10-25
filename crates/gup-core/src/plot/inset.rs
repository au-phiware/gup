// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data-aware edge insets of an explicit-domain axis (GUP-528).
//!
//! GUP-540 insets every explicit axis by the largest overhang of any mark at
//! both ends. Here each end is inset only by what the marks bound to the
//! axis need: a mark at normalized position `t` with half-extent `h` px must
//! stay inside the plot, which needs `near + t * span >= h` and
//! `far + (1 - t) * span >= h`.

/// The most rows decoded per resolve, shared by all layers and axes. Below
/// it edge insets are exact; above it they come from chunk statistics.
pub(crate) const EDGE_ROW_BUDGET: usize = 65_536;

/// An axis whose edge insets are being solved, as a layer sees it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct EdgeAxis {
    /// The scale's identity (`DynPositionScale::identity`): only position
    /// channels it encodes can be helped by an inset.
    pub identity: usize,
    /// The provisional pixel range, near end then far end.
    pub range: (f32, f32),
    /// The span the solver starts from; a candidate that cannot bind at
    /// that span never will.
    pub span_min: f32,
}

impl EdgeAxis {
    /// The position `px` normalized to the range: 0 at the near end, 1 at
    /// the far end. `None` for a degenerate range or a non-finite pixel.
    pub(crate) fn t(&self, px: f64) -> Option<f32> {
        let len = f64::from(self.range.1) - f64::from(self.range.0);
        let t = (px - f64::from(self.range.0)) / len;
        (len != 0.0 && t.is_finite()).then_some(t as f32)
    }

    /// Push the candidate `(t, h)` if it lies in the domain and can bind.
    pub(crate) fn push(&self, out: &mut Vec<(f32, f32)>, t: f32, h: f32) {
        let inside = (0.0..=1.0).contains(&t);
        let binds = h - t * self.span_min > 0.0 || h - (1.0 - t) * self.span_min > 0.0;
        if inside && binds && !h.is_nan() {
            out.push((t, h));
        }
    }
}

/// Near and far insets (px, the far one without padding) of an axis
/// `len` px long whose marks are `cands` as `(t, h)`; each at most `cap`.
pub(crate) fn edge_insets(cands: &[(f32, f32)], len: f32, pad: f32, cap: f32) -> (f32, f32) {
    // GUP-540's span is the smallest possible; every pass only grows it.
    let mut span = len - 2.0 * cap - pad;
    let (mut near, mut far) = (0.0f32, 0.0f32);
    for _ in 0..3 {
        near = cands
            .iter()
            .fold(0.0f32, |m, &(t, h)| m.max(h - t * span))
            .min(cap);
        far = cands
            .iter()
            .fold(0.0f32, |m, &(t, h)| m.max(h - (1.0 - t) * span))
            .min(cap);
        span = len - near - far - pad;
    }
    (near, far)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_cases() {
        let e = |c: &[(f32, f32)]| edge_insets(c, 100.0, 4.0, 10.0);
        assert_eq!(e(&[]), (0.0, 0.0));
        assert_eq!(e(&[(0.0, 5.0)]), (5.0, 0.0));
        assert_eq!(e(&[(1.0, 5.0)]), (0.0, 5.0));
        assert_eq!(e(&[(0.5, 5.0)]), (0.0, 0.0));
    }

    #[test]
    fn no_candidate_is_clipped() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) as f32) / ((1u64 << 31) as f32)
        };
        for _ in 0..1000 {
            let len = 50.0 + next() * 950.0;
            let pad = next() * 8.0;
            // GUP-540's own span must be non-negative: a plot smaller than
            // its two overhangs has no room for any inset to help.
            let cap = next() * 60.0f32.min((len - pad) / 2.0);
            let cands: Vec<(f32, f32)> = (0..(next() * 8.0) as usize)
                .map(|_| (next(), next() * cap))
                .collect();
            let (near, far) = edge_insets(&cands, len, pad, cap);
            let span = len - near - far - pad;
            assert!(near <= cap && far <= cap);
            for &(t, h) in &cands {
                assert!(near + t * span >= h - 1e-3, "near {near} {t} {h} {len}");
                assert!(
                    far + (1.0 - t) * span >= h - 1e-3,
                    "far {far} {t} {h} {len}"
                );
            }
        }
    }
}
