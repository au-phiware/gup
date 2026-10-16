// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Theme`: the colours, text sizes and spacing a [`Plot`](crate::Plot)
//! styles itself with (RFC-001 S7).

use crate::channel::{Color, Px};
use crate::scale::NULL_COLOR;

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
    /// Tick labels and axis titles.
    pub label: Color,
    /// The chart title.
    pub title: Color,
    /// Chart title text size.
    pub title_size: Px,
    /// Axis title text size.
    pub axis_title_size: Px,
    /// Tick label text size.
    pub tick_size: Px,
    /// Tick mark length.
    pub tick_len: Px,
    /// Gap between a tick and its label, and between labels and titles.
    pub label_gap: Px,
    /// Inset of the axis ranges, so marks on the domain edge stay off the
    /// plot rect's edge.
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
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            axis: Color::hex(0x333333),
            label: Color::hex(0x333333),
            title: Color::hex(0x111111),
            title_size: Px(18.0),
            axis_title_size: Px(13.0),
            tick_size: Px(12.0),
            tick_len: Px(5.0),
            label_gap: Px(4.0),
            plot_padding: Px(4.0),
            pad: Px(12.0),
            x_tick_spacing: Px(80.0),
            y_tick_spacing: Px(45.0),
            null_color: NULL_COLOR,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the values `plot.rs` hard-coded before themes.
    #[test]
    fn default_is_the_old_styling() {
        let t = Theme::default();
        assert_eq!(t.background, Color::WHITE);
        assert_eq!(t.axis, Color::hex(0x333333));
        assert_eq!(t.label, Color::hex(0x333333));
        assert_eq!(t.title, Color::hex(0x111111));
        assert_eq!(t.title_size, Px(18.0));
        assert_eq!(t.axis_title_size, Px(13.0));
        assert_eq!(t.tick_size, Px(12.0));
        assert_eq!(t.tick_len, Px(5.0));
        assert_eq!(t.label_gap, Px(4.0));
        assert_eq!(t.pad, Px(12.0));
        assert_eq!(t.x_tick_spacing, Px(80.0));
        assert_eq!(t.y_tick_spacing, Px(45.0));
        assert_eq!(t.null_color, NULL_COLOR);
    }
}
