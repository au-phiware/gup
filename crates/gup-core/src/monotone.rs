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

/// The cubic's parameter at joint `j` (0 to 16) of a segment's pieces:
/// the Chebyshev node `(1 - cos(pi j / 16)) / 2`, as `line.wgsl`'s
/// `monotone_point`. The pieces are shortest at the rows, where a
/// monotone cubic bends most (a level tangent at an extremum), so the
/// pieces either side of a row both leave it along its tangent.
pub(crate) fn joint(j: u32) -> f32 {
    let n = crate::marks::MONOTONE_PIECES;
    match j {
        0 => 0.0,
        j if j >= n => 1.0,
        j => 0.5 - 0.5 * (std::f32::consts::PI * j as f32 / n as f32).cos(),
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
