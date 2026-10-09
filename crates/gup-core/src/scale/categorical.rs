// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::channel::Color;
use crate::column::{ColumnFormat, NULL_CODE};
use crate::encoding::{CpuMirror, ShaderFn};
use crate::shader::{COLOR_CATEGORICAL, WgslModule};

/// The colour of a null: a missing key in a dictionary column or a
/// non-finite value driving a colour channel. Neutral grey `#999999`,
/// distinct from every [`OKABE_ITO`] colour.
///
/// A fixed constant until RFC-001 S7's `Theme` makes it configurable as
/// `theme.null_color`.
pub const NULL_COLOR: Color = Color::hex(0x999999);

/// The Okabe-Ito colour-blind-safe palette, black last: orange, sky blue,
/// bluish green, yellow, blue, vermillion, reddish purple, black.
pub const OKABE_ITO: [Color; 8] = [
    Color::hex(0xe69f00),
    Color::hex(0x56b4e9),
    Color::hex(0x009e73),
    Color::hex(0xf0e442),
    Color::hex(0x0072b2),
    Color::hex(0xd55e00),
    Color::hex(0xcc79a7),
    Color::hex(0x000000),
];

/// A categorical colour scale: dictionary codes onto a palette of up to 8
/// colours, cycling past its end, with [`NULL_COLOR`] for a missing key.
///
/// Drive it with [`ShaderFn::encode_key`] (or
/// [`encode_nullable_key`](ShaderFn::encode_nullable_key)): each distinct
/// key gets the next code in first-seen order, so the first key seen draws
/// in the first palette colour.
///
/// This is RFC-001 S4b's minimal stand-in, enough to carry a dictionary
/// column to the screen. S5's full scale family (explicit domains, domain
/// growth, legends, other palettes) replaces it.
///
/// ```
/// use gup_core::prelude::*;
/// use gup_core::{Categorical, NULL_COLOR, OKABE_ITO};
///
/// struct Row { x: f64, y: f64, continent: String }
/// let rows = vec![
///     Row { x: 1.0, y: 2.0, continent: "Asia".into() },
///     Row { x: 2.0, y: 3.0, continent: "Europe".into() },
/// ];
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// plot.add(Selection::<Row, Circle>::new(rows))
///     .attr(Circle::X, x.encode(|r: &Row| r.x))
///     .attr(Circle::Y, y.encode(|r: &Row| r.y))
///     // The accessor returns a borrow of its row.
///     .attr(Circle::FILL, Categorical::okabe_ito().encode_key(|r: &Row| r.continent.as_str()));
/// // Asia was seen first, so it is drawn in the first palette colour.
/// assert_eq!(Categorical::okabe_ito().eval(0.0), OKABE_ITO[0]);
/// assert_eq!(Categorical::okabe_ito().eval(f64::NAN), NULL_COLOR);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Categorical {
    palette: [Color; 8],
}

impl Categorical {
    /// The [`OKABE_ITO`] palette.
    pub fn okabe_ito() -> Self {
        Self { palette: OKABE_ITO }
    }
}

impl Default for Categorical {
    fn default() -> Self {
        Self::okabe_ito()
    }
}

/// One WGSL `vec4<f32>` colour.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
struct Rgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl From<Color> for Rgba {
    fn from(c: Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

/// WGSL `gup::color::categorical::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct CategoricalParams {
    colors: [Rgba; 8],
    null_color: Rgba,
    count: u32,
    // To 16 bytes, as every uniform `Params` (see the WGSL module).
    pad_a: u32,
    pad_b: u32,
    pad_c: u32,
}

impl ShaderFn for Categorical {
    type In = u32;
    type Out = Color;
    type Params = CategoricalParams;
    const MODULE: &'static WgslModule = &COLOR_CATEGORICAL;
    const ENTRY: &'static str = "map";

    fn params(&self) -> CategoricalParams {
        CategoricalParams {
            colors: self.palette.map(Rgba::from),
            null_color: NULL_COLOR.into(),
            count: self.palette.len() as u32,
            pad_a: 0,
            pad_b: 0,
            pad_c: 0,
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::U32
    }
}

impl CpuMirror for Categorical {
    /// The colour of dictionary code `code` (NaN or [`NULL_CODE`] is
    /// null), as the shader picks it.
    fn eval(&self, code: f64) -> Color {
        if !(code.is_finite() && (0.0..f64::from(NULL_CODE)).contains(&code)) {
            return NULL_COLOR;
        }
        self.palette[code as usize % self.palette.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marks::Circle;

    #[test]
    fn codes_cycle_through_the_palette_and_null_is_grey() {
        let c = Categorical::okabe_ito();
        assert_eq!(c.eval(0.0).to_rgba8(), [0xe6, 0x9f, 0x00, 255]);
        assert_eq!(c.eval(4.0), Circle::DEFAULT_FILL);
        assert_eq!(c.eval(8.0), c.eval(0.0));
        assert_eq!(c.eval(f64::from(NULL_CODE)), NULL_COLOR);
        assert_eq!(c.eval(f64::NAN), NULL_COLOR);
        assert!(!OKABE_ITO.contains(&NULL_COLOR));
    }
}
