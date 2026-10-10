// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One scale family (RFC-001 §5): every scale is a [`ShaderFn`] with an
//! exact f64 [`CpuMirror`], so axes, ticks and the GPU always agree.
//!
//! - **Position** ([`PositionScale`]): [`Linear`], [`Pow`] (and
//!   [`Pow::sqrt`]), [`Log`], [`Symlog`] and [`Time`] over numbers;
//!   [`Band`] and [`Point`] over dictionary codes (categorical axes).
//! - **Colour** ([`ColorScale`], each with a [`Legend`]): [`Sequential`]
//!   and [`Diverging`] over numbers (palette LUTs); [`Categorical`] over
//!   dictionary codes.
//!
//! Any scale with a numeric output can feed another through
//! [`then`](crate::EncodeFn::then). Scales over dictionary codes take
//! their domain from the channel's keys ([`ShaderFn::fit_keys`]).

mod categorical;
mod diverging;
mod legend;
mod linear;
mod log;
mod pow;
pub(crate) mod sequential;
mod symlog;
mod time;

#[cfg(test)]
mod conformance;

pub use categorical::{
    CATEGORICAL_COLORS, Categorical, CategoricalParams, MAX_PALETTE, NULL_COLOR, OKABE_ITO,
};
pub use diverging::{Diverging, DivergingParams};
pub use legend::{ColorScale, Legend, Ramp, RampTick, Swatch};
pub use linear::{Linear, LinearParams};
pub use log::{Log, LogParams};
pub use pow::{Pow, PowParams};
pub use sequential::{Sequential, SequentialParams};
pub use symlog::{Symlog, SymlogParams};
pub use time::Time;

use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, EncodeFn, Resource, ShaderFn};
use crate::error::Result;
use crate::shader::WgslModule;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Tick positions (domain values) and their labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ticks {
    /// Tick values in domain units, ascending.
    pub values: Vec<f64>,
    /// One label per value.
    pub labels: Vec<String>,
}

/// A scale from a numeric domain to a pixel range.
pub trait PositionScale:
    ShaderFn<In = f32, Out = Px> + EncodeFn<Input = f32, Output = Px> + CpuMirror
{
    /// The domain, if set or fitted.
    fn current_domain(&self) -> Option<(f64, f64)>;
    /// Whether the domain is data-driven (not set explicitly).
    fn is_auto(&self) -> bool;
    /// Set an explicit domain, e.g. to zoom or pan. Only uniforms derived
    /// from the domain change; no column bytes are re-uploaded.
    fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()>;
    /// Set the pixel range the domain maps onto.
    fn set_range(&mut self, r0: Px, r1: Px);
    /// The domain value drawn at pixel `px`: the inverse of
    /// [`CpuMirror::eval`].
    fn invert(&self, px: f64) -> f64;
    /// Extend the domain to round values.
    fn nice(&mut self);
    /// About `count` ticks inside the domain.
    fn ticks(&self, count: usize) -> Ticks;
}

/// A shared handle to a scale. The plot keeps one clone to draw axes and
/// set ranges; the layer's encoding keeps another to write uniforms, so
/// both always see the same domain and range.
#[derive(Debug, Default)]
pub struct ScaleRef<S>(Arc<RwLock<S>>);

impl<S> Clone for ScaleRef<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S> ScaleRef<S> {
    /// Share `scale`.
    pub fn new(scale: S) -> Self {
        Self(Arc::new(RwLock::new(scale)))
    }

    /// Read the scale.
    pub fn read(&self) -> RwLockReadGuard<'_, S> {
        self.0.read().unwrap_or_else(|p| p.into_inner())
    }

    /// Modify the scale.
    pub fn write(&self) -> RwLockWriteGuard<'_, S> {
        self.0.write().unwrap_or_else(|p| p.into_inner())
    }
}

impl<S: ShaderFn> ShaderFn for ScaleRef<S> {
    type In = S::In;
    type Out = S::Out;
    type Params = S::Params;
    const MODULE: &'static WgslModule = S::MODULE;
    const ENTRY: &'static str = S::ENTRY;

    fn params(&self) -> Self::Params {
        self.read().params()
    }

    fn input_format(&self) -> ColumnFormat {
        self.read().input_format()
    }

    fn chunk_base(&self, origin: f64) -> f64 {
        self.read().chunk_base(origin)
    }

    fn resources(&self) -> Vec<Resource> {
        self.read().resources()
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        self.write().fit_domain(extent)
    }

    fn fit_keys(&mut self, keys: &[Arc<str>]) -> Result<()> {
        self.write().fit_keys(keys)
    }

    fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)> {
        self.read().image(extent)
    }
}

impl<S> CpuMirror for ScaleRef<S>
where
    S: ShaderFn + CpuMirror<Output = <S as ShaderFn>::Out>,
{
    fn eval(&self, x: f64) -> <S::Out as crate::channel::GpuType>::Cpu {
        self.read().eval(x)
    }
}

impl<S> ColorScale for ScaleRef<S>
where
    S: ColorScale + CpuMirror<Output = <S as ShaderFn>::Out>,
{
    fn legend(&self) -> Legend {
        self.read().legend()
    }
}

/// Object-safe position-scale operations the plot needs for its x and y
/// slots.
pub(crate) trait DynPositionScale: Send + Sync {
    fn is_auto(&self) -> bool;
    fn nice(&self);
    fn set_range(&self, r0: Px, r1: Px);
    fn ticks(&self, count: usize) -> Ticks;
    fn eval(&self, x: f64) -> f64;
    fn current_domain(&self) -> Option<(f64, f64)>;
    fn set_domain(&self, d0: f64, d1: f64) -> Result<()>;
    fn invert(&self, px: f64) -> f64;

    /// Zoom by `factor` about pixel `anchor`: the domain becomes the values
    /// now drawn at the range ends pulled towards (`factor < 1`) or pushed
    /// away from (`factor > 1`) the anchor. Working in pixels and inverting
    /// makes this right for every scale (a log scale zooms in log space).
    fn zoom(&self, anchor: f64, factor: f64) -> Result<()> {
        let Some((d0, d1)) = self.current_domain() else {
            return Ok(());
        };
        let (r0, r1) = (self.eval(d0), self.eval(d1));
        let at = |r: f64| self.invert(anchor + (r - anchor) * factor);
        self.set_domain(at(r0), at(r1))
    }
}

impl<S: PositionScale> DynPositionScale for ScaleRef<S> {
    fn is_auto(&self) -> bool {
        self.read().is_auto()
    }

    fn nice(&self) {
        self.write().nice();
    }

    fn set_range(&self, r0: Px, r1: Px) {
        self.write().set_range(r0, r1);
    }

    fn ticks(&self, count: usize) -> Ticks {
        self.read().ticks(count)
    }

    fn eval(&self, x: f64) -> f64 {
        self.read().eval(x)
    }

    fn current_domain(&self) -> Option<(f64, f64)> {
        self.read().current_domain()
    }

    fn set_domain(&self, d0: f64, d1: f64) -> Result<()> {
        self.write().set_domain(d0, d1)
    }

    fn invert(&self, px: f64) -> f64 {
        self.read().invert(px)
    }
}

/// The image of `extent` under the monotonic `f`, ordered; `None` if
/// either end maps to a non-finite value.
pub(crate) fn monotonic_image(f: impl Fn(f64) -> f64, extent: (f64, f64)) -> Option<(f64, f64)> {
    let (a, b) = (f(extent.0), f(extent.1));
    (a.is_finite() && b.is_finite()).then(|| (a.min(b), a.max(b)))
}

/// A 1–2–5 step close to `(hi - lo) / count`.
pub(crate) fn tick_step(lo: f64, hi: f64, count: usize) -> f64 {
    let raw = (hi - lo).abs() / count.max(1) as f64;
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let power = 10f64.powf(raw.log10().floor());
    let err = raw / power;
    let mult = if err >= 50f64.sqrt() {
        10.0
    } else if err >= 10f64.sqrt() {
        5.0
    } else if err >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    mult * power
}

/// Multiply `i` by `step` without accumulating error for fractional steps.
pub(crate) fn step_value(i: f64, step: f64) -> f64 {
    if step < 1.0 {
        let inv = (1.0 / step).round();
        if (inv * step - 1.0).abs() < 1e-9 {
            return i / inv;
        }
    }
    i * step
}

/// Format `v` with as many decimals as `step` needs (no `-0`).
pub(crate) fn format_with_step(v: f64, step: f64) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    };
    let s = format!("{v:.decimals$}");
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s.trim_start_matches('-').to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_one_two_five() {
        assert_eq!(tick_step(0.0, 10.0, 5), 2.0);
        assert_eq!(tick_step(0.0, 100.0, 10), 10.0);
        assert_eq!(tick_step(0.0, 1.0, 4), 0.2);
        assert_eq!(tick_step(0.0, 47.0, 5), 10.0);
    }

    #[test]
    fn fractional_steps_do_not_accumulate_error() {
        assert_eq!(step_value(3.0, 0.1), 0.3);
        assert_eq!(format_with_step(0.30000000000000004, 0.1), "0.3");
        assert_eq!(format_with_step(-0.0, 0.5), "0.0");
        assert_eq!(format_with_step(2500.0, 500.0), "2500");
    }

    /// GUP-407: the bundled font is a subset, so every character a tick
    /// formatter can emit must be in it. Sweeps linear and log domains
    /// from 1e-12 to 1e15, positive, negative and straddling zero.
    #[test]
    fn tick_labels_are_covered_by_the_bundled_font() {
        let mut emitted = std::collections::BTreeSet::new();
        let mut collect = |ticks: Ticks| {
            assert!(!ticks.labels.is_empty());
            emitted.extend(ticks.labels.iter().flat_map(|l| l.chars()));
        };
        for exp in -12..=15 {
            let m = 10f64.powi(exp);
            for (d0, d1) in [(0.0, m), (-m, m), (-3.7 * m, -1.2 * m), (1.5 * m, 1.75 * m)] {
                for count in [2, 5, 10] {
                    collect(Linear::new().domain(d0, d1).ticks(count));
                }
            }
            if exp < 15 {
                for count in [3, 10, 30] {
                    collect(Log::new().domain(m, m * 1000.0).ticks(count));
                    collect(Log::new().domain(m, m * 3.0).ticks(count));
                }
            }
            for count in [2, 5, 10] {
                collect(
                    Symlog::new()
                        .constant(m)
                        .domain(-m * 1e3, m * 1e4)
                        .ticks(count),
                );
                collect(Pow::sqrt().domain(0.0, m).ticks(count));
            }
        }
        // Time: spans from a tenth of a millisecond to a century, at the
        // epoch, around 1.7e9 and before 1970.
        for exp in -4..=10 {
            let span = 10f64.powi(exp);
            for t0 in [0.0, 1.7e9 - span / 3.0, -2.2e9] {
                for count in [2, 5, 10] {
                    collect(Time::new().domain(t0, t0 + span).ticks(count));
                }
            }
        }
        // Digits, sign, point, exponent, every SI suffix and the calendar
        // labels' colon were reached (month names are checked below).
        emitted.extend("JanFebMarAprMayJunJulAugSepOctNovDec".chars());
        for c in "0123456789-.ekMGT:".chars() {
            assert!(
                emitted.contains(&c),
                "the sweep never emitted {c:?}: {emitted:?}"
            );
        }
        let font = gup_text::Font::inter();
        let missing: Vec<char> = emitted
            .iter()
            .copied()
            .filter(|&c| !font.has_glyph(c))
            .collect();
        assert!(missing.is_empty(), "the bundled font lacks {missing:?}");
    }
}
