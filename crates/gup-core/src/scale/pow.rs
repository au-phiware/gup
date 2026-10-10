// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{PositionScale, Ticks, format_with_step, step_value, tick_step};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{SCALE_POW, WgslModule};

/// A power scale: `px = r0 + (f(x) − f(d0)) · (r1 − r0) / (f(d1) − f(d0))`
/// with `f(x) = sign(x) · |x|^e`. [`Pow::sqrt`] (`e = 0.5`) sizes marks by
/// area: `Circle::RADIUS` through a square-root scale whose domain starts
/// at 0 makes a mark's area proportional to its value.
///
/// The transform keeps the sign, so domains that cross zero map
/// monotonically. The exponent must be positive and finite. Reads absolute
/// [`ColumnFormat::F32`] columns (a power is not shift invariant, so values
/// cannot be stored relative to an origin). Ticks are evenly spaced in the
/// domain, as for [`Linear`](super::Linear).
#[derive(Clone, Debug, PartialEq)]
pub struct Pow {
    exponent: f64,
    domain: Option<(f64, f64)>,
    explicit: bool,
    range: (f64, f64),
}

impl Pow {
    /// A power scale with `exponent` (positive, finite), its domain fitted
    /// to the data (and made nice).
    ///
    /// # Panics
    ///
    /// If `exponent` is not positive and finite.
    pub fn new(exponent: f64) -> Self {
        assert!(
            exponent.is_finite() && exponent > 0.0,
            "a power scale's exponent must be positive and finite, not {exponent}"
        );
        Self {
            exponent,
            domain: None,
            explicit: false,
            range: (0.0, 1.0),
        }
    }

    /// A square-root scale: `Pow::new(0.5)`.
    pub fn sqrt() -> Self {
        Self::new(0.5)
    }

    /// The exponent.
    pub fn exponent(&self) -> f64 {
        self.exponent
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
        self.domain.unwrap_or((0.0, 1.0))
    }

    /// The sign-keeping power transform.
    fn transform(&self, x: f64) -> f64 {
        if x == 0.0 {
            0.0
        } else {
            x.signum() * x.abs().powf(self.exponent)
        }
    }

    fn untransform(&self, t: f64) -> f64 {
        if t == 0.0 {
            0.0
        } else {
            t.signum() * t.abs().powf(1.0 / self.exponent)
        }
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
}

/// WGSL `gup::scale::pow::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct PowParams {
    exponent: f32,
    lo: f32,
    k: f32,
    range_start: f32,
}

impl ShaderFn for Pow {
    type In = f32;
    type Out = Px;
    type Params = PowParams;
    const MODULE: &'static WgslModule = &SCALE_POW;
    const ENTRY: &'static str = "map";

    fn params(&self) -> PowParams {
        let (lo, k) = self.parts();
        PowParams {
            exponent: self.exponent as f32,
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
                "pow scale domain",
                format!("data extent {extent:?} is not finite"),
            ));
        }
        if !self.explicit {
            self.domain = Some(extent);
        }
        Ok(())
    }
}

impl CpuMirror for Pow {
    fn eval(&self, x: f64) -> f64 {
        let (lo, k) = self.parts();
        self.range.0 + (self.transform(x) - lo) * k
    }
}

impl PositionScale for Pow {
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
                "pow scale domain",
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

    /// Extend the domain to round values, as a linear scale does.
    fn nice(&mut self) {
        let (mut lo, mut hi) = self.resolved_domain();
        if lo == hi {
            let pad = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
            (lo, hi) = (lo - pad, hi + pad);
        }
        for _ in 0..2 {
            let step = tick_step(lo, hi, 10);
            lo = (lo / step).floor() * step;
            hi = (hi / step).ceil() * step;
        }
        self.domain = Some((lo, hi));
    }

    /// Evenly spaced in the domain, as a linear scale's.
    fn ticks(&self, count: usize) -> Ticks {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let step = tick_step(lo, hi, count);
        let (start, end) = ((lo / step - 1e-9).ceil(), (hi / step + 1e-9).floor());
        let values: Vec<f64> = (0..=((end - start).max(-1.0) as i64))
            .map(|i| step_value(start + i as f64, step))
            .collect();
        let labels = values.iter().map(|&v| format_with_step(v, step)).collect();
        Ticks { values, labels }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_maps_squares_evenly() {
        let s = Pow::sqrt().domain(0.0, 100.0).range(Px(0.0), Px(10.0));
        for (x, px) in [
            (0.0, 0.0),
            (1.0, 1.0),
            (25.0, 5.0),
            (81.0, 9.0),
            (100.0, 10.0),
        ] {
            assert!((s.eval(x) - px).abs() < 1e-12, "{x} → {}", s.eval(x));
            assert!((s.invert(px) - x).abs() < 1e-9, "{px} → {}", s.invert(px));
        }
    }

    #[test]
    fn the_sign_is_kept_across_zero() {
        let s = Pow::new(2.0).domain(-2.0, 2.0).range(Px(0.0), Px(80.0));
        // f(-2) = -4, f(2) = 4: 10 px per transformed unit.
        assert_eq!(s.eval(-2.0), 0.0);
        assert_eq!(s.eval(-1.0), 30.0);
        assert_eq!(s.eval(0.0), 40.0);
        assert_eq!(s.eval(1.0), 50.0);
        assert_eq!(s.eval(2.0), 80.0);
        assert_eq!(s.invert(30.0), -1.0);
        // Monotonic through zero.
        let px: Vec<f64> = (-20..=20).map(|i| s.eval(f64::from(i) / 10.0)).collect();
        assert!(px.windows(2).all(|w| w[0] < w[1]), "{px:?}");
    }

    #[test]
    fn exponent_one_is_linear() {
        let p = Pow::new(1.0).domain(10.0, 20.0).range(Px(100.0), Px(300.0));
        let l = super::super::Linear::new()
            .domain(10.0, 20.0)
            .range(Px(100.0), Px(300.0));
        for x in [10.0, 12.5, 17.0, 20.0] {
            assert!((p.eval(x) - l.eval(x)).abs() < 1e-12);
        }
    }

    #[test]
    #[should_panic(expected = "positive and finite")]
    fn non_positive_exponents_are_refused() {
        let _ = Pow::new(0.0);
    }

    #[test]
    fn fitted_domains_nice_and_tick_like_linear() {
        let mut s = Pow::sqrt();
        s.fit_domain((3.0, 97.0)).unwrap();
        s.nice();
        assert_eq!(s.current_domain(), Some((0.0, 100.0)));
        assert_eq!(s.ticks(5).labels, vec!["0", "20", "40", "60", "80", "100"]);
        assert!(s.fit_domain((f64::NAN, 1.0)).is_err());
        assert!(s.set_domain(1.0, 1.0).is_err());
    }
}
