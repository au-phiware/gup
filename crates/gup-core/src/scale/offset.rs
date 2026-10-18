// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, Derived, ShaderFn};
use crate::shader::{SCALE_OFFSET, WgslModule};

/// Adds a pixel offset, read each time the plot resolves, to its input.
/// For chains: `band.then(Offset::by(dx))`. It has no domain.
///
/// The offset is a [`Derived`], so it can follow a scale: grouped bars
/// use it to move a bar from its band's centre to its slot
/// ([`ScaleRef::sub_band`](crate::ScaleRef)). Changing what it reads moves
/// the marks with a uniform write only.
///
/// ```
/// use gup_core::prelude::*;
///
/// let shifted = Linear::new().domain(0.0, 10.0).range(Px(0.0), Px(100.0))
///     .then(Offset::px(5.0));
/// assert_eq!(shifted.eval(10.0), 105.0);
/// ```
#[derive(Clone, Debug)]
pub struct Offset(Derived<Px>);

impl Offset {
    /// An offset read from `offset` at each resolve.
    pub fn by(offset: Derived<Px>) -> Self {
        Self(offset)
    }

    /// A fixed offset.
    pub fn px(offset: f32) -> Self {
        Self(Derived::new(move || Px(offset)))
    }

    fn get(&self) -> f32 {
        self.0.get().0
    }
}

/// WGSL `gup::scale::offset::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct OffsetParams {
    offset: f32,
    // To 16 bytes, as every uniform `Params` (see the WGSL module).
    pad_a: u32,
    pad_b: u32,
    pad_c: u32,
}

impl ShaderFn for Offset {
    type In = f32;
    type Out = Px;
    type Params = OffsetParams;
    const MODULE: &'static WgslModule = &SCALE_OFFSET;
    const ENTRY: &'static str = "map";

    fn params(&self) -> OffsetParams {
        OffsetParams {
            offset: self.get(),
            pad_a: 0,
            pad_b: 0,
            pad_c: 0,
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32
    }

    fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)> {
        let o = f64::from(self.get());
        Some((extent.0 + o, extent.1 + o))
    }
}

impl CpuMirror for Offset {
    fn eval(&self, v: f64) -> f64 {
        v + f64::from(self.get())
    }
}
