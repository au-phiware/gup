// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One scale family (RFC-001 §5): every scale is a [`ShaderFn`] with an
//! exact f64 [`CpuMirror`], so axes, ticks and the GPU always agree.
//!
//! S0a implements [`Linear`], [`Log`] and [`Sequential`]; the rest of the
//! family (Pow/Sqrt, Symlog, Time, Band, Point, Diverging, Categorical) and
//! `then` composition are RFC-001 S5.

mod linear;
mod log;
mod sequential;

pub use linear::{Linear, LinearParams};
pub use log::{Log, LogParams};
pub use sequential::{Sequential, SequentialParams};

use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, Resource, ShaderFn};
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
pub trait PositionScale: ShaderFn<In = f32, Out = Px> + CpuMirror {
    /// The domain, if set or fitted.
    fn current_domain(&self) -> Option<(f64, f64)>;
    /// Whether the domain is data-driven (not set explicitly).
    fn is_auto(&self) -> bool;
    /// Set the pixel range the domain maps onto.
    fn set_range(&mut self, r0: Px, r1: Px);
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

    fn chunk_base(&self, origin: f64) -> f32 {
        self.read().chunk_base(origin)
    }

    fn resources(&self) -> Vec<Resource> {
        self.read().resources()
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        self.write().fit_domain(extent)
    }
}

impl<S: CpuMirror> CpuMirror for ScaleRef<S> {
    fn eval(&self, x: f64) -> <S::Out as crate::channel::GpuType>::Cpu {
        self.read().eval(x)
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
}
