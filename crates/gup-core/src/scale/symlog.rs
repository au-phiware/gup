// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::log::format_log_tick;
use super::{PositionScale, Ticks, format_with_step, step_value, tick_step};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{SCALE_SYMLOG, WgslModule};

/// A symmetric log scale, for signed data spanning decades:
/// `f(x) = sign(x) · log2(1 + |x| / c)`, mapped linearly onto the range.
///
/// It is linear through zero (for `|x|` well below the constant `c`) and
/// logarithmic beyond it, and continuous and smooth everywhere, so data on
/// both sides of zero has no gap and no jump at the threshold. Reads
/// absolute [`ColumnFormat::F32`] columns, like [`Log`](super::Log), whose
/// companion it is (RFC-001 §5: "`Log` (with `Symlog`)"); it has its own
/// WGSL module so `Log`'s parameters and generated glue are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct Symlog {
    constant: f64,
    domain: Option<(f64, f64)>,
    explicit: bool,
    range: (f64, f64),
}

impl Default for Symlog {
    fn default() -> Self {
        Self::new()
    }
}

impl Symlog {
    /// A symmetric log scale with constant 1, its domain fitted to the
    /// data (and made nice).
    pub fn new() -> Self {
        Self {
            constant: 1.0,
            domain: None,
            explicit: false,
            range: (0.0, 1.0),
        }
    }

    /// Set the linear threshold `c` (positive, finite): values within
    /// about `±c` are spaced linearly, values beyond logarithmically.
    ///
    /// # Panics
    ///
    /// If `c` is not positive and finite.
    pub fn constant(mut self, c: f64) -> Self {
        assert!(
            c.is_finite() && c > 0.0,
            "a symlog scale's constant must be positive and finite, not {c}"
        );
        self.constant = c;
        self
    }

    /// Use a fixed domain instead of fitting the data.
    pub fn domain(mut self, d0: f64, d1: f64) -> Self {
        self.domain = Some((d0, d1));
        self.explicit = true;
        self
    }

    /// Set the pixel range.
    pub fn range(mut self, r0: Px, r1: Px) -> Self {
        self.set_range(r0, r1);
        self
    }

    fn resolved_domain(&self) -> (f64, f64) {
        self.domain.unwrap_or((-self.constant, self.constant))
    }

    fn transform(&self, x: f64) -> f64 {
        x.signum() * (x.abs() / self.constant).ln_1p() / std::f64::consts::LN_2
    }

    fn untransform(&self, t: f64) -> f64 {
        t.signum() * self.constant * (t.abs() * std::f64::consts::LN_2).exp_m1()
    }

    /// `(f(d0), pixels per transformed unit)` (0 for an empty domain).
    fn parts(&self) -> (f64, f64) {
        let (d0, d1) = self.resolved_domain();
        let (t0, t1) = (self.transform(d0), self.transform(d1));
        let k = if t1 == t0 {
            0.0
        } else {
            (self.range.1 - self.range.0) / (t1 - t0)
        };
        (t0, k)
    }

    /// Whether the domain reaches far enough past the constant to tick in
    /// decades.
    fn logarithmic(&self, lo: f64, hi: f64) -> bool {
        lo.abs().max(hi.abs()) > 10.0 * self.constant
    }
}

/// WGSL `gup::scale::symlog::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct SymlogParams {
    inv_c: f32,
    lo: f32,
    k: f32,
    range_start: f32,
}

impl ShaderFn for Symlog {
    type In = f32;
    type Out = Px;
    type Params = SymlogParams;
    const MODULE: &'static WgslModule = &SCALE_SYMLOG;
    const ENTRY: &'static str = "map";

    fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)> {
        super::monotonic_image(|x| self.eval(x), extent)
    }

    fn params(&self) -> SymlogParams {
        let (lo, k) = self.parts();
        SymlogParams {
            inv_c: (1.0 / self.constant) as f32,
            lo: lo as f32,
            k: k as f32,
            range_start: self.range.0 as f32,
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        if !extent.0.is_finite() || !extent.1.is_finite() {
            return Err(Error::config(
                "symlog scale domain",
                format!("data extent {extent:?} is not finite"),
            ));
        }
        if !self.explicit {
            self.domain = Some(extent);
        }
        Ok(())
    }
}

impl CpuMirror for Symlog {
    fn eval(&self, x: f64) -> f64 {
        let (lo, k) = self.parts();
        self.range.0 + (self.transform(x) - lo) * k
    }
}

impl PositionScale for Symlog {
    fn current_domain(&self) -> Option<(f64, f64)> {
        self.domain
    }

    fn is_auto(&self) -> bool {
        !self.explicit
    }

    fn set_range(&mut self, r0: Px, r1: Px) {
        self.range = (f64::from(r0.0), f64::from(r1.0));
    }

    fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()> {
        if !(d0.is_finite() && d1.is_finite()) || d0 == d1 {
            return Err(Error::config(
                "symlog scale domain",
                format!("({d0}, {d1}) must be finite and non-empty"),
            ));
        }
        self.domain = Some((d0, d1));
        self.explicit = true;
        Ok(())
    }

    fn invert(&self, px: f64) -> f64 {
        let (lo, k) = self.parts();
        if k == 0.0 {
            return self.resolved_domain().0;
        }
        self.untransform(lo + (px - self.range.0) / k)
    }

    /// Extend each end to a power of ten (keeping its sign; zero stays
    /// zero) once the domain reaches past ten times the constant, else to
    /// round values as a linear scale does.
    fn nice(&mut self) {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let (lo, hi) = if self.logarithmic(lo, hi) {
            let out = |v: f64, up: bool| {
                if v == 0.0 {
                    return 0.0;
                }
                let away = (v > 0.0) == up;
                let e = v.abs().log10();
                let e = if away {
                    (e - 1e-9).ceil()
                } else {
                    (e + 1e-9).floor()
                };
                v.signum() * 10f64.powf(e)
            };
            (out(lo, false), out(hi, true))
        } else {
            let (mut lo, mut hi) = (lo, hi);
            if lo == hi {
                (lo, hi) = (lo - self.constant, hi + self.constant);
            }
            for _ in 0..2 {
                let step = tick_step(lo, hi, 10);
                lo = (lo / step).floor() * step;
                hi = (hi / step).ceil() * step;
            }
            (lo, hi)
        };
        self.domain = Some(if d0 <= d1 { (lo, hi) } else { (hi, lo) });
    }

    /// Zero and signed powers of ten (with 2× and 5× multiples when there
    /// are few decades) once the domain reaches past ten times the
    /// constant; evenly spaced ticks inside it.
    fn ticks(&self, count: usize) -> Ticks {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let in_domain = |v: f64| v >= lo - 1e-9 * lo.abs() && v <= hi + 1e-9 * hi.abs();
        if !self.logarithmic(lo, hi) {
            let step = tick_step(lo, hi, count);
            let (start, end) = ((lo / step - 1e-9).ceil(), (hi / step + 1e-9).floor());
            let values: Vec<f64> = (0..=((end - start).max(-1.0) as i64))
                .map(|i| step_value(start + i as f64, step))
                .collect();
            let labels = values.iter().map(|&v| format_with_step(v, step)).collect();
            return Ticks { values, labels };
        }
        // Decades from the first power of ten past about 3c (so the ticks
        // nearest zero sit at least two transformed units from it, as far
        // as later decades sit from each other) to the largest |end|.
        let k0 = (self.constant.log10() + 0.5).ceil() as i32;
        let k1 = (lo.abs().max(hi.abs()).log10() + 1e-9).floor() as i32;
        let signed = |mults: &[f64]| {
            let mut v: Vec<f64> = (k0..=k1)
                .flat_map(|k| mults.iter().map(move |m| m * 10f64.powi(k)))
                .flat_map(|m| [m, -m])
                .chain(std::iter::once(0.0))
                .filter(|&v| in_domain(v))
                .collect();
            v.sort_by(f64::total_cmp);
            v.dedup();
            v
        };
        let mut values = signed(&[1.0]);
        if values.len() < count {
            let more = signed(&[1.0, 2.0, 5.0]);
            if more.len() <= count + count / 2 {
                values = more;
            }
        }
        // Far too many decades: keep every n-th power on each side of zero.
        let n = (values.len() as f64 / (1.5 * count.max(2) as f64)).ceil() as usize;
        if n > 1 {
            values.retain(|&v| {
                v == 0.0 || (v.abs().log10().round() as i32 - k0).rem_euclid(n as i32) == 0
            });
        }
        let labels = values
            .iter()
            .map(|&v| match v {
                0.0 => "0".to_string(),
                v if v < 0.0 => format!("{}{}", super::MINUS, format_log_tick(-v)),
                v => format_log_tick(v),
            })
            .collect();
        Ticks { values, labels }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_near_zero_logarithmic_beyond() {
        let s = Symlog::new()
            .constant(1.0)
            .domain(-1e6, 1e6)
            .range(Px(0.0), Px(1000.0));
        // Odd and centred.
        assert!((s.eval(0.0) - 500.0).abs() < 1e-9);
        for x in [0.01, 0.5, 3.0, 1e3, 9e5] {
            assert!(
                (s.eval(x) - 500.0 + (s.eval(-x) - 500.0)).abs() < 1e-9,
                "{x}"
            );
        }
        // Near zero, equal steps are equal pixels (to within 1%).
        let (a, b) = (s.eval(0.01) - s.eval(0.0), s.eval(0.02) - s.eval(0.01));
        assert!((a / b - 1.0).abs() < 0.01, "{a} {b}");
        // Far out, each decade takes the same pixels.
        let (c, d) = (s.eval(1e4) - s.eval(1e3), s.eval(1e5) - s.eval(1e4));
        assert!((c / d - 1.0).abs() < 1e-3, "{c} {d}");
        // Strictly increasing through the threshold.
        let px: Vec<f64> = (-300..=300).map(|i| s.eval(f64::from(i) / 100.0)).collect();
        assert!(px.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn invert_undoes_eval() {
        let s = Symlog::new()
            .constant(10.0)
            .domain(-5e4, 2e3)
            .range(Px(400.0), Px(0.0));
        for x in [-5e4, -123.0, -10.0, -0.5, 0.0, 3.0, 10.0, 999.0, 2e3] {
            let back = s.invert(s.eval(x));
            assert!((back - x).abs() <= 1e-9 * x.abs().max(1.0), "{x} → {back}");
        }
    }

    #[test]
    fn decade_ticks_cross_zero() {
        let s = Symlog::new().constant(1.0).domain(-1e4, 1e4);
        let t = s.ticks(10);
        assert_eq!(
            t.labels,
            vec!["−10k", "−1k", "−100", "−10", "0", "10", "100", "1k", "10k"]
        );
        // Few decades get 2× and 5×; a wide domain thins to every other.
        let few = Symlog::new().constant(1.0).domain(-50.0, 50.0).ticks(10);
        assert!(few.labels.contains(&"−20".to_string()), "{:?}", few.labels);
        let wide = Symlog::new().constant(1.0).domain(-1e12, 1e12).ticks(8);
        assert!(wide.values.len() <= 13, "{:?}", wide.labels);
        assert!(wide.values.contains(&0.0));
        // Inside the linear region: evenly spaced.
        let lin = Symlog::new().constant(10.0).domain(-4.0, 4.0).ticks(4);
        assert_eq!(lin.labels, vec!["−4", "−2", "0", "2", "4"]);
    }

    #[test]
    fn nice_extends_to_signed_powers_of_ten() {
        let mut s = Symlog::new();
        s.fit_domain((-370.0, 8_200.0)).unwrap();
        s.nice();
        assert_eq!(s.current_domain(), Some((-1_000.0, 10_000.0)));
        let mut z = Symlog::new();
        z.fit_domain((0.0, 4_200.0)).unwrap();
        z.nice();
        assert_eq!(z.current_domain(), Some((0.0, 10_000.0)));
        let mut lin = Symlog::new().constant(100.0);
        lin.fit_domain((-3.7, 9.2)).unwrap();
        lin.nice();
        assert_eq!(lin.current_domain(), Some((-4.0, 10.0)));
    }
}
