// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Monotone cubic interpolation in x (GUP-551): the control values of
//! [`Curve::MonotoneX`](crate::Curve::MonotoneX).
//!
//! Each segment from row `i` to row `i + 1` is a cubic Bezier whose
//! control points lie at one and two thirds of the segment in x, with
//! y values `c1_i = y_i + m_i h_i / 3` and `c2_i = y_{i+1} − m_{i+1} h_i / 3`
//! (`h_i` the x gap, `m` the tangents). The tangents are Steffen's ("A
//! simple method for monotonic interpolation in one dimension", 1990),
//! computed in `f64` per **run**: a maximal sequence of rows with finite x
//! and y whose x strictly increases. Steffen's tangents keep every
//! segment's curve monotone and inside the segment's y range, so both
//! control values lie between the segment's end values.
//!
//! A segment whose x does not increase (`x_{i+1} <= x_i`) is straight: its
//! control values lie at one and two thirds of its y change. A row with no
//! segment (the next row is null, or there is none) stores its own y as
//! both control values, so a control is finite exactly when its row's y
//! is, and the column store records no null the y column does not.

/// Steffen's tangent at the first row of a run, from its first two
/// secants `s0`, `s1` and gaps `h0`, `h1`: the slope at the first point of
/// the parabola through the first three points, limited so the end
/// segment stays monotone (zero when it disagrees with `s0` in sign, at
/// most `2 s0`).
fn end_tangent(s0: f64, s1: f64, h0: f64, h1: f64) -> f64 {
    let p = s0 * (1.0 + h0 / (h0 + h1)) - s1 * h0 / (h0 + h1);
    if p * s0 <= 0.0 {
        0.0
    } else if p.abs() > 2.0 * s0.abs() {
        2.0 * s0
    } else {
        p
    }
}

/// `-1`, `0` or `1`.
fn sign(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Steffen's tangents of one run of `x`, `y` (at least two rows, finite,
/// x strictly increasing).
pub(crate) fn tangents(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len();
    debug_assert!(n >= 2 && y.len() == n);
    let h: Vec<f64> = x.windows(2).map(|w| w[1] - w[0]).collect();
    let s: Vec<f64> = (0..n - 1).map(|i| (y[i + 1] - y[i]) / h[i]).collect();
    if n == 2 {
        // Two rows: straight.
        return vec![s[0], s[0]];
    }
    let mut m = Vec::with_capacity(n);
    m.push(end_tangent(s[0], s[1], h[0], h[1]));
    for i in 1..n - 1 {
        let p = (s[i - 1] * h[i] + s[i] * h[i - 1]) / (h[i - 1] + h[i]);
        let limit = s[i - 1].abs().min(s[i].abs()).min(0.5 * p.abs());
        m.push((sign(s[i - 1]) + sign(s[i])) * limit);
    }
    m.push(end_tangent(s[n - 2], s[n - 3], h[n - 2], h[n - 3]));
    m
}

/// The control values of every row of `x`, `y` (data units, data order):
/// `(c1, c2)`, both belonging to the segment from each row to the next.
/// See the [module docs](self).
pub(crate) fn controls(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len().min(y.len());
    // Without a segment: the row's own y (NaN for a null y).
    let mut c1: Vec<f64> = y[..n].to_vec();
    let mut c2 = c1.clone();
    let valid = |i: usize| x[i].is_finite() && y[i].is_finite();
    let mut i = 0;
    while i < n {
        if !valid(i) {
            i += 1;
            continue;
        }
        // The run from `i`: valid rows whose x strictly increases.
        let mut j = i + 1;
        while j < n && valid(j) && x[j] > x[j - 1] {
            j += 1;
        }
        if j - i >= 2 {
            let m = tangents(&x[i..j], &y[i..j]);
            for k in i..j - 1 {
                let h = x[k + 1] - x[k];
                c1[k] = y[k] + m[k - i] * h / 3.0;
                c2[k] = y[k + 1] - m[k + 1 - i] * h / 3.0;
            }
        }
        // A valid next row ends the run by not increasing x: a straight
        // segment to it.
        if j < n && valid(j) {
            let dy = y[j] - y[j - 1];
            c1[j - 1] = y[j - 1] + dy / 3.0;
            c2[j - 1] = y[j - 1] + 2.0 * dy / 3.0;
        }
        i = j;
    }
    (c1, c2)
}

/// The cubic's parameter at joint `j` (0 to `n`) of a segment drawn as
/// `n` pieces: the Chebyshev node `(1 - cos(pi j / n)) / 2`, as
/// `line.wgsl`'s `monotone_point`. The pieces are shortest at the rows,
/// where a monotone cubic bends most (a level tangent at an extremum),
/// so the pieces either side of a row both leave it along its tangent.
pub(crate) fn joint(j: u32, n: u32) -> f32 {
    match j {
        0 => 0.0,
        j if j >= n => 1.0,
        j => 0.5 - 0.5 * (std::f32::consts::PI * j as f32 / n as f32).cos(),
    }
}

/// The most ink a segment's pieces may misplace against its cubic
/// (GUP-576): an eighth of a pixel's coverage, as a pixel-wide stretch
/// of the line drawn an eighth of a pixel off.
const TOLERANCE: f64 = 0.125;

/// The ink `n` pieces misplace on a segment `width` logical pixels wide,
/// as a fraction of a pixel's coverage. The pieces, at the Chebyshev
/// nodes, stray from the cubic by about `width / n²` (GUP-551 measured
/// 0.19 px for 16 pieces and 0.68 px for 8 at the sharpest knee of a
/// 50 px segment; a monotone segment stays inside its rows' box, so one
/// piece strays at most `width`). Across a segment narrower than a pixel
/// that offset spans only `width` of a pixel's width, so it moves at
/// most `width` of the ink an offset across the whole pixel would.
fn misplaced(width: f64, n: u32) -> f64 {
    let n = f64::from(n);
    width / (n * n) * width.min(1.0)
}

/// The pieces a monotone segment `width` logical pixels wide is drawn as
/// (GUP-576): the fewest, a power of two from 1 to
/// [`MAX_MONOTONE_PIECES`](crate::marks::MAX_MONOTONE_PIECES), that
/// misplace at most [`TOLERANCE`] of a pixel's ink ([`misplaced`]). So a
/// segment wider than 8 px is drawn as 16 pieces, as GUP-551 drew every
/// segment, and one narrower than 0.35 px (4600 rows or more across
/// 1600 px) as one, at about the cost of a straight segment.
/// Powers of two keep the pipelines a layer links while zooming to five.
/// A width that is not a number (no CPU map of the x scale) draws the
/// most.
pub(crate) fn pieces_for(width: f64) -> u32 {
    let most = crate::marks::MAX_MONOTONE_PIECES;
    if width.is_nan() {
        return most;
    }
    let mut n = 1;
    while n < most && misplaced(width, n) > TOLERANCE {
        n *= 2;
    }
    n
}

/// A monotone line's segment widths, in data units, summarised for
/// choosing its pieces (GUP-576): in buckets of a quarter octave of
/// width, the summed width of the segments in each and the x of the
/// first one's rows. Appends add to it; it holds a few words per bucket,
/// whatever the rows.
#[derive(Clone, Debug, Default)]
pub(crate) struct Widths {
    buckets: std::collections::BTreeMap<i32, (f64, [f64; 2])>,
    total: f64,
}

impl Widths {
    /// Add the curved segments between consecutive rows of `x`, `y`:
    /// finite rows whose x increases (any other segment is drawn
    /// straight, whatever the count).
    pub(crate) fn add(&mut self, x: &[f64], y: &[f64]) {
        let n = x.len().min(y.len());
        for i in 1..n {
            let (a, b) = (x[i - 1], x[i]);
            let finite = a.is_finite() && b.is_finite() && y[i - 1].is_finite() && y[i].is_finite();
            if !finite || b <= a {
                continue;
            }
            let w = b - a;
            let key = (w.log2() * 4.0).floor() as i32;
            self.buckets.entry(key).or_insert((0.0, [a, b])).0 += w;
            self.total += w;
        }
    }

    /// The x of the rows of a segment at the width-weighted median: the
    /// segments in narrower buckets span less than half the curve's
    /// extent in x, and those in wider ones at most half. Under an affine
    /// x scale (linear or time) its pixel width is that median at every
    /// zoom. A few wide gaps among dense rows do not raise it; a gap
    /// spanning most of the chart does. `None` without a curved segment.
    pub(crate) fn median(&self) -> Option<[f64; 2]> {
        let mut sum = 0.0;
        for &(w, segment) in self.buckets.values() {
            sum += w;
            if sum >= self.total * 0.5 {
                return Some(segment);
            }
        }
        None
    }
}

/// The cubic Bezier through `p0`..`p3` at `t`, evaluated as `line.wgsl`'s
/// `bezier` does.
pub(crate) fn bezier(p: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [0, 1].map(|k| b0 * p[0][k] + b1 * p[1][k] + b2 * p[2][k] + b3 * p[3][k])
}

/// The control points of the segment from `a` to `b` (pixels) whose
/// control values are `ctrl` (pixels): at one and two thirds of the
/// segment in x.
pub(crate) fn control_points(a: [f32; 2], b: [f32; 2], ctrl: [f32; 2]) -> [[f32; 2]; 4] {
    let dx = b[0] - a[0];
    [
        a,
        [a[0] + dx / 3.0, ctrl[0]],
        [a[0] + dx * 2.0 / 3.0, ctrl[1]],
        b,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cubic of segment `i` at `t`, in data units.
    fn at(x: &[f64], y: &[f64], c: &(Vec<f64>, Vec<f64>), i: usize, t: f64) -> (f64, f64) {
        let u = 1.0 - t;
        let px = x[i] + (x[i + 1] - x[i]) * t;
        let py = u * u * u * y[i]
            + 3.0 * u * u * t * c.0[i]
            + 3.0 * u * t * t * c.1[i]
            + t * t * t * y[i + 1];
        (px, py)
    }

    /// Whether every segment's curve is monotone and inside its y range,
    /// sampled at 200 points.
    fn monotone_between_rows(x: &[f64], y: &[f64]) -> bool {
        let c = controls(x, y);
        (0..x.len() - 1).all(|i| {
            let (lo, hi) = (y[i].min(y[i + 1]), y[i].max(y[i + 1]));
            let up = y[i + 1] >= y[i];
            let mut last = y[i];
            (1..=200).all(|k| {
                let (_, v) = at(x, y, &c, i, f64::from(k) / 200.0);
                let ordered = if up {
                    v >= last - 1e-9
                } else {
                    v <= last + 1e-9
                };
                let ok = v >= lo - 1e-9 && v <= hi + 1e-9 && ordered;
                last = v;
                ok
            })
        })
    }

    #[test]
    fn monotone_data_gives_a_monotone_interpolant() {
        let x = [0.0, 1.0, 1.5, 4.0, 5.0, 9.0, 9.2];
        let y = [0.0, 0.1, 3.0, 3.2, 8.0, 8.1, 20.0];
        assert!(monotone_between_rows(&x, &y));
        let (c1, c2) = controls(&x, &y);
        for i in 0..x.len() - 1 {
            for c in [c1[i], c2[i]] {
                assert!((y[i]..=y[i + 1]).contains(&c), "segment {i}: {c}");
            }
        }
        // Decreasing too.
        let down: Vec<f64> = y.iter().map(|v| -v).collect();
        assert!(monotone_between_rows(&x, &down));
    }

    #[test]
    fn noisy_data_never_overshoots() {
        let x: Vec<f64> = (0..40)
            .map(|i| f64::from(i) * 1.3 + f64::from(i % 3))
            .collect();
        let y: Vec<f64> = (0..40)
            .map(|i| (f64::from(i) * 0.7).sin() * 10.0 + f64::from(i % 5))
            .collect();
        assert!(monotone_between_rows(&x, &y));
    }

    #[test]
    fn a_flat_run_has_zero_tangents() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [2.0, 2.0, 2.0, 2.0];
        assert_eq!(tangents(&x, &y), vec![0.0; 4]);
        let (c1, c2) = controls(&x, &y);
        assert_eq!(c1[..3], [2.0; 3]);
        assert_eq!(c2[..3], [2.0; 3]);
        // A flat stretch inside a rise: zero at its rows.
        let m = tangents(&[0.0, 1.0, 2.0, 3.0, 4.0], &[0.0, 1.0, 1.0, 1.0, 2.0]);
        assert_eq!((m[1], m[2], m[3]), (0.0, 0.0, 0.0));
    }

    #[test]
    fn a_local_extremum_has_a_zero_tangent() {
        let m = tangents(&[0.0, 1.0, 2.0, 3.0, 4.0], &[0.0, 3.0, 5.0, 1.0, 0.5]);
        assert_eq!(m[2], 0.0, "{m:?}");
        assert!(m[1] > 0.0 && m[3] < 0.0, "{m:?}");
        let m = tangents(&[0.0, 2.0, 3.0], &[4.0, 1.0, 2.0]);
        assert_eq!(m[1], 0.0, "{m:?}");
    }

    #[test]
    fn two_row_runs_are_straight() {
        let (x, y) = ([1.0, 4.0], [2.0, 8.0]);
        assert_eq!(tangents(&x, &y), vec![2.0, 2.0]);
        let (c1, c2) = controls(&x, &y);
        assert_eq!((c1[0], c2[0]), (4.0, 6.0));
        // The last row has no segment: its own y.
        assert_eq!((c1[1], c2[1]), (8.0, 8.0));
    }

    #[test]
    fn interior_tangents_follow_steffen() {
        // h = (1, 2), s = (2, 0.5): p = (2·2 + 0.5·1) / 3 = 1.5, limited by
        // min(|s|) = 0.5 and |p| / 2 = 0.75.
        let m = tangents(&[0.0, 1.0, 3.0], &[0.0, 2.0, 3.0]);
        assert_eq!(m[1], 2.0 * 0.5);
        // End: p0 = s0 (1 + h0 / (h0 + h1)) - s1 h0 / (h0 + h1)
        //         = 2 · 4/3 - 0.5 / 3 = 2.5, within 2 s0 = 4.
        assert!((m[0] - 2.5).abs() < 1e-12, "{m:?}");
        // The other end, mirrored: p = 0.5 (1 + 2/3) - 2 · 2/3 < 0, so 0.
        assert_eq!(m[2], 0.0);
    }

    #[test]
    fn non_increasing_x_draws_straight_and_restarts() {
        let x = [0.0, 1.0, 2.0, 2.0, 3.0, 4.0];
        let y = [0.0, 1.0, 3.0, 6.0, 7.0, 9.0];
        let (c1, c2) = controls(&x, &y);
        // Segment 2 (x 2 → 2) is straight: thirds of its y change.
        assert_eq!((c1[2], c2[2]), (4.0, 5.0));
        // The runs either side are computed alone: rows 0-2 and 3-5.
        let left = tangents(&x[..3], &y[..3]);
        let right = tangents(&x[3..], &y[3..]);
        assert_eq!(c2[1], y[2] - left[2] / 3.0);
        assert_eq!(c1[3], y[3] + right[0] / 3.0);
    }

    #[test]
    fn nulls_break_runs_and_restart_tangents() {
        let x = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let y = [0.0, 2.0, 1.0, f64::NAN, 5.0, 4.0, 6.0];
        let (c1, c2) = controls(&x, &y);
        // No segment from row 2 or row 3: their own y.
        assert_eq!((c1[2], c2[2]), (1.0, 1.0));
        assert!(c1[3].is_nan() && c2[3].is_nan());
        // The second run restarts: its first tangent is an end tangent.
        let m = tangents(&x[4..], &y[4..]);
        assert_eq!(c1[4], 5.0 + m[0] / 3.0);
        let (a1, a2) = controls(&x[4..], &y[4..]);
        assert_eq!((&c1[4..], &c2[4..]), (&a1[..], &a2[..]));
    }

    #[test]
    fn controls_of_a_window_match_for_rows_two_past_its_start() {
        // What an append relies on: a row's tangent depends on the rows
        // within two of it, so controls computed from three rows back match
        // the whole for every row after the first.
        let x: Vec<f64> = (0..30).map(f64::from).collect();
        let y: Vec<f64> = (0..30).map(|i| (f64::from(i) * 0.9).cos() * 4.0).collect();
        let (c1, c2) = controls(&x, &y);
        for start in 0..25 {
            let (w1, w2) = controls(&x[start..], &y[start..]);
            for k in 1..w1.len() {
                assert_eq!(w1[k], c1[start + k], "c1 at {}", start + k);
                assert_eq!(w2[k], c2[start + k], "c2 at {}", start + k);
            }
        }
    }

    /// GUP-576: the count rule. 16 pieces for the golden's ~50 px
    /// segments, one for 100K rows in 1600 px, powers of two between.
    #[test]
    fn pieces_follow_pixel_width() {
        let cases = [
            (0.0, 1),
            (0.016, 1),
            (0.35, 1),
            (0.36, 2),
            (0.7, 2),
            (0.71, 4),
            (2.0, 4),
            (2.1, 8),
            (8.0, 8),
            (8.1, 16),
            (50.0, 16),
            (f64::INFINITY, 16),
            (f64::NAN, 16),
        ];
        for (w, n) in cases {
            assert_eq!(pieces_for(w), n, "width {w}");
        }
        // Each count keeps the modelled misplaced ink in tolerance (below
        // 16), and half the count would not.
        for k in 1..400 {
            let w = f64::from(k) * 0.05;
            let n = pieces_for(w);
            assert!(n == 16 || misplaced(w, n) <= TOLERANCE, "{w}");
            assert!(n == 1 || misplaced(w, n / 2) > TOLERANCE, "{w}");
        }
        // Over a pixel wide, the offset itself.
        assert_eq!(misplaced(8.0, 8), 0.125);
        assert_eq!(misplaced(0.5, 1), 0.25);
    }

    /// The joints at every count: the ends exactly, the Chebyshev nodes
    /// between, increasing and symmetric about the middle.
    #[test]
    fn joints_are_chebyshev_nodes_at_every_count() {
        for n in [1, 2, 4, 8, 16] {
            assert_eq!((joint(0, n), joint(n, n)), (0.0, 1.0));
            for j in 1..n {
                let t = joint(j, n);
                assert!((t + joint(n - j, n) - 1.0).abs() < 1e-6, "{j}/{n}");
                assert!(t > joint(j - 1, n), "{j}/{n}");
            }
        }
        assert!((joint(1, 2) - 0.5).abs() < 1e-6);
    }

    /// The width-weighted median: uniform rows give their width; a few
    /// gaps among dense rows do not count, a gap spanning most of the
    /// extent does; straight and null segments are left out; appends add.
    #[test]
    fn widths_take_the_width_weighted_median() {
        let x: Vec<f64> = (0..1000).map(f64::from).collect();
        let mut w = Widths::default();
        w.add(&x, &vec![1.0; x.len()]);
        assert_eq!(w.median().map(|s| s[1] - s[0]), Some(1.0));
        // Three gaps of 50: 150 of 1149 units.
        let mut gaps = x.clone();
        gaps.extend([1049.0, 1099.0, 1149.0]);
        let mut w = Widths::default();
        w.add(&gaps, &vec![1.0; gaps.len()]);
        assert_eq!(w.median().map(|s| s[1] - s[0]), Some(1.0));
        // Appending a gap of 5000 (after the last stored row).
        w.add(&[1149.0, 6149.0], &[1.0, 1.0]);
        assert_eq!(w.median(), Some([1149.0, 6149.0]));
        // Nulls and x that does not increase are drawn straight.
        let mut w = Widths::default();
        w.add(
            &[0.0, 10.0, 10.0, 5.0, 7.0, 8.0],
            &[1.0, f64::NAN, 2.0, 2.0, 2.0, 2.0],
        );
        assert_eq!(w.median(), Some([5.0, 7.0]));
        assert_eq!(Widths::default().median(), None);
    }

    #[test]
    fn bezier_hits_its_ends_and_thirds_are_straight() {
        let (a, b) = ([10.0, 20.0], [40.0, 80.0]);
        let p = control_points(a, b, [40.0, 60.0]);
        assert_eq!(bezier(p, 0.0), a);
        assert_eq!(bezier(p, 1.0), b);
        // Controls at thirds of the y change: the straight segment.
        let mid = bezier(p, 0.5);
        assert!((mid[0] - 25.0).abs() < 1e-4 && (mid[1] - 50.0).abs() < 1e-4);
    }
}
