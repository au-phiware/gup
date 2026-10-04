// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! 8-bit RGBA colours and perceptual colour difference (CIEDE2000).

use std::fmt;

/// An 8-bit, sRGB-encoded, straight-alpha RGBA colour.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Rgba8 {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (255 = opaque).
    pub a: u8,
}

impl Rgba8 {
    /// Opaque white.
    pub const WHITE: Rgba8 = Rgba8::rgb(255, 255, 255);
    /// Opaque black.
    pub const BLACK: Rgba8 = Rgba8::rgb(0, 0, 0);
    /// Fully transparent black.
    pub const TRANSPARENT: Rgba8 = Rgba8::new(0, 0, 0, 0);

    /// Create a colour from all four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Create an opaque colour from a `0xRRGGBB` literal.
    pub const fn from_hex(hex: u32) -> Self {
        Self::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// Create a colour from normalised `[r, g, b, a]` floats in `0.0..=1.0`
    /// (sRGB-encoded, as public colour APIs take them).
    pub fn from_unit_f32(rgba: [f32; 4]) -> Self {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self::new(q(rgba[0]), q(rgba[1]), q(rgba[2]), q(rgba[3]))
    }

    /// Composite this colour over an opaque backdrop (straight alpha).
    pub fn over(self, backdrop: Rgba8) -> Rgba8 {
        let a = self.a as f32 / 255.0;
        let mix = |fg: u8, bg: u8| (fg as f32 * a + bg as f32 * (1.0 - a)).round() as u8;
        Rgba8::rgb(
            mix(self.r, backdrop.r),
            mix(self.g, backdrop.g),
            mix(self.b, backdrop.b),
        )
    }

    /// Perceptual difference (CIEDE2000) between two colours.
    ///
    /// Alpha is taken into account by compositing both colours over white
    /// and over black and returning the larger difference, so two colours
    /// that only differ in alpha are still reported as different.
    pub fn delta_e(self, other: Rgba8) -> f32 {
        if self == other {
            return 0.0;
        }
        if self.a == 255 && other.a == 255 {
            return delta_e_2000(srgb_to_lab(self), srgb_to_lab(other)) as f32;
        }
        let on_white = delta_e_2000(
            srgb_to_lab(self.over(Rgba8::WHITE)),
            srgb_to_lab(other.over(Rgba8::WHITE)),
        );
        let on_black = delta_e_2000(
            srgb_to_lab(self.over(Rgba8::BLACK)),
            srgb_to_lab(other.over(Rgba8::BLACK)),
        );
        on_white.max(on_black) as f32
    }
}

impl fmt::Debug for Rgba8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rgba({}, {}, {}, {}) #{:02x}{:02x}{:02x}",
            self.r, self.g, self.b, self.a, self.r, self.g, self.b
        )
    }
}

/// A colour in CIE L\*a\*b\* (D65 white point).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Lab {
    /// Lightness, 0–100.
    pub l: f64,
    /// Green–red axis.
    pub a: f64,
    /// Blue–yellow axis.
    pub b: f64,
}

/// Convert an sRGB-encoded colour (alpha ignored) to CIE L\*a\*b\* (D65).
pub fn srgb_to_lab(c: Rgba8) -> Lab {
    fn linear(v: u8) -> f64 {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    let (r, g, b) = (linear(c.r), linear(c.g), linear(c.b));
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
    // D65 reference white.
    let (xn, yn, zn) = (0.950_47, 1.0, 1.088_83);
    fn f(t: f64) -> f64 {
        const DELTA: f64 = 6.0 / 29.0;
        if t > DELTA * DELTA * DELTA {
            t.cbrt()
        } else {
            t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    }
    let (fx, fy, fz) = (f(x / xn), f(y / yn), f(z / zn));
    Lab {
        l: 116.0 * fy - 16.0,
        a: 500.0 * (fx - fy),
        b: 200.0 * (fy - fz),
    }
}

/// CIEDE2000 colour difference between two L\*a\*b\* colours
/// (kL = kC = kH = 1).
///
/// A difference below ~1 is imperceptible; ~2.3 is a "just noticeable
/// difference".
pub fn delta_e_2000(c1: Lab, c2: Lab) -> f64 {
    use std::f64::consts::PI;
    let deg = |r: f64| r * 180.0 / PI;
    let rad = |d: f64| d * PI / 180.0;

    let c1_ab = c1.a.hypot(c1.b);
    let c2_ab = c2.a.hypot(c2.b);
    let c_bar = (c1_ab + c2_ab) / 2.0;
    let c_bar7 = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar7 / (c_bar7 + 25f64.powi(7))).sqrt());

    let a1p = (1.0 + g) * c1.a;
    let a2p = (1.0 + g) * c2.a;
    let c1p = a1p.hypot(c1.b);
    let c2p = a2p.hypot(c2.b);

    let hue = |b: f64, ap: f64| {
        if b == 0.0 && ap == 0.0 {
            0.0
        } else {
            let h = deg(b.atan2(ap));
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let h1p = hue(c1.b, a1p);
    let h2p = hue(c2.b, a2p);

    let dl = c2.l - c1.l;
    let dc = c2p - c1p;
    let dh_angle = if c1p * c2p == 0.0 {
        0.0
    } else {
        let d = h2p - h1p;
        if d > 180.0 {
            d - 360.0
        } else if d < -180.0 {
            d + 360.0
        } else {
            d
        }
    };
    let dh = 2.0 * (c1p * c2p).sqrt() * rad(dh_angle / 2.0).sin();

    let l_bar = (c1.l + c2.l) / 2.0;
    let cp_bar = (c1p + c2p) / 2.0;
    let hp_bar = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };

    let t = 1.0 - 0.17 * rad(hp_bar - 30.0).cos()
        + 0.24 * rad(2.0 * hp_bar).cos()
        + 0.32 * rad(3.0 * hp_bar + 6.0).cos()
        - 0.20 * rad(4.0 * hp_bar - 63.0).cos();
    let d_theta = 30.0 * (-((hp_bar - 275.0) / 25.0).powi(2)).exp();
    let cp_bar7 = cp_bar.powi(7);
    let rc = 2.0 * (cp_bar7 / (cp_bar7 + 25f64.powi(7))).sqrt();
    let l50 = (l_bar - 50.0).powi(2);
    let sl = 1.0 + 0.015 * l50 / (20.0 + l50).sqrt();
    let sc = 1.0 + 0.045 * cp_bar;
    let sh = 1.0 + 0.015 * cp_bar * t;
    let rt = -rad(2.0 * d_theta).sin() * rc;

    let (tl, tc, th) = (dl / sl, dc / sc, dh / sh);
    (tl * tl + tc * tc + th * th + rt * tc * th).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab(l: f64, a: f64, b: f64) -> Lab {
        Lab { l, a, b }
    }

    /// Reference pairs from Sharma, Wu & Dalal (2005), "The CIEDE2000
    /// Color-Difference Formula: Implementation Notes".
    #[test]
    fn ciede2000_matches_published_reference_pairs() {
        let cases = [
            (
                lab(50.0, 2.6772, -79.7751),
                lab(50.0, 0.0, -82.7485),
                2.0425,
            ),
            (lab(50.0, 0.0, 0.0), lab(50.0, -1.0, 2.0), 2.3669),
            (lab(50.0, 2.5, 0.0), lab(73.0, 25.0, -18.0), 27.1492),
            (
                lab(60.2574, -34.0099, 36.2677),
                lab(60.4626, -34.1751, 39.4387),
                1.2644,
            ),
            (lab(50.0, 2.5, 0.0), lab(50.0, 0.0, -2.5), 4.3065),
            (
                lab(2.0776, 0.0795, -1.1350),
                lab(0.9033, -0.0636, -0.5514),
                0.9082,
            ),
        ];
        for (a, b, expected) in cases {
            let got = delta_e_2000(a, b);
            assert!(
                (got - expected).abs() < 1e-4,
                "ΔE00({a:?}, {b:?}) = {got}, expected {expected}"
            );
            // Symmetric.
            assert!((delta_e_2000(b, a) - got).abs() < 1e-9);
        }
    }

    #[test]
    fn srgb_white_and_black_map_to_lab_extremes() {
        let w = srgb_to_lab(Rgba8::WHITE);
        assert!((w.l - 100.0).abs() < 0.01 && w.a.abs() < 0.01 && w.b.abs() < 0.01);
        let k = srgb_to_lab(Rgba8::BLACK);
        assert!(k.l.abs() < 0.01);
    }

    #[test]
    fn delta_e_identical_is_zero_and_alpha_matters() {
        let c = Rgba8::from_hex(0x1f77b4);
        assert_eq!(c.delta_e(c), 0.0);
        assert!(c.delta_e(Rgba8::new(0x1f, 0x77, 0xb4, 0)) > 10.0);
        // The double gamma-encoded variant of #1f77b4 is clearly different.
        assert!(c.delta_e(Rgba8::rgb(142, 189, 219)) > 10.0);
        // One code value off is imperceptible.
        assert!(c.delta_e(Rgba8::rgb(0x20, 0x77, 0xb4)) < 1.0);
    }

    #[test]
    fn from_unit_f32_rounds_and_clamps() {
        assert_eq!(
            Rgba8::from_unit_f32([1.2, 0.5, -0.1, 1.0]),
            Rgba8::new(255, 128, 0, 255)
        );
    }
}
