// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Theme`: the colours, text sizes and spacing a [`Plot`](crate::Plot)
//! styles itself with (RFC-001 S7).

use crate::channel::{Color, Px};
use crate::scale::{NULL_COLOR, OKABE_ITO, OKABE_ITO_DARK};

/// How rows outside a brush are drawn while one exists.
///
/// ```
/// use gup_core::prelude::*;
///
/// let theme = Theme {
///     unbrushed: Unbrushed::Grey(Color::hex(0xbfbfbf)),
///     ..Theme::default()
/// };
/// let mut plot = Plot::new();
/// plot.drag(Drag::Brush).theme(theme);
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Unbrushed {
    /// Multiply every colour channel's alpha by this opacity
    /// (0.0..=1.0). `Fade(1.0)` turns dimming off.
    Fade(f32),
    /// Draw every colour channel in this colour; its alpha multiplies
    /// the mark's alpha.
    Grey(Color),
}

impl Default for Unbrushed {
    fn default() -> Self {
        Unbrushed::Fade(0.3)
    }
}

impl Unbrushed {
    /// Whether a brushed plot draws unbrushed rows differently at all.
    pub(crate) fn dims(&self) -> bool {
        match self {
            Unbrushed::Fade(o) => *o < 1.0,
            Unbrushed::Grey(_) => true,
        }
    }
}

/// Styling for a [`Plot`](crate::Plot). The default is the look every
/// plot had before themes existed.
///
/// ```
/// use gup_core::prelude::*;
///
/// let theme = Theme { null_color: Color::hex(0xFF00FF), ..Theme::default() };
/// assert_eq!(theme.tick_size, Px(12.0));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// The canvas colour.
    pub background: Color,
    /// Axis lines and ticks.
    pub axis: Color,
    /// Grid lines (see `Plot::grid`).
    pub grid: Color,
    /// Tick labels and axis titles.
    pub label: Color,
    /// The chart title.
    pub title: Color,
    /// The chart subtitle.
    pub subtitle: Color,
    /// Chart title text size.
    pub title_size: Px,
    /// Chart subtitle text size.
    pub subtitle_size: Px,
    /// Axis title text size.
    pub axis_title_size: Px,
    /// Tick label text size.
    pub tick_size: Px,
    /// Tick mark length.
    pub tick_len: Px,
    /// Gap between a tick and its label, and between labels and titles.
    pub label_gap: Px,
    /// Inset of a numeric axis's far end (right, top), so marks on the
    /// domain edge stay off the plot rect's edge. A band or point axis makes
    /// its own room with its outer padding.
    pub plot_padding: Px,
    /// Margin around the chart.
    pub pad: Px,
    /// Target spacing between x ticks.
    pub x_tick_spacing: Px,
    /// Target spacing between y ticks.
    pub y_tick_spacing: Px,
    /// The colour drawn for a missing colour value (a null `Sequential`
    /// or `Diverging` input, a missing `Categorical` key). A
    /// `Categorical` can override it for itself.
    pub null_color: Color,
    /// Tooltip box fill.
    pub tooltip_background: Color,
    /// Tooltip box border (1 px).
    pub tooltip_border: Color,
    /// Tooltip text colour.
    pub tooltip_text: Color,
    /// Tooltip text size.
    pub tooltip_size: Px,
    /// Brush rectangle fill (translucent).
    pub brush_fill: Color,
    /// Brush rectangle border (1 px).
    pub brush_stroke: Color,
    /// How rows outside the brush are drawn while one exists.
    pub unbrushed: Unbrushed,
    /// The default categorical palette
    /// ([`Categorical::okabe_ito`](crate::Categorical::okabe_ito)): its
    /// first eight colours, with later ones generated against
    /// [`background`](Self::background). A [`Categorical::from_palette`]
    /// ignores it.
    ///
    /// [`Categorical::from_palette`]: crate::Categorical::from_palette
    pub palette: [Color; 8],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            axis: Color::hex(0x333333),
            grid: Color::hex(0xe5e5e5),
            label: Color::hex(0x333333),
            title: Color::hex(0x111111),
            subtitle: Color::hex(0x555555),
            title_size: Px(18.0),
            subtitle_size: Px(13.0),
            axis_title_size: Px(13.0),
            tick_size: Px(12.0),
            tick_len: Px(5.0),
            label_gap: Px(4.0),
            plot_padding: Px(4.0),
            pad: Px(12.0),
            x_tick_spacing: Px(80.0),
            y_tick_spacing: Px(45.0),
            null_color: NULL_COLOR,
            tooltip_background: Color::WHITE,
            tooltip_border: Color::hex(0x999999),
            tooltip_text: Color::hex(0x333333),
            tooltip_size: Px(12.0),
            brush_fill: Color {
                a: 0.15,
                ..Color::hex(0x4477aa)
            },
            brush_stroke: Color::hex(0x4477aa),
            unbrushed: Unbrushed::default(),
            palette: OKABE_ITO,
        }
    }
}

impl Theme {
    /// The light theme: black-on-white, the same as [`Theme::default`].
    pub fn light() -> Self {
        Self::default()
    }

    /// A dark theme for dark-mode apps: a near-black background, light
    /// text, a subtle grid, dark tooltips and a blue brush. Sizes and
    /// spacing are the default's. Contrast ratios (WCAG 2) against the
    /// background are tested: labels 7:1, titles 12:1, axes 4.5:1, every
    /// palette colour 3:1.
    ///
    /// The default categorical palette follows the theme
    /// ([`OKABE_ITO_DARK`]: Okabe-Ito with black replaced by a light
    /// grey), and colours generated past eight keys are chosen against the
    /// dark background.
    ///
    /// Not themed, by design: the marks' default colour (`#0072B2`, 3.2:1
    /// on this background, which meets the 3:1 guideline for graphics),
    /// and the sequential and diverging palettes (viridis' dark end has
    /// low contrast on a dark background: a known limit).
    ///
    /// ```
    /// use gup_core::prelude::*;
    ///
    /// struct Row { x: f64, y: f64 }
    /// let rows = vec![Row { x: 1.0, y: 2.0 }, Row { x: 2.0, y: 1.0 }];
    /// let mut chart = gup_core::scatter(rows)
    ///     .x(|r| r.x)
    ///     .y(|r| r.y)
    ///     .theme(Theme::dark());
    /// let image = chart.render(&Context::shared()?, 320, 200)?;
    /// assert_eq!(image.get_pixel(0, 0).0, Theme::dark().background.to_rgba8());
    /// # Ok::<(), gup_core::Error>(())
    /// ```
    pub fn dark() -> Self {
        let blue = Color::hex(0x8ab4f8);
        Self {
            background: Color::hex(0x1e1e22),
            axis: Color::hex(0xa0a0a8),
            grid: Color::hex(0x34343a),
            label: Color::hex(0xc8c8d0),
            title: Color::hex(0xf0f0f4),
            subtitle: Color::hex(0xa8a8b4),
            null_color: Color::hex(0x6e6e78),
            tooltip_background: Color::hex(0x2a2a30),
            tooltip_border: Color::hex(0x5a5a64),
            tooltip_text: Color::hex(0xe6e6ea),
            brush_fill: Color { a: 0.2, ..blue },
            brush_stroke: blue,
            unbrushed: Unbrushed::Fade(0.3),
            palette: OKABE_ITO_DARK,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2 relative luminance of an sRGB colour.
    fn luminance(c: Color) -> f64 {
        let lin = |v: f32| {
            let v = f64::from(v);
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }

    /// WCAG 2 contrast ratio.
    fn contrast(a: Color, b: Color) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn light_is_the_default() {
        assert_eq!(Theme::light(), Theme::default());
        assert_eq!(Theme::default().palette, OKABE_ITO);
    }

    /// AC1: the dark theme's contrast ratios (GUP-553).
    #[test]
    fn dark_colours_meet_their_contrast_targets() {
        let t = Theme::dark();
        let bg = t.background;
        let report = |name: &str, r: f64| eprintln!("{name}: {r:.2}");
        report("label", contrast(t.label, bg));
        report("title", contrast(t.title, bg));
        report("subtitle", contrast(t.subtitle, bg));
        report("axis", contrast(t.axis, bg));
        report("grid", contrast(t.grid, bg));
        report("null", contrast(t.null_color, bg));
        report("tooltip", contrast(t.tooltip_text, t.tooltip_background));
        assert!(contrast(t.label, bg) >= 7.0);
        assert!(contrast(t.title, bg) >= 12.0);
        assert!(contrast(t.subtitle, bg) >= 4.5);
        assert!(contrast(t.subtitle, bg) < contrast(t.title, bg));
        assert!(contrast(t.axis, bg) >= 4.5);
        assert!((1.2..=1.6).contains(&contrast(t.grid, bg)));
        assert!(contrast(t.tooltip_text, t.tooltip_background) >= 7.0);
        for (i, c) in t.palette.iter().enumerate() {
            let r = contrast(*c, bg);
            report(&format!("palette[{i}]"), r);
            assert!(r >= 3.0, "palette[{i}] {c:?}: {r:.2}");
        }
        assert!(contrast(t.null_color, bg) >= 3.0);
        assert!(contrast(t.brush_stroke, bg) >= 3.0);
    }

    /// The light palette is the old one; only the last colour differs.
    #[test]
    fn dark_palette_replaces_only_black() {
        assert_eq!(OKABE_ITO_DARK[..7], OKABE_ITO[..7]);
        assert_eq!(OKABE_ITO_DARK[7], Color::hex(0xe8e8e8));
        assert_eq!(Theme::dark().palette, OKABE_ITO_DARK);
    }

    /// The defaults are the values `plot.rs` hard-coded before themes.
    #[test]
    fn default_is_the_old_styling() {
        let t = Theme::default();
        assert_eq!(t.background, Color::WHITE);
        assert_eq!(t.axis, Color::hex(0x333333));
        assert_eq!(t.label, Color::hex(0x333333));
        assert_eq!(t.title, Color::hex(0x111111));
        assert_eq!(t.subtitle, Color::hex(0x555555));
        assert_eq!(t.title_size, Px(18.0));
        assert_eq!(t.subtitle_size, Px(13.0));
        assert_eq!(t.axis_title_size, Px(13.0));
        assert_eq!(t.tick_size, Px(12.0));
        assert_eq!(t.tick_len, Px(5.0));
        assert_eq!(t.label_gap, Px(4.0));
        assert_eq!(t.pad, Px(12.0));
        assert_eq!(t.x_tick_spacing, Px(80.0));
        assert_eq!(t.y_tick_spacing, Px(45.0));
        assert_eq!(t.null_color, NULL_COLOR);
        assert_eq!(t.tooltip_background, Color::WHITE);
        assert_eq!(t.tooltip_border, Color::hex(0x999999));
        assert_eq!(t.tooltip_text, Color::hex(0x333333));
        assert_eq!(t.tooltip_size, Px(12.0));
        assert_eq!(t.brush_stroke, Color::hex(0x4477aa));
        assert_eq!(t.unbrushed, Unbrushed::Fade(0.3));
        assert!(Unbrushed::Grey(Color::BLACK).dims());
        assert!(Unbrushed::Fade(0.0).dims());
        assert!(!Unbrushed::Fade(1.0).dims());
        assert_eq!(
            t.brush_fill,
            Color {
                a: 0.15,
                ..Color::hex(0x4477aa)
            }
        );
    }
}
