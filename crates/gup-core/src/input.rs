// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Input a host passes to a [`Chart`](crate::Chart).

use crate::geom::Point;

/// Pointer and wheel input in chart coordinates: logical pixels from the
/// chart's top-left corner, y down (the coordinates of
/// [`Layout`](crate::Layout)).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Input {
    /// The pointer moved over the chart.
    PointerMoved {
        /// Where, in chart coordinates.
        at: Point,
    },
    /// The pointer left the chart.
    PointerLeft,
    /// A button was pressed.
    PointerDown {
        /// Where, in chart coordinates.
        at: Point,
        /// Which button.
        button: Button,
    },
    /// A button was released.
    PointerUp {
        /// Where, in chart coordinates.
        at: Point,
        /// Which button.
        button: Button,
    },
    /// The wheel turned.
    Wheel {
        /// Where the pointer is, in chart coordinates.
        at: Point,
        /// `lines > 0` scrolls up (away from the user), which zooms in.
        lines: f32,
    },
}

/// A pointer button.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Button {
    /// The left button (or a touch).
    Primary,
    /// The right button.
    Secondary,
    /// The middle button.
    Middle,
}

/// What the host should do after [`Chart::handle`](crate::Chart::handle).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The chart looks different: resolve and render again.
    pub redraw: bool,
}
