// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::legend::{ColorScale, Legend, Swatch};
use super::sequential::{oklab_to_srgb, srgb_to_oklab};
use crate::channel::Color;
use crate::column::{ColumnFormat, NULL_CODE};
use crate::encoding::{CpuMirror, Resource, ShaderFn};
use crate::error::Result;
use crate::shader::{COLOR_CATEGORICAL, WgslModule};
use std::sync::{Arc, OnceLock};

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

/// Distinct colours a categorical palette provides: its own, then
/// generated ones up to this many. Past them, colours repeat.
pub const CATEGORICAL_COLORS: usize = 64;

/// The most colours a categorical palette may have: a LUT texture is one
/// row of texels, and 2048 is the narrowest a device may allow
/// (`max_texture_dimension_2d` on downlevel devices).
pub const MAX_PALETTE: usize = 2048;

/// A categorical colour scale: dictionary codes onto a palette, sized to
/// the dictionary's keys, with [`NULL_COLOR`] for a missing key.
///
/// Drive it with [`EncodeFn::encode_key`](crate::EncodeFn::encode_key)
/// (or [`encode_nullable_key`](crate::EncodeFn::encode_nullable_key), or
/// [`encode_owned_key`](crate::EncodeFn::encode_owned_key) for integer
/// and enum keys). Its domain is the channel's keys, in first-seen order
/// ([`ShaderFn::fit_keys`]): the first key draws in the first palette
/// colour, and [`legend`](ColorScale::legend) lists the keys in that
/// order with their colours. A new key (appended rows) grows the domain:
/// one uniform write, with no column bytes and no texture write.
///
/// **Palette.** The colours live in a one-row LUT texture read by texel,
/// so a palette may have any length up to [`MAX_PALETTE`]. The default is
/// [`OKABE_ITO`] (8 colours, colour-blind safe). A domain with more keys
/// than the palette has colours does not cycle: the scale extends the
/// palette, once, to [`CATEGORICAL_COLORS`] with generated colours, each
/// the farthest (in OKLab) from every colour before it, from white (the
/// background) and from [`NULL_COLOR`]. Generated colours are distinct
/// but not colour-blind safe; past [`CATEGORICAL_COLORS`] keys, colours
/// repeat. A new key never changes an earlier key's colour.
///
/// ```
/// use gup_core::prelude::*;
/// use gup_core::{NULL_COLOR, OKABE_ITO};
///
/// struct Row { x: f64, y: f64, continent: Option<String> }
/// let rows = vec![
///     Row { x: 1.0, y: 1.0, continent: Some("Asia".into()) },
///     Row { x: 2.0, y: 2.0, continent: Some("Europe".into()) },
///     Row { x: 3.0, y: 3.0, continent: None },
/// ];
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// let colour = ScaleRef::new(Categorical::okabe_ito());
/// plot.add(Selection::<Row, Circle>::new(rows))
///     .attr(Circle::X, x.encode(|r: &Row| r.x))
///     .attr(Circle::Y, y.encode(|r: &Row| r.y))
///     .attr(Circle::RADIUS, Px(6.0))
///     // The accessor returns a borrow of its row; `None` is a null.
///     .attr(
///         Circle::FILL,
///         colour.encode_nullable_key(|r: &Row| r.continent.as_deref()),
///     );
/// let image = plot.render(&cx, 320, 200)?;
///
/// // Codes in first-seen order: Asia 0, Europe 1; the missing key is null.
/// let centre = |v: f64| image.get_pixel(x.read().eval(v) as u32, y.read().eval(v) as u32).0;
/// assert_eq!(centre(1.0), OKABE_ITO[0].to_rgba8());
/// assert_eq!(centre(2.0), OKABE_ITO[1].to_rgba8());
/// assert_eq!(centre(3.0), NULL_COLOR.to_rgba8());
/// // The legend lists the keys, with the colours drawn.
/// let legend = colour.read().legend();
/// let swatches = legend.swatches();
/// assert_eq!((&*swatches[1].label, swatches[1].color), ("Europe", OKABE_ITO[1]));
/// # Ok::<(), gup_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Categorical {
    /// The palette extended to at least [`CATEGORICAL_COLORS`] entries,
    /// sRGB-encoded: entry `code % len` colours `code`.
    lut: Arc<[[u8; 4]]>,
    /// The domain: the keys' labels, in code order.
    keys: Vec<Arc<str>>,
}

impl Categorical {
    /// The [`OKABE_ITO`] palette (extended past 8 keys).
    pub fn okabe_ito() -> Self {
        static LUT: OnceLock<Arc<[[u8; 4]]>> = OnceLock::new();
        Self {
            lut: Arc::clone(LUT.get_or_init(|| extend(&OKABE_ITO).into())),
            keys: Vec::new(),
        }
    }

    /// A palette of `colors`, in order (extended past its length up to
    /// [`CATEGORICAL_COLORS`] keys). Colours are stored as 8-bit sRGB.
    ///
    /// # Panics
    ///
    /// If `colors` is empty or longer than [`MAX_PALETTE`].
    pub fn from_palette(colors: impl IntoIterator<Item = Color>) -> Self {
        let colors: Vec<Color> = colors.into_iter().collect();
        assert!(
            (1..=MAX_PALETTE).contains(&colors.len()),
            "a categorical palette needs 1 to {MAX_PALETTE} colours, not {}",
            colors.len()
        );
        Self {
            lut: extend(&colors).into(),
            keys: Vec::new(),
        }
    }

    /// The domain: each key's label, in code (first-seen) order.
    pub fn keys(&self) -> &[Arc<str>] {
        &self.keys
    }

    /// How many distinct colours the palette has before repeating.
    pub fn colors(&self) -> usize {
        self.lut.len()
    }

    /// The colour of LUT entry `i`.
    fn entry(&self, i: usize) -> Color {
        let [r, g, b, a] = self.lut[i].map(|c| f32::from(c) / 255.0);
        Color { r, g, b, a }
    }
}

impl Default for Categorical {
    fn default() -> Self {
        Self::okabe_ito()
    }
}

/// `palette` as 8-bit sRGB, extended with generated colours to
/// [`CATEGORICAL_COLORS`] entries (a longer palette is kept as it is).
///
/// Each generated colour is the candidate farthest, in OKLab, from every
/// colour already chosen, from white and from [`NULL_COLOR`]. Candidates
/// are in-gamut OKLCh colours at four lightnesses (0.45 to 0.75) and 36
/// hues, at the most chroma up to 0.15 that stays in sRGB. Greedy, so a
/// longer domain never changes an earlier colour.
fn extend(palette: &[Color]) -> Vec<[u8; 4]> {
    let quantise = |c: Color| c.to_rgba8();
    let mut lut: Vec<[u8; 4]> = palette.iter().map(|&c| quantise(c)).collect();
    if lut.len() >= CATEGORICAL_COLORS {
        return lut;
    }
    let lab = |c: [u8; 4]| srgb_to_oklab([c[0], c[1], c[2]].map(|v| f32::from(v) / 255.0));
    let mut taken: Vec<[f32; 3]> = lut.iter().map(|&c| lab(c)).collect();
    taken.push(lab([255, 255, 255, 255]));
    taken.push(lab(NULL_COLOR.to_rgba8()));
    let mut candidates: Vec<([u8; 4], [f32; 3])> = Vec::new();
    for l in [0.45f32, 0.55, 0.65, 0.75] {
        for h in (0..36).map(|i| (i as f32 * 10.0).to_radians()) {
            // The most chroma (≤ 0.15) that stays in gamut.
            let fits = |c: f32| {
                let lab = [l, c * h.cos(), c * h.sin()];
                in_gamut(lab).then_some(lab)
            };
            let Some(lab_c) = (0..=15).rev().find_map(|k| fits(k as f32 * 0.01)) else {
                continue;
            };
            let rgb = oklab_to_srgb(lab_c).map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            let c = [rgb[0], rgb[1], rgb[2], 255];
            candidates.push((c, lab(c)));
        }
    }
    let dist = |a: [f32; 3], b: [f32; 3]| {
        (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
    };
    while lut.len() < CATEGORICAL_COLORS && !candidates.is_empty() {
        let nearest = |p: [f32; 3]| taken.iter().map(|&t| dist(p, t)).fold(f32::MAX, f32::min);
        let (best, _) = candidates
            .iter()
            .enumerate()
            .map(|(i, &(_, p))| (i, nearest(p)))
            .fold((0, f32::MIN), |a, b| if b.1 > a.1 { b } else { a });
        let (c, p) = candidates.swap_remove(best);
        lut.push(c);
        taken.push(p);
    }
    lut
}

/// Whether OKLab `lab` is inside the sRGB gamut.
fn in_gamut([l, a, b]: [f32; 3]) -> bool {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let rgb = [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ];
    rgb.iter().all(|c| (-1e-4..=1.0 + 1e-4).contains(c))
}

/// One WGSL `vec4<f32>` colour (as a std140 struct of four `f32`s, which
/// lays out like the vector at a 16-byte offset).
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub(crate) struct Rgba {
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
            null_color: NULL_COLOR.into(),
            count: u32::try_from(self.keys.len()).unwrap_or(NULL_CODE),
            pad_a: 0,
            pad_b: 0,
            pad_c: 0,
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::U32
    }

    fn resources(&self) -> Vec<Resource> {
        vec![Resource::Lut(self.lut.to_vec())]
    }

    fn fit_keys(&mut self, keys: &[Arc<str>]) -> Result<()> {
        if self.keys != keys {
            self.keys = keys.to_vec();
        }
        Ok(())
    }
}

impl CpuMirror for Categorical {
    /// The colour of dictionary code `code`, as the shader picks it:
    /// [`NULL_COLOR`] for NaN, [`NULL_CODE`] or a code outside the domain.
    fn eval(&self, code: f64) -> Color {
        if !(code.is_finite() && code >= 0.0 && code < self.keys.len() as f64) {
            return NULL_COLOR;
        }
        self.entry(code as usize % self.lut.len())
    }
}

impl ColorScale for Categorical {
    fn legend(&self) -> Legend {
        Legend::Swatches(
            self.keys
                .iter()
                .enumerate()
                .map(|(code, label)| Swatch {
                    label: Arc::clone(label),
                    color: self.eval(code as f64),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marks::Circle;

    fn fitted(n: usize) -> Categorical {
        let mut c = Categorical::okabe_ito();
        let keys: Vec<Arc<str>> = (0..n).map(|i| format!("k{i}").into()).collect();
        c.fit_keys(&keys).unwrap();
        c
    }

    #[test]
    fn codes_follow_the_palette_and_null_is_grey() {
        let c = fitted(8);
        assert_eq!(c.eval(0.0).to_rgba8(), [0xe6, 0x9f, 0x00, 255]);
        assert_eq!(c.eval(4.0), Circle::DEFAULT_FILL);
        for (i, color) in OKABE_ITO.iter().enumerate() {
            assert_eq!(c.eval(i as f64), *color);
        }
        assert_eq!(c.eval(f64::from(NULL_CODE)), NULL_COLOR);
        assert_eq!(c.eval(f64::NAN), NULL_COLOR);
        // Outside the domain (no such key yet) is null, as on the GPU.
        assert_eq!(c.eval(8.0), NULL_COLOR);
        assert!(!OKABE_ITO.contains(&NULL_COLOR));
    }

    /// AC2: past the palette's 8 colours, keys get generated colours
    /// rather than repeating: every pair of the first 16 keys, and each
    /// against white and the null colour, is at least 0.08 apart in
    /// OKLab (a just-noticeable difference is about 0.02), and growing
    /// the domain never recolours a key.
    #[test]
    fn keys_past_the_palette_get_distinct_generated_colours() {
        let c = fitted(CATEGORICAL_COLORS);
        assert_eq!(c.colors(), CATEGORICAL_COLORS);
        let lab = |c: Color| srgb_to_oklab([c.r, c.g, c.b]);
        let mut seen: Vec<[f32; 3]> = vec![lab(Color::WHITE), lab(NULL_COLOR)];
        let mut min = f32::MAX;
        for code in 0..16 {
            let p = lab(c.eval(f64::from(code)));
            for q in &seen {
                let d =
                    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
                min = min.min(d);
            }
            seen.push(p);
        }
        assert!(min >= 0.08, "closest pair {min}");
        let small = fitted(12);
        for code in 0..12 {
            assert_eq!(small.eval(f64::from(code)), c.eval(f64::from(code)));
        }
        // Past CATEGORICAL_COLORS, colours repeat.
        let big = fitted(CATEGORICAL_COLORS + 1);
        assert_eq!(big.eval(CATEGORICAL_COLORS as f64), big.eval(0.0));
    }

    #[test]
    fn a_long_palette_is_kept_whole() {
        let palette: Vec<Color> = (0..100u32).map(|i| Color::hex(i * 0x020202)).collect();
        let c = Categorical::from_palette(palette.clone());
        assert_eq!(c.colors(), 100);
        let c = {
            let mut c = c;
            let keys: Vec<Arc<str>> = (0..100).map(|i| i.to_string().into()).collect();
            c.fit_keys(&keys).unwrap();
            c
        };
        assert_eq!(c.eval(99.0), palette[99]);
    }

    #[test]
    fn the_legend_lists_keys_in_code_order_with_their_colours() {
        let mut c = Categorical::okabe_ito();
        let keys: Vec<Arc<str>> = ["Asia", "Europe", "Africa"].map(Arc::from).to_vec();
        c.fit_keys(&keys).unwrap();
        let legend = c.legend();
        let s = legend.swatches();
        assert_eq!(s.len(), 3);
        assert_eq!((&*s[2].label, s[2].color), ("Africa", OKABE_ITO[2]));
        assert!(legend.ramp().is_none());
    }
}
