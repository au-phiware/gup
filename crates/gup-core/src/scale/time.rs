// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Linear, LinearParams, PositionScale, Ticks};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::Result;
use crate::shader::{SCALE_TIME, WgslModule};

/// A time position scale: linear over f64 seconds (Unix seconds, say),
/// with calendar ticks.
///
/// Reads [`ColumnFormat::F32x2Relative`] (hi/lo) columns, so positions stay
/// within a quarter pixel of the f64 mirror however deep the zoom: a chunk
/// of 2^20 one-per-second samples zoomed to one millisecond across 1000 px
/// is still exact (GUP-418).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Time {
    linear: Linear,
}

impl Time {
    /// A scale whose domain is fitted to the data (and made nice).
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a fixed domain (seconds) instead of fitting the data.
    pub fn domain(mut self, d0: f64, d1: f64) -> Self {
        self.linear = self.linear.domain(d0, d1);
        self
    }

    /// Set the pixel range.
    pub fn range(mut self, r0: Px, r1: Px) -> Self {
        self.linear = self.linear.range(r0, r1);
        self
    }
}

impl ShaderFn for Time {
    type In = f32;
    type Out = Px;
    /// The same layout as [`Linear`]'s: `gup::scale::time::Params`.
    type Params = LinearParams;
    const MODULE: &'static WgslModule = &SCALE_TIME;
    const ENTRY: &'static str = "map_rel";

    fn params(&self) -> LinearParams {
        self.linear.params()
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32x2Relative
    }

    /// `origin - d0`, computed in f64 (and split into hi/lo by the
    /// column format).
    fn chunk_base(&self, origin: f64) -> f64 {
        ShaderFn::chunk_base(&self.linear, origin)
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        self.linear.fit_domain(extent)
    }
}

impl CpuMirror for Time {
    fn eval(&self, x: f64) -> f64 {
        self.linear.eval(x)
    }
}

impl PositionScale for Time {
    fn current_domain(&self) -> Option<(f64, f64)> {
        self.linear.current_domain()
    }

    fn is_auto(&self) -> bool {
        self.linear.is_auto()
    }

    fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()> {
        self.linear.set_domain(d0, d1)
    }

    fn set_range(&mut self, r0: Px, r1: Px) {
        self.linear.set_range(r0, r1);
    }

    fn invert(&self, px: f64) -> f64 {
        self.linear.invert(px)
    }

    fn nice(&mut self) {
        self.linear.nice();
    }

    fn ticks(&self, count: usize) -> Ticks {
        self.linear.ticks(count)
    }
}
