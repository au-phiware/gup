// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Enhanced color description for accessibility.
//!
//! Provides HSL-based color naming that converts RGBA values to
//! human-readable color names for screen reader users. Supports
//! both basic names (e.g. "red") and detailed descriptions
//! (e.g. "light grayish-blue").
//!
//! # Examples
//!
//! ```rust
//! use gup::color_descriptor::{describe_color, describe_color_detailed};
//!
//! // Basic description
//! assert_eq!(describe_color([1.0, 0.5, 0.0, 1.0]), "orange");
//!
//! // Detailed description
//! assert_eq!(describe_color_detailed([0.7, 0.7, 0.85, 1.0]), "light grayish-blue");
//! ```

/// HSL representation of a colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    /// Hue in degrees (0–360).
    pub h: f32,
    /// Saturation (0–1).
    pub s: f32,
    /// Lightness (0–1).
    pub l: f32,
}

/// Convert an RGBA colour (each component 0–1) to HSL.
pub fn rgba_to_hsl(rgba: [f32; 4]) -> Hsl {
    let [r, g, b, _a] = rgba;
    let r = r.clamp(0.0, 1.0);
    let g = g.clamp(0.0, 1.0);
    let b = b.clamp(0.0, 1.0);

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return Hsl { h: 0.0, s: 0.0, l };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut hue = (g - b) / d;
        if g < b {
            hue += 6.0;
        }
        hue
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    Hsl { h: h * 60.0, s, l }
}

/// Return a base hue name from a hue angle (0–360).
fn hue_name(h: f32) -> &'static str {
    let h = ((h % 360.0) + 360.0) % 360.0;
    match h as u32 {
        0..=14 => "red",
        15..=39 => "orange",
        40..=64 => "yellow",
        65..=74 => "yellow-green",
        75..=159 => "green",
        160..=189 => "cyan",
        190..=259 => "blue",
        260..=289 => "purple",
        290..=329 => "magenta",
        330..=360 => "red",
        _ => "red",
    }
}

/// Whether the colour is perceptually brown.
///
/// Brown is a dark, warm, low-saturation colour that sits
/// roughly in the orange–yellow hue range.
fn is_brown(hsl: Hsl) -> bool {
    let h = ((hsl.h % 360.0) + 360.0) % 360.0;
    (10.0..=50.0).contains(&h) && hsl.l < 0.45 && hsl.s > 0.15 && hsl.s < 0.85
}

/// Whether the colour is perceptually pink.
///
/// Pink is a light, moderately-saturated reddish/magenta colour.
fn is_pink(hsl: Hsl) -> bool {
    let h = ((hsl.h % 360.0) + 360.0) % 360.0;
    ((290.0..=360.0).contains(&h) || (0.0..=15.0).contains(&h)) && hsl.l > 0.55 && hsl.s > 0.15
}

/// Describe an RGBA colour with a simple human-readable name.
///
/// Returns one of: red, orange, yellow, yellow-green, green, cyan,
/// blue, purple, magenta, pink, brown, white, black, gray.
///
/// # Examples
///
/// ```rust
/// use gup::color_descriptor::describe_color;
///
/// assert_eq!(describe_color([1.0, 0.0, 0.0, 1.0]), "red");
/// assert_eq!(describe_color([1.0, 0.5, 0.0, 1.0]), "orange");
/// assert_eq!(describe_color([0.0, 0.0, 0.0, 1.0]), "black");
/// assert_eq!(describe_color([1.0, 1.0, 1.0, 1.0]), "white");
/// assert_eq!(describe_color([0.5, 0.5, 0.5, 1.0]), "gray");
/// ```
pub fn describe_color(rgba: [f32; 4]) -> &'static str {
    let hsl = rgba_to_hsl(rgba);

    // Achromatic cases
    if hsl.l > 0.95 {
        return "white";
    }
    if hsl.l < 0.08 {
        return "black";
    }
    if hsl.s < 0.1 {
        return "gray";
    }

    // Perceptual special cases
    if is_pink(hsl) {
        return "pink";
    }
    if is_brown(hsl) {
        return "brown";
    }

    hue_name(hsl.h)
}

/// Describe an RGBA colour with a detailed human-readable string.
///
/// Includes lightness qualifiers ("light", "dark") and saturation
/// qualifiers ("grayish") when applicable. For example:
/// `"light grayish-blue"`, `"dark red"`, `"green"`.
///
/// # Examples
///
/// ```rust
/// use gup::color_descriptor::describe_color_detailed;
///
/// assert_eq!(describe_color_detailed([0.2, 0.0, 0.0, 1.0]), "dark red");
/// assert_eq!(describe_color_detailed([0.7, 0.7, 0.85, 1.0]), "light grayish-blue");
/// ```
pub fn describe_color_detailed(rgba: [f32; 4]) -> String {
    let hsl = rgba_to_hsl(rgba);

    // Achromatic cases — no qualifiers needed.
    if hsl.l > 0.95 {
        return "white".to_string();
    }
    if hsl.l < 0.08 {
        return "black".to_string();
    }
    if hsl.s < 0.1 {
        // Light / dark gray
        return match () {
            _ if hsl.l > 0.7 => "light gray",
            _ if hsl.l < 0.35 => "dark gray",
            _ => "gray",
        }
        .to_string();
    }

    // Build up qualifiers.
    let mut parts: Vec<&str> = Vec::new();

    // Lightness qualifier
    if hsl.l > 0.7 {
        parts.push("light");
    } else if hsl.l < 0.3 {
        parts.push("dark");
    }

    // Saturation qualifier
    let grayish = hsl.s < 0.35;
    if grayish {
        parts.push("grayish");
    }

    // Base colour name (special cases first)
    let base = if is_pink(hsl) {
        "pink"
    } else if is_brown(hsl) {
        "brown"
    } else {
        hue_name(hsl.h)
    };

    if grayish {
        // Use hyphenated form: "grayish-blue"
        if parts.len() >= 2 {
            // Already has "light" or "dark" before "grayish"
            let prefix = parts[0];
            return format!("{prefix} grayish-{base}");
        }
        return format!("grayish-{base}");
    }

    if parts.is_empty() {
        return base.to_string();
    }

    format!("{} {base}", parts.join(" "))
}

/// A colour naming scheme that users can supply to override
/// the default HSL-based names.
///
/// Implement this trait to provide custom colour names
/// (e.g. brand-specific palette names).
///
/// # Examples
///
/// ```rust
/// use gup::color_descriptor::{ColorNamer, describe_color_with};
///
/// struct BrandPalette;
///
/// impl ColorNamer for BrandPalette {
///     fn name(&self, rgba: [f32; 4]) -> Option<String> {
///         let [r, g, b, _] = rgba;
///         if (r - 0.0).abs() < 0.1 && (g - 0.47).abs() < 0.1 && (b - 0.84).abs() < 0.1 {
///             Some("brand blue".to_string())
///         } else {
///             None // fall back to default
///         }
///     }
/// }
///
/// let name = describe_color_with([0.0, 0.47, 0.84, 1.0], &BrandPalette);
/// assert_eq!(name, "brand blue");
/// ```
pub trait ColorNamer {
    /// Return a custom name for the given RGBA colour, or `None`
    /// to fall back to the default naming algorithm.
    fn name(&self, rgba: [f32; 4]) -> Option<String>;
}

/// Describe a colour using a custom [`ColorNamer`], falling back to the
/// default detailed description when the namer returns `None`.
pub fn describe_color_with(rgba: [f32; 4], namer: &dyn ColorNamer) -> String {
    namer
        .name(rgba)
        .unwrap_or_else(|| describe_color_detailed(rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // RGBA → HSL conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_rgba_to_hsl_pure_red() {
        let hsl = rgba_to_hsl([1.0, 0.0, 0.0, 1.0]);
        assert!((hsl.h - 0.0).abs() < 1.0, "hue: {}", hsl.h);
        assert!((hsl.s - 1.0).abs() < 0.01, "sat: {}", hsl.s);
        assert!((hsl.l - 0.5).abs() < 0.01, "lit: {}", hsl.l);
    }

    #[test]
    fn test_rgba_to_hsl_pure_green() {
        let hsl = rgba_to_hsl([0.0, 1.0, 0.0, 1.0]);
        assert!((hsl.h - 120.0).abs() < 1.0, "hue: {}", hsl.h);
        assert!((hsl.s - 1.0).abs() < 0.01, "sat: {}", hsl.s);
        assert!((hsl.l - 0.5).abs() < 0.01, "lit: {}", hsl.l);
    }

    #[test]
    fn test_rgba_to_hsl_pure_blue() {
        let hsl = rgba_to_hsl([0.0, 0.0, 1.0, 1.0]);
        assert!((hsl.h - 240.0).abs() < 1.0, "hue: {}", hsl.h);
        assert!((hsl.s - 1.0).abs() < 0.01, "sat: {}", hsl.s);
        assert!((hsl.l - 0.5).abs() < 0.01, "lit: {}", hsl.l);
    }

    #[test]
    fn test_rgba_to_hsl_white() {
        let hsl = rgba_to_hsl([1.0, 1.0, 1.0, 1.0]);
        assert!((hsl.s - 0.0).abs() < 0.01);
        assert!((hsl.l - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rgba_to_hsl_black() {
        let hsl = rgba_to_hsl([0.0, 0.0, 0.0, 1.0]);
        assert!((hsl.s - 0.0).abs() < 0.01);
        assert!((hsl.l - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_rgba_to_hsl_gray() {
        let hsl = rgba_to_hsl([0.5, 0.5, 0.5, 1.0]);
        assert!((hsl.s - 0.0).abs() < 0.01);
        assert!((hsl.l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_rgba_to_hsl_orange() {
        // #FF8000 ≈ [1.0, 0.5, 0.0]
        let hsl = rgba_to_hsl([1.0, 0.5, 0.0, 1.0]);
        assert!((hsl.h - 30.0).abs() < 1.0, "hue: {}", hsl.h);
        assert!((hsl.s - 1.0).abs() < 0.01, "sat: {}", hsl.s);
        assert!((hsl.l - 0.5).abs() < 0.01, "lit: {}", hsl.l);
    }

    #[test]
    fn test_rgba_to_hsl_yellow() {
        let hsl = rgba_to_hsl([1.0, 1.0, 0.0, 1.0]);
        assert!((hsl.h - 60.0).abs() < 1.0, "hue: {}", hsl.h);
    }

    #[test]
    fn test_rgba_to_hsl_cyan() {
        let hsl = rgba_to_hsl([0.0, 1.0, 1.0, 1.0]);
        assert!((hsl.h - 180.0).abs() < 1.0, "hue: {}", hsl.h);
    }

    // -----------------------------------------------------------------------
    // Basic colour naming
    // -----------------------------------------------------------------------

    #[test]
    fn test_basic_primaries() {
        assert_eq!(describe_color([1.0, 0.0, 0.0, 1.0]), "red");
        assert_eq!(describe_color([0.0, 1.0, 0.0, 1.0]), "green");
        assert_eq!(describe_color([0.0, 0.0, 1.0, 1.0]), "blue");
    }

    #[test]
    fn test_basic_secondaries() {
        assert_eq!(describe_color([1.0, 1.0, 0.0, 1.0]), "yellow");
        assert_eq!(describe_color([0.0, 1.0, 1.0, 1.0]), "cyan");
        assert_eq!(describe_color([1.0, 0.0, 1.0, 1.0]), "magenta");
    }

    #[test]
    fn test_basic_achromatic() {
        assert_eq!(describe_color([1.0, 1.0, 1.0, 1.0]), "white");
        assert_eq!(describe_color([0.0, 0.0, 0.0, 1.0]), "black");
        assert_eq!(describe_color([0.5, 0.5, 0.5, 1.0]), "gray");
    }

    #[test]
    fn test_orange() {
        assert_eq!(describe_color([1.0, 0.5, 0.0, 1.0]), "orange");
        assert_eq!(describe_color([1.0, 0.65, 0.0, 1.0]), "orange");
    }

    #[test]
    fn test_purple() {
        // Violet: hue 270
        assert_eq!(describe_color([0.5, 0.0, 1.0, 1.0]), "purple");
        // Dark purple
        assert_eq!(describe_color([0.3, 0.0, 0.5, 1.0]), "purple");
    }

    #[test]
    fn test_magenta() {
        // Fuchsia / magenta: hue 300
        assert_eq!(describe_color([1.0, 0.0, 1.0, 1.0]), "magenta");
        assert_eq!(describe_color([0.5, 0.0, 0.5, 1.0]), "magenta");
    }

    #[test]
    fn test_pink() {
        assert_eq!(describe_color([1.0, 0.75, 0.8, 1.0]), "pink");
        assert_eq!(describe_color([1.0, 0.71, 0.76, 1.0]), "pink");
    }

    #[test]
    fn test_brown() {
        // Typical brown: dark orange
        assert_eq!(describe_color([0.55, 0.27, 0.07, 1.0]), "brown");
        // Saddle brown (#8B4513)
        assert_eq!(describe_color([0.545, 0.27, 0.075, 1.0]), "brown");
    }

    #[test]
    fn test_near_white() {
        assert_eq!(describe_color([0.97, 0.97, 0.97, 1.0]), "white");
    }

    #[test]
    fn test_near_black() {
        assert_eq!(describe_color([0.05, 0.05, 0.05, 1.0]), "black");
    }

    #[test]
    fn test_dark_gray() {
        assert_eq!(describe_color([0.25, 0.25, 0.25, 1.0]), "gray");
    }

    #[test]
    fn test_alpha_ignored() {
        assert_eq!(describe_color([1.0, 0.0, 0.0, 0.5]), "red");
        assert_eq!(describe_color([1.0, 0.0, 0.0, 0.0]), "red");
    }

    // -----------------------------------------------------------------------
    // Detailed colour naming
    // -----------------------------------------------------------------------

    #[test]
    fn test_detailed_light_blue() {
        let name = describe_color_detailed([0.6, 0.6, 1.0, 1.0]);
        assert!(
            name.starts_with("light"),
            "expected 'light …' but got '{name}'"
        );
        assert!(name.contains("blue"), "expected '…blue' but got '{name}'");
    }

    #[test]
    fn test_detailed_dark_red() {
        let name = describe_color_detailed([0.3, 0.0, 0.0, 1.0]);
        assert_eq!(name, "dark red");
    }

    #[test]
    fn test_detailed_grayish_blue() {
        let name = describe_color_detailed([0.45, 0.45, 0.6, 1.0]);
        assert!(name.contains("grayish"), "expected 'grayish' in '{name}'");
        assert!(name.contains("blue"), "expected 'blue' in '{name}'");
    }

    #[test]
    fn test_detailed_light_grayish_blue() {
        // HSL(240, ~0.33, ~0.78) — desaturated light blue
        let name = describe_color_detailed([0.7, 0.7, 0.85, 1.0]);
        assert_eq!(name, "light grayish-blue");
    }

    #[test]
    fn test_detailed_white() {
        assert_eq!(describe_color_detailed([1.0, 1.0, 1.0, 1.0]), "white");
    }

    #[test]
    fn test_detailed_black() {
        assert_eq!(describe_color_detailed([0.0, 0.0, 0.0, 1.0]), "black");
    }

    #[test]
    fn test_detailed_gray_shades() {
        let light = describe_color_detailed([0.8, 0.8, 0.8, 1.0]);
        assert_eq!(light, "light gray");

        let dark = describe_color_detailed([0.2, 0.2, 0.2, 1.0]);
        assert_eq!(dark, "dark gray");

        let mid = describe_color_detailed([0.5, 0.5, 0.5, 1.0]);
        assert_eq!(mid, "gray");
    }

    #[test]
    fn test_detailed_pure_colour_no_qualifier() {
        let name = describe_color_detailed([1.0, 0.0, 0.0, 1.0]);
        assert_eq!(name, "red");
    }

    // -----------------------------------------------------------------------
    // Custom namer
    // -----------------------------------------------------------------------

    struct TestNamer;

    impl ColorNamer for TestNamer {
        fn name(&self, rgba: [f32; 4]) -> Option<String> {
            let [r, g, b, _] = rgba;
            if (r - 1.0).abs() < 0.01 && g < 0.01 && b < 0.01 {
                Some("danger".to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn test_custom_namer_match() {
        let name = describe_color_with([1.0, 0.0, 0.0, 1.0], &TestNamer);
        assert_eq!(name, "danger");
    }

    #[test]
    fn test_custom_namer_fallback() {
        let name = describe_color_with([0.0, 0.0, 1.0, 1.0], &TestNamer);
        assert_eq!(name, "blue");
    }

    // -----------------------------------------------------------------------
    // Common data visualisation palette colours
    // -----------------------------------------------------------------------

    /// Tableau 10 palette names and their expected basic descriptions.
    #[test]
    fn test_tableau_10_palette() {
        // Tableau 10 colours (approximate sRGB 0-1)
        let palette: &[([f32; 4], &str)] = &[
            ([0.122, 0.467, 0.706, 1.0], "blue"),   // #1f77b4
            ([1.0, 0.498, 0.055, 1.0], "orange"),   // #ff7f0e
            ([0.173, 0.627, 0.173, 1.0], "green"),  // #2ca02c
            ([0.839, 0.153, 0.157, 1.0], "red"),    // #d62728
            ([0.580, 0.404, 0.741, 1.0], "purple"), // #9467bd
            ([0.549, 0.337, 0.294, 1.0], "brown"),  // #8c564b
            ([0.890, 0.467, 0.761, 1.0], "pink"),   // #e377c2
            ([0.498, 0.498, 0.498, 1.0], "gray"),   // #7f7f7f
            ([0.737, 0.741, 0.133, 1.0], "yellow"), // #bcbd22
            ([0.090, 0.745, 0.812, 1.0], "cyan"),   // #17becf
        ];

        for &(rgba, expected) in palette {
            let got = describe_color(rgba);
            assert_eq!(
                got, expected,
                "Tableau10 colour {:?} → expected {expected}, got {got}",
                rgba
            );
        }
    }

    // -----------------------------------------------------------------------
    // CSS named colour spot-checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_css_named_colours() {
        // A representative set of CSS named colours.
        let cases: &[([f32; 4], &str)] = &[
            ([1.0, 0.0, 0.0, 1.0], "red"),      // red
            ([0.0, 0.502, 0.0, 1.0], "green"),  // green (#008000)
            ([0.0, 0.0, 1.0, 1.0], "blue"),     // blue
            ([1.0, 1.0, 0.0, 1.0], "yellow"),   // yellow
            ([0.0, 1.0, 1.0, 1.0], "cyan"),     // cyan / aqua
            ([1.0, 0.0, 1.0, 1.0], "magenta"), // magenta / fuchsia            ([0.502, 0.0, 0.502, 1.0], "purple"),    // purple (#800080)
            ([1.0, 0.647, 0.0, 1.0], "orange"), // orange
        ];

        for &(rgba, expected) in cases {
            let got = describe_color(rgba);
            assert_eq!(
                got, expected,
                "CSS colour {:?} → expected {expected}, got {got}",
                rgba
            );
        }
    }
}
